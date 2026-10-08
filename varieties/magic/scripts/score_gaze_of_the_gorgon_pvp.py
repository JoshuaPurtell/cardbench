#!/usr/bin/env python3
"""Run a Gaze of the Gorgon candidate in a bounded, networkless Rust worker."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import shutil
import subprocess
import sys
import tempfile
import uuid
from pathlib import Path

REPO = Path(__file__).resolve().parents[3]
MAGIC = REPO / "varieties" / "magic"
MATERIALIZER = Path(__file__).with_name("materialize_gaze_of_the_gorgon_task.py")
TARGET = Path("varieties/magic/sets/ravnica_city_of_guilds/src/cards/gaze_of_the_gorgon.rs")
AUTHORITY = Path("varieties/magic/session/src/bin/rav_gaze_of_the_gorgon_pvp.rs")
DEFAULT_WORKER_IMAGE = "rust@sha256:d7482085ff5b415f84dba5647ae71606650bdef00db7aeb69f4b3d170c3e4082"


class WorkerFailure(RuntimeError):
    pass


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def verify_workspace(workspace: Path, manifest: Path) -> dict:
    subprocess.run(
        [sys.executable, str(MATERIALIZER), "--workspace", str(workspace), "--manifest", str(manifest), "--verify"],
        check=True,
        cwd=REPO,
    )
    return json.loads(manifest.read_text(encoding="utf-8"))


def snapshot(workspace: Path, manifest: dict, destination: Path) -> Path:
    destination.mkdir(parents=True, exist_ok=True)
    for relative in sorted(manifest["files"]):
        source = workspace / Path(relative)
        if source.is_symlink() or not source.is_file():
            raise WorkerFailure(f"candidate_snapshot_source_missing: {relative}")
        target = destination / Path(relative)
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(source, target)
    return destination


def _int(value: object) -> bool:
    return type(value) is int


def validate_report(report: dict) -> None:
    if (
        report.get("schema_version") != "cardbench.magic.card-pvp.check.v1"
        or report.get("task_id") != "cardbench/magic/card"
        or report.get("expansion") != "RAV"
        or report.get("passed") is not True
        or report.get("pilot_card_passed") is not True
        or report.get("replay_deterministic") is not True
        or report.get("cards_passed") != 1
        or report.get("cards_total") != 291
        or report.get("printings_total") != 306
        or report.get("cards_credited") != 0
        or report.get("coverage_complete") is not False
        or report.get("full_reward") is not False
        or report.get("harbor_reward") != 0.0
        or report.get("pilot_reward") != 0.0
        or report.get("viewer_scope") != "Judge"
        or report.get("card") != {"collector_number": 246, "definition_id": "RAV-GAZE-OF-THE-GORGON", "name": "Gaze of the Gorgon"}
        or report.get("rules_authority", {}).get("comprehensive_rules") != "2026-08-07"
    ):
        raise WorkerFailure("candidate_protocol_error: incomplete Gaze of the Gorgon result or nonzero credit/reward")
    candidate = report.get("candidate")
    if not isinstance(candidate, dict) or candidate.get("card_definition") != "RAV-GAZE-OF-THE-GORGON":
        raise WorkerFailure("candidate_protocol_error: wrong Gaze candidate identity")
    guard = report.get("candidate_source_validation")
    if not isinstance(guard, dict) or any(
        guard.get(key) is not expected
        for key, expected in (
            ("normalized_literal_source_and_ast_checked_before_candidate_calls", True),
            ("compile_valid_side_effect_candidate_by_construction", True),
            ("compile_valid_injected_side_effect_candidate_rejected_before_candidate_calls", True),
            ("candidate_functions_called_before_source_validation", False),
            ("side_effect_fixture_executed", False),
        )
    ):
        raise WorkerFailure("candidate_protocol_error: normalized AST/source guard or malicious-candidate rejection is missing")

    traces = report.get("traces")
    if not isinstance(traces, list) or len(traces) != 1:
        raise WorkerFailure("candidate_protocol_error: exactly one Gaze full-game trace is required")
    trace = traces[0]
    if (
        not isinstance(trace, dict)
        or trace.get("id") != "rav-gaze-of-the-gorgon-regeneration-and-block-history"
        or trace.get("passed") is not True
        or trace.get("winner_seat") != 0
        or trace.get("terminal") != "Winner(PlayerId(0))"
        or trace.get("decks") != ["rav_gaze_of_the_gorgon_pilot", "rav_gaze_of_the_gorgon_opponent"]
        or trace.get("policies") != ["rav.gaze-of-the-gorgon-combat-pilot.v1", "rav.gaze-of-the-gorgon-blocker.v1"]
        or trace.get("rejected_moves") != 0
        or trace.get("gaze_mana_cost") != "{3}{B/G}"
        or trace.get("gaze_zone_after_game") != "Some(Graveyard)"
        or trace.get("target_zone_after_game") != "Some(Battlefield)"
        or trace.get("blocker_zone_after_game") != "Some(Graveyard)"
    ):
        raise WorkerFailure("candidate_protocol_error: Gaze target, regeneration, blocker removal, or terminal game differs")
    if not _int(trace.get("seed")) or not 0 <= trace["seed"] <= 512 or not _int(trace.get("turns")) or trace["turns"] <= 0 or not _int(trace.get("accepted_moves")) or trace["accepted_moves"] <= 0:
        raise WorkerFailure("candidate_protocol_error: seeded full-game trace metadata is malformed")
    objects = [trace.get(key) for key in ("gaze_object", "target_object", "blocker_object", "opponent_watchwolf_object", "pilot_watchwolf_object")]
    if any(not _int(object_id) or object_id <= 0 for object_id in objects):
        raise WorkerFailure("candidate_protocol_error: exact Gaze, attacker, blocker, and terminal creature identities are missing")
    if trace["target_object"] == trace["blocker_object"] or trace["blocker_object"] != trace["opponent_watchwolf_object"]:
        raise WorkerFailure("candidate_protocol_error: block-history participant is not the exact opposing Watchwolf")
    payment = trace.get("gaze_payment")
    if not isinstance(payment, dict) or len(payment.get("colors", [])) != 4 or len(payment.get("generic", [])) != 3 or len(payment.get("hybrid", [])) != 1 or payment["hybrid"][0] not in {"Black", "Green"}:
        raise WorkerFailure("candidate_protocol_error: actual three-generic plus one legal black-or-green hybrid payment is required")
    if sorted(payment["colors"]) != sorted(payment["generic"] + payment["hybrid"]):
        raise WorkerFailure("candidate_protocol_error: hybrid payment detail differs from the engine payment event")
    life = trace.get("life_at_match_end")
    if not isinstance(life, list) or len(life) != 2 or not all(_int(value) for value in life) or life[0] <= 0 or life[1] > 0:
        raise WorkerFailure("candidate_protocol_error: full-game terminal life totals are invalid")

    events = trace.get("events")
    positions = trace.get("ordered_event_sequence")
    expected_kinds = [
        "SpellManaPaid", "SpellCast", "AttackersDeclared", "BlockersDeclared",
        "SpellManaPaid", "SpellCast", "SpellResolved", "RegenerationShieldCreated",
        "DelayedCombatDestructionScheduled", "DamageDealtToPermanent", "RegenerationShieldUsed",
        "DelayedCombatDestructionStacked", "CardDestroyed", "CardMoved", "AbilityResolved",
        "AttackersDeclared", "DamageDealtToPlayer", "GameEnded",
    ]
    if not isinstance(events, list) or not isinstance(positions, list) or len(positions) != len(expected_kinds):
        raise WorkerFailure("candidate_protocol_error: ordered typed game event sequence is missing")
    if any(not isinstance(event, dict) or not _int(event.get("sequence")) or not isinstance(event.get("canonical"), str) for event in events):
        raise WorkerFailure("candidate_protocol_error: projected event list is malformed")
    if [event["sequence"] for event in events] != list(range(len(events))):
        raise WorkerFailure("candidate_protocol_error: projected event sequence is incomplete or unordered")
    if any(not _int(position) for position in positions) or positions != sorted(positions) or any(position >= len(events) for position in positions):
        raise WorkerFailure("candidate_protocol_error: referenced trace positions are malformed")
    selected = [events[position] for position in positions]
    if [event["kind"] for event in selected] != expected_kinds or any(event["sequence"] != position for event, position in zip(selected, positions, strict=True)):
        raise WorkerFailure("candidate_protocol_error: expected payment, target combat, regeneration, delayed action, terminal event order differs")
    gaze, target, blocker, pilot_wolf = objects[0], objects[1], objects[2], objects[4]
    canonical = [event["canonical"] for event in selected]
    constraints = (
        f"ObjectId({objects[3]})" in canonical[0] and "PlayerId(1)" in canonical[0] and "Green" in canonical[0] and "White" in canonical[0],
        f"ObjectId({target})" in canonical[2] and "PlayerId(0)" in canonical[2],
        f"ObjectId({target})" in canonical[3] and f"ObjectId({blocker})" in canonical[3] and "PlayerId(1)" in canonical[3],
        f"ObjectId({gaze})" in canonical[4] and "PlayerId(0)" in canonical[4],
        f"ObjectId({gaze})" in canonical[5] and "PlayerId(0)" in canonical[5],
        f"source: ObjectId({gaze})" in canonical[7] and f"target: ObjectId({target})" in canonical[7],
        f"source: ObjectId({gaze})" in canonical[8] and f"target: ObjectId({target})" in canonical[8],
        f"source: ObjectId({blocker})" in canonical[9] and f"permanent: ObjectId({target})" in canonical[9] and "amount: 3" in canonical[9],
        f"source: ObjectId({gaze})" in canonical[10] and f"target: ObjectId({target})" in canonical[10],
        f"source: ObjectId({gaze})" in canonical[11] and f"target: ObjectId({target})" in canonical[11] and f"permanent: ObjectId({blocker})" in canonical[11],
        f"source: ObjectId({gaze})" in canonical[12] and f"card: ObjectId({blocker})" in canonical[12],
        f"ObjectId({blocker})" in canonical[13] and "to: Graveyard" in canonical[13],
        f"source: ObjectId({gaze})" in canonical[14] and "delayed-combat-history-destruction" in canonical[14],
        f"ObjectId({pilot_wolf})" in canonical[15] and "PlayerId(0)" in canonical[15],
        f"ObjectId({pilot_wolf})" in canonical[16] and "PlayerId(1)" in canonical[16] and "amount: 3" in canonical[16],
        "winner: Some(PlayerId(0))" in canonical[17],
    )
    if not all(constraints):
        raise WorkerFailure("candidate_protocol_error: exact source/target/blocker identities or mechanic event facts are not bound")

    mutations = report.get("mutation_sensitivity", {})
    if any(mutations.get(key) is not True for key in (
        "combined_effect_removal_detected", "hybrid_cost_mutation_detected",
        "compile_valid_side_effect_candidate_rejected_before_candidate_calls",
    )) or mutations.get("sealed_mutation_authority") is not False:
        raise WorkerFailure("candidate_protocol_error: source or mechanic mutations are incomplete")


def parse_report(stdout: bytes) -> dict:
    for line in reversed(stdout.decode(errors="replace").splitlines()):
        try:
            value = json.loads(line)
        except json.JSONDecodeError:
            continue
        if isinstance(value, dict) and value.get("schema_version") == "cardbench.magic.card-pvp.check.v1":
            validate_report(value)
            return value
    raise WorkerFailure("candidate_protocol_error: no Gaze of the Gorgon PvP report")


def run_worker(workspace: Path, authority: Path, image: str, candidate_kind: str, timeout: int) -> tuple[int, bytes, bytes]:
    registry = Path.home() / ".cargo" / "registry"
    if not registry.is_dir():
        raise FileNotFoundError(f"offline Cargo registry cache is missing: {registry}")
    if not shutil.which("docker"):
        raise FileNotFoundError("Docker is required for isolated Magic candidate scoring")
    name = f"cardbench-magic-gaze-{uuid.uuid4().hex[:12]}"
    candidate_flag = " --candidate" if candidate_kind == "agent" else ""
    command = [
        "docker", "run", "--rm", "--name", name, "--network", "none", "--read-only",
        "--cap-drop", "ALL", "--security-opt", "no-new-privileges", "--pids-limit", "128",
        "--memory", "3g", "--cpus", "2", "--user", "1000:1000",
        "--tmpfs", "/work:rw,exec,size=2147483648,uid=1000,gid=1000",
        "--tmpfs", "/tmp:rw,noexec,size=67108864,uid=1000,gid=1000",
        "--mount", f"type=bind,src={workspace},dst=/submission,readonly",
        "--mount", f"type=bind,src={registry.resolve()},dst=/usr/local/cargo/registry,readonly",
        "--mount", f"type=bind,src={authority.resolve()},dst=/trusted-authority.rs,readonly",
        "--env", "HOME=/tmp/home", "--env", "CARGO_HOME=/usr/local/cargo", "--env", "RUSTUP_HOME=/usr/local/rustup",
        "--env", "CARGO_NET_OFFLINE=true", "--env", "CARGO_BUILD_JOBS=2", "--env", "RAYON_NUM_THREADS=2",
        image, "sh", "-c",
        "cp -R /submission/. /work/ && cp /trusted-authority.rs /work/varieties/magic/session/src/bin/rav_gaze_of_the_gorgon_pvp.rs && cd /work && cargo run --offline --locked -j 2 --manifest-path varieties/magic/Cargo.toml -p cardbench-magic-session --bin rav-gaze-of-the-gorgon-pvp --" + candidate_flag,
    ]
    completed = subprocess.run(command, stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=timeout, check=False)
    return completed.returncode, completed.stdout, completed.stderr


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--workspace", required=True, type=Path)
    parser.add_argument("--manifest", required=True, type=Path)
    parser.add_argument("--output-root", required=True, type=Path)
    parser.add_argument("--candidate-kind", choices=("reference", "agent"), default="agent")
    parser.add_argument("--timeout", type=int, default=900)
    args = parser.parse_args()
    args.output_root.mkdir(parents=True, exist_ok=True)
    try:
        manifest = verify_workspace(args.workspace.resolve(), args.manifest.resolve())
        image = os.environ.get("CARDBENCH_MAGIC_WORKER_IMAGE", DEFAULT_WORKER_IMAGE)
        if not (image.startswith("rust@sha256:") or image.startswith("sha256:")):
            raise ValueError("CARDBENCH_MAGIC_WORKER_IMAGE must be an immutable digest reference")
        with tempfile.TemporaryDirectory(prefix="gaze-of-the-gorgon-candidate-") as temporary:
            candidate = snapshot(args.workspace.resolve(), manifest, Path(temporary) / "workspace")
            authority = (REPO / AUTHORITY).resolve()
            code, stdout, stderr = run_worker(candidate, authority, image, args.candidate_kind, args.timeout)
        (args.output_root / "worker.stdout").write_bytes(stdout)
        (args.output_root / "worker.stderr").write_bytes(stderr)
        if code != 0:
            result = {"schema_version":"cardbench.magic.card-pvp.check.v1","task_id":"cardbench/magic/card","candidate":{"kind":args.candidate_kind,"source_sha256":sha256((args.workspace.resolve()/TARGET).read_bytes())},"passed":False,"pilot_card_passed":False,"cards_passed":0,"cards_total":291,"printings_total":306,"cards_credited":0,"coverage_complete":False,"full_reward":False,"harbor_reward":0.0,"pilot_reward":0.0,"sealed_authority":False,"error":"candidate compile or Gaze full-game trace gate failed","diagnostics":stderr.decode(errors="replace")[-12000:]}
        else:
            result = parse_report(stdout)
            result["candidate"] = {"kind":args.candidate_kind,"card_definition":"RAV-GAZE-OF-THE-GORGON","source_sha256":sha256((args.workspace.resolve()/TARGET).read_bytes()),"materialized_workspace_sha256":manifest["workspace_sha256"]}
            result["candidate_kind"] = args.candidate_kind
            result["starting_workspace_sha256"] = manifest["workspace_sha256"]
            result["sealed_authority"] = False
            result["worker_image"] = image
            result["sandbox"] = {"network":"none","rootfs":"read-only","pids_limit":128}
            result["harbor_reward"] = 0.0
            result["full_reward"] = False
        (args.output_root / "result.json").write_text(json.dumps(result, indent=2)+"\n", encoding="utf-8")
        (args.output_root / "reward.txt").write_text("0.00000000\n", encoding="utf-8")
        return 0 if result.get("passed") else 1
    except Exception as error:
        failure = {"schema_version":"cardbench.magic.card-pvp.check.v1","task_id":"cardbench/magic/card","passed":False,"pilot_card_passed":False,"cards_passed":0,"cards_total":291,"printings_total":306,"cards_credited":0,"coverage_complete":False,"full_reward":False,"harbor_reward":0.0,"pilot_reward":0.0,"sealed_authority":False,"error":f"{type(error).__name__}: {error}"}
        (args.output_root / "result.json").write_text(json.dumps(failure, indent=2)+"\n", encoding="utf-8")
        (args.output_root / "reward.txt").write_text("0.00000000\n", encoding="utf-8")
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
