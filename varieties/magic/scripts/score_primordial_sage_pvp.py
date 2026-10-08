#!/usr/bin/env python3
"""Score the isolated Primordial Sage optional-draw PvP authority."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import shutil
import uuid
from pathlib import Path

from score_watchwolf_pvp import WorkerFailure, make_scoring_snapshot, run_bounded
from materialize_primordial_sage_task import verify

REPO = Path(__file__).resolve().parents[3]
AUTHORITY_SOURCE = Path("varieties/magic/session/src/bin/rav_primordial_sage_pvp.rs")
TARGET = Path("varieties/magic/sets/ravnica_city_of_guilds/src/cards/primordial_sage.rs")
DEFAULT_WORKER_IMAGE = "rust@sha256:d7482085ff5b415f84dba5647ae71606650bdef00db7aeb69f4b3d170c3e4082"


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def validate_report(report: dict) -> None:
    expected_card = {
        "collector_number": 177,
        "printings": 1,
        "definition_id": "RAV-PRIMORDIAL-SAGE",
        "name": "Primordial Sage",
    }
    if (
        report.get("schema_version") != "cardbench.magic.card-pvp.check.v1"
        or report.get("task_id") != "cardbench/magic/card"
        or report.get("passed") is not True
        or report.get("pilot_card_passed") is not True
        or report.get("replay_deterministic") is not True
        or report.get("cards_passed") != 1
        or report.get("cards_credited") != 0
        or report.get("cards_total") != 291
        or report.get("printings_total") != 306
        or report.get("coverage_complete") is not False
        or report.get("full_reward") is not False
        or report.get("harbor_reward") != 0.0
        or report.get("pilot_reward") != 0.0
        or report.get("viewer_scope") != "Judge"
        or report.get("card") != expected_card
    ):
        raise WorkerFailure("candidate_protocol_error: incomplete Primordial Sage single-card result")
    candidate = report.get("candidate")
    if not isinstance(candidate, dict) or candidate.get("card_definition") != "RAV-PRIMORDIAL-SAGE":
        raise WorkerFailure("candidate_protocol_error: wrong Primordial Sage candidate identity")
    traces = report.get("traces")
    if not isinstance(traces, list) or len(traces) != 2:
        raise WorkerFailure("candidate_protocol_error: accepted and declined Sage traces are both required")
    by_choice: dict[str, dict] = {}
    for trace in traces:
        if not isinstance(trace, dict) or trace.get("passed") is not True:
            raise WorkerFailure("candidate_protocol_error: a Primordial Sage trace failed")
        mechanics = trace.get("trigger")
        sage = trace.get("sage")
        choice = mechanics.get("draw_choice") if isinstance(mechanics, dict) else None
        if choice not in {"accept", "decline"} or choice in by_choice:
            raise WorkerFailure("candidate_protocol_error: duplicate or unknown Primordial Sage choice branch")
        by_choice[choice] = trace
        expected_trace_id = (
            "rav-primordial-sage-creature-cast-optional-draw-accepted"
            if choice == "accept"
            else "rav-primordial-sage-creature-cast-optional-draw-declined"
        )
        if (
            trace.get("trace_id") != expected_trace_id
            or trace.get("terminal") != "Winner(PlayerId(0))"
            or trace.get("winner_seat") != 0
            or trace.get("rejected_moves") != 0
            or not isinstance(trace.get("turns"), int)
            or trace["turns"] <= 0
            or not isinstance(sage, dict)
            or sage.get("mana_cost") != "{4}{G}{G}"
            or sage.get("mana_payment") != ["Green"] * 6
            or sage.get("power") != 4
            or sage.get("toughness") != 5
            or sage.get("live_characteristics_valid") is not True
            or not isinstance(mechanics, dict)
            or mechanics.get("condition") != "CastsCreatureSpell"
            or mechanics.get("source") != sage.get("object")
            or mechanics.get("controller_creature_spells", 0) < 1
            or mechanics.get("opponent_creature_spells", 0) < 1
            or mechanics.get("trigger_count") != mechanics.get("controller_creature_spells")
            or mechanics.get("trigger_count") != mechanics.get("accepted", 0) + mechanics.get("declined", 0)
            or mechanics.get("choice_card_zones_valid") is not True
        ):
            raise WorkerFailure("candidate_protocol_error: Sage characteristics, controller scope, or terminal trace differs")
        if choice == "accept" and (mechanics.get("accepted", 0) < 1 or mechanics.get("declined") != 0):
            raise WorkerFailure("candidate_protocol_error: Sage accept branch did not accept every optional draw")
        if choice == "decline" and (mechanics.get("declined", 0) < 1 or mechanics.get("accepted") != 0):
            raise WorkerFailure("candidate_protocol_error: Sage decline branch did not decline every optional draw")
        sequences = trace.get("ordered_trigger_sequences")
        events = trace.get("events")
        if not isinstance(sequences, list) or len(sequences) != mechanics.get("trigger_count") or not isinstance(events, list):
            raise WorkerFailure("candidate_protocol_error: Sage trigger event sequences are incomplete")
        by_sequence = {event.get("sequence"): event for event in events if isinstance(event, dict)}
        for sequence in sequences:
            if not isinstance(sequence, dict):
                raise WorkerFailure("candidate_protocol_error: malformed Sage trigger sequence")
            cast, stacked, choice_action, draw_move, resolved, creature_resolved = (
                sequence.get("creature_cast"),
                sequence.get("trigger_stacked"),
                sequence.get("choice_action"),
                sequence.get("draw_move"),
                sequence.get("trigger_resolved"),
                sequence.get("creature_resolved"),
            )
            if not all(isinstance(index, int) for index in (cast, stacked, choice_action, resolved, creature_resolved)) or not cast < stacked < choice_action < resolved < creature_resolved:
                raise WorkerFailure("candidate_protocol_error: Sage trigger did not resolve above its creature spell")
            if (
                by_sequence.get(cast, {}).get("kind") != "SpellCast"
                or by_sequence.get(stacked, {}).get("kind") != "TriggeredAbilityStacked"
                or by_sequence.get(resolved, {}).get("kind") != "AbilityResolved"
                or by_sequence.get(creature_resolved, {}).get("kind") != "SpellResolved"
            ):
                raise WorkerFailure("candidate_protocol_error: Sage cast/trigger/resolve receipts differ")
            if choice == "accept":
                if not isinstance(draw_move, int) or not choice_action < draw_move < resolved or by_sequence.get(draw_move, {}).get("kind") != "CardMoved" or by_sequence.get(draw_move, {}).get("facts", {}).get("zone") != "Hand":
                    raise WorkerFailure("candidate_protocol_error: accepted Sage trigger did not move the drawn card to hand")
            elif draw_move is not None:
                raise WorkerFailure("candidate_protocol_error: declined Sage trigger moved a card")
    if set(by_choice) != {"accept", "decline"}:
        raise WorkerFailure("candidate_protocol_error: a Primordial Sage choice branch is missing")
    mutations = report.get("mutation_sensitivity")
    required = (
        "wrong_mana_cost_detected",
        "wrong_power_detected",
        "wrong_toughness_detected",
        "trigger_removal_detected",
        "draw_effect_removal_detected",
        "mandatory_trigger_detected",
        "wrong_trigger_condition_detected",
    )
    if not isinstance(mutations, dict) or any(mutations.get(key) is not True for key in required):
        raise WorkerFailure("candidate_protocol_error: Primordial Sage semantic mutations escaped")


def parse_report(stdout: bytes) -> dict:
    for line in reversed(stdout.decode(errors="replace").splitlines()):
        try:
            value = json.loads(line)
        except json.JSONDecodeError:
            continue
        if isinstance(value, dict) and value.get("schema_version") == "cardbench.magic.card-pvp.check.v1":
            validate_report(value)
            return value
    raise WorkerFailure("candidate_protocol_error: no valid Primordial Sage PvP report")


def sandbox_command(workspace: Path, registry: Path, authority: Path, name: str, image: str, candidate_kind: str) -> list[str]:
    if authority.is_symlink() or not authority.is_file():
        raise WorkerFailure("trusted Primordial Sage authority is missing")
    candidate_flag = " --candidate" if candidate_kind == "agent" else ""
    return [
        "docker", "run", "--rm", "--name", name, "--network", "none", "--read-only",
        "--cap-drop", "ALL", "--security-opt", "no-new-privileges", "--pids-limit", "128",
        "--memory", "3g", "--cpus", "2", "--user", "1000:1000",
        "--tmpfs", "/work:rw,exec,size=2147483648,uid=1000,gid=1000",
        "--tmpfs", "/tmp:rw,noexec,size=67108864,uid=1000,gid=1000",
        "--mount", f"type=bind,src={workspace},dst=/submission,readonly",
        "--mount", f"type=bind,src={registry},dst=/usr/local/cargo/registry,readonly",
        "--mount", f"type=bind,src={authority.resolve()},dst=/trusted-authority.rs,readonly",
        "--env", "HOME=/tmp/home", "--env", "CARGO_HOME=/usr/local/cargo",
        "--env", "RUSTUP_HOME=/usr/local/rustup", "--env", "CARGO_NET_OFFLINE=true",
        "--env", "CARGO_BUILD_JOBS=2", "--env", "RAYON_NUM_THREADS=2", image, "sh", "-c",
        "cp -R /submission/. /work/ && "
        "cp /trusted-authority.rs /work/varieties/magic/session/src/bin/rav_primordial_sage_pvp.rs && "
        "cd /work && cargo run --offline --locked -j 2 --manifest-path varieties/magic/Cargo.toml "
        "-p cardbench-magic-session --bin rav-primordial-sage-pvp --" + candidate_flag,
    ]


def score(workspace: Path, manifest: Path, output_root: Path, candidate_kind: str) -> int:
    output_root.mkdir(parents=True, exist_ok=True)
    manifest_data = json.loads(manifest.read_text(encoding="utf-8"))
    image = os.environ.get("CARDBENCH_MAGIC_WORKER_IMAGE", DEFAULT_WORKER_IMAGE)
    if not (image.startswith("rust@sha256:") or image.startswith("sha256:")):
        raise ValueError("CARDBENCH_MAGIC_WORKER_IMAGE must be an immutable digest reference")
    registry = Path.home() / ".cargo" / "registry"
    if not registry.is_dir():
        raise FileNotFoundError(f"offline Cargo registry cache is missing: {registry}")
    authority = REPO / AUTHORITY_SOURCE
    if authority.is_symlink() or not authority.is_file():
        raise FileNotFoundError(f"trusted Primordial Sage authority is missing: {authority}")
    if not shutil.which("docker"):
        raise FileNotFoundError("Docker is required for Magic candidate isolation")
    verify(workspace, manifest)
    snapshot = make_scoring_snapshot(workspace, manifest_data["files"], output_root / "candidate-workspace")
    candidate_hash = sha256((snapshot / TARGET).read_bytes())
    name = f"cardbench-magic-primordial-sage-{uuid.uuid4().hex[:16]}"
    command = sandbox_command(snapshot, registry.resolve(), authority, name, image, candidate_kind)
    code, stdout, stderr = run_bounded(command, timeout=900, name=name)
    (output_root / "worker.stdout").write_bytes(stdout)
    (output_root / "worker.stderr").write_bytes(stderr)
    if code != 0:
        report = {"schema_version": "cardbench.magic.card-pvp.check.v1", "task_id": "cardbench/magic/card", "passed": False, "pilot_card_passed": False, "cards_passed": 0, "cards_total": 291, "printings_total": 306, "cards_credited": 0, "coverage_complete": False, "full_reward": False, "harbor_reward": 0.0, "pilot_reward": 0.0, "error": "candidate compile or Primordial Sage trusted trace failed", "diagnostics": stderr.decode(errors="replace")[-12000:]}
    else:
        report = parse_report(stdout)
        report["candidate"] = {"kind": candidate_kind, "card_definition": "RAV-PRIMORDIAL-SAGE", "source_sha256": candidate_hash, "materialized_workspace_sha256": manifest_data["workspace_sha256"]}
        report["candidate_kind"] = candidate_kind
        report["starting_workspace_sha256"] = manifest_data["workspace_sha256"]
        report["sealed_authority"] = False
        report["worker_image"] = image
        report["sandbox"] = {"network": "none", "rootfs": "read-only", "pids_limit": 128}
        report["harbor_reward"] = 0.0
        report["pilot_reward"] = 0.0
        report["full_reward"] = False
    (output_root / "result.json").write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    (output_root / "reward.txt").write_text("0.00000000\n", encoding="utf-8")
    print(f"candidate_passed={report.get('pilot_card_passed', False)} cards={report.get('cards_passed', 0)}/291 coverage_complete=false")
    return 0 if report.get("passed") is True and report.get("pilot_card_passed") is True else 1


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--workspace", required=True, type=Path)
    parser.add_argument("--manifest", required=True, type=Path)
    parser.add_argument("--output-root", required=True, type=Path)
    parser.add_argument("--candidate-kind", choices=["reference", "agent"], default="agent")
    args = parser.parse_args()
    return score(args.workspace.resolve(), args.manifest.resolve(), args.output_root.resolve(), args.candidate_kind)


if __name__ == "__main__":
    raise SystemExit(main())
