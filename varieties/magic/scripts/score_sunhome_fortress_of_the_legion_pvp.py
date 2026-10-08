#!/usr/bin/env python3
"""Score a Sunhome submission using a sealed local authority in an offline worker."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path

from score_watchwolf_pvp import (
    WorkerFailure,
    make_scoring_snapshot,
    run_bounded,
)
from materialize_sunhome_fortress_of_the_legion_task import verify

REPO = Path(__file__).resolve().parents[3]
AUTHORITY_SOURCE = Path("varieties/magic/session/src/bin/rav_sunhome_fortress_of_the_legion_pvp.rs")
TARGET = Path("varieties/magic/sets/ravnica_city_of_guilds/src/cards/sunhome_fortress_of_the_legion.rs")
MATERIALIZER = Path(__file__).with_name("materialize_sunhome_fortress_of_the_legion_task.py")
DEFAULT_WORKER_IMAGE = "rust@sha256:d7482085ff5b415f84dba5647ae71606650bdef00db7aeb69f4b3d170c3e4082"


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def validate_report(report: dict) -> None:
    if (
        report.get("task_id") != "cardbench/magic/card"
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
        or report.get("card") != {"collector_number": 282, "definition_id": "RAV-SUNHOME-FORTRESS", "name": "Sunhome, Fortress of the Legion"}
    ):
        raise WorkerFailure("candidate_protocol_error: incomplete Sunhome single-card report")
    trace_list = report.get("traces")
    if not isinstance(trace_list, list) or len(trace_list) != 1:
        raise WorkerFailure("candidate_protocol_error: one Sunhome trace is required")
    trace = trace_list[0]
    if (
        not isinstance(trace, dict)
        or trace.get("id") != "rav-sunhome-double-strike-targeted-activation"
        or trace.get("passed") is not True
        or trace.get("terminal") != "Winner(PlayerId(0))"
        or trace.get("winner_seat") != 0
        or trace.get("rejected_moves") != 0
        or trace.get("sunhome_colorless_mana") != {"ability": "produce-colorless", "color": "Colorless", "amount": 1, "source_tapped": True}
        or trace.get("target_cast_payment") != {"required": {"generic_symbols": 1, "red_symbols": 1, "white_symbols": 1, "total_symbols": 3}, "spent": {"colorless": 1, "red": 1, "white": 1}}
        or trace.get("activated_ability_payment") != {"required": "{2}{R}{W}", "source_tapped": True}
        or trace.get("target_gained_double_strike") is not True
        or trace.get("target_had_double_strike_at_attack") is not True
        or trace.get("double_strike_damage_packets") != 2
    ):
        raise WorkerFailure("candidate_protocol_error: Sunhome mana, tap payment, target, or Double Strike trace differs")
    mutation = report.get("mutation_sensitivity")
    required = ("colorless_output_mutation_detected", "activated_cost_mutation_detected", "tap_cost_mutation_detected", "double_strike_effect_mutation_detected")
    if not isinstance(mutation, dict) or any(mutation.get(key) is not True for key in required):
        raise WorkerFailure("candidate_protocol_error: Sunhome semantic mutation controls are incomplete")
    events = trace.get("events")
    if not isinstance(events, list) or not all(isinstance(event, dict) for event in events):
        raise WorkerFailure("candidate_protocol_error: Sunhome event trace is malformed")
    sequences = [event.get("sequence") for event in events]
    if any(not isinstance(value, int) for value in sequences) or sequences != sorted(sequences):
        raise WorkerFailure("candidate_protocol_error: Sunhome event sequence is malformed")
    sunhome = trace.get("sunhome_object")
    target = trace.get("target_creature_object")
    if not isinstance(sunhome, int) or not isinstance(target, int) or sunhome == target:
        raise WorkerFailure("candidate_protocol_error: Sunhome and targeted creature identities are invalid")
    kinds = [event.get("kind") for event in events]
    ability_resolved = next((i for i, event in enumerate(events) if event.get("kind") == "AbilityResolved" and event.get("facts", {}).get("objects") == [sunhome]), None)
    if ability_resolved is None:
        raise WorkerFailure("candidate_protocol_error: resolved Sunhome activated ability receipt missing")
    attacks = [i for i, event in enumerate(events) if i > ability_resolved and event.get("kind") == "AttackersDeclared" and target in event.get("facts", {}).get("objects", [])]
    if not attacks:
        raise WorkerFailure("candidate_protocol_error: targeted creature did not attack after ability resolution")
    damage_indices = [i for i, event in enumerate(events) if i > attacks[0] and event.get("kind") == "DamageDealtToPlayer" and event.get("facts", {}).get("objects") == [target]]
    damage = [events[i] for i in damage_indices]
    if len(damage) < 2:
        raise WorkerFailure("candidate_protocol_error: both Double Strike damage packets are required")
    terminal = next((i for i, event in enumerate(events) if event.get("kind") == "GameEnded" and event.get("facts", {}).get("seats") == [0]), None)
    if terminal is None or damage_indices[1] >= terminal:
        raise WorkerFailure("candidate_protocol_error: clean terminal winner event missing")
    if not any(event.get("kind") == "AbilityManaPaid" and event.get("facts", {}).get("objects") == [sunhome] for event in events):
        raise WorkerFailure("candidate_protocol_error: activated ability payment receipt missing")


def parse_report(stdout: bytes) -> dict:
    for line in reversed(stdout.decode(errors="replace").splitlines()):
        try:
            value = json.loads(line)
        except json.JSONDecodeError:
            continue
        if value.get("schema_version") == "cardbench.magic.card-pvp.check.v1":
            validate_report(value)
            return value
    raise WorkerFailure("candidate_protocol_error: no valid Sunhome PvP result")


def sandbox_command(workspace: Path, registry: Path, authority: Path, name: str, image: str, candidate_kind: str) -> list[str]:
    if authority.is_symlink() or not authority.is_file():
        raise WorkerFailure("trusted Sunhome authority source is missing")
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
        "cp /trusted-authority.rs /work/varieties/magic/session/src/bin/rav_sunhome_fortress_of_the_legion_pvp.rs && "
        "cd /work && cargo run --offline --locked -j 2 --manifest-path varieties/magic/Cargo.toml "
        "-p cardbench-magic-session --bin rav-sunhome-fortress-of-the-legion-pvp --" + candidate_flag,
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
        raise FileNotFoundError(f"trusted Sunhome authority source is missing: {authority}")
    import shutil
    import uuid
    if not shutil.which("docker"):
        raise FileNotFoundError("Docker is required for Magic candidate isolation")
    verify(workspace, manifest)
    scoring_workspace = make_scoring_snapshot(workspace, manifest_data["files"], output_root / "candidate-workspace")
    candidate_hash = sha256((scoring_workspace / TARGET).read_bytes())
    name = f"cardbench-magic-sunhome-{uuid.uuid4().hex[:16]}"
    command = sandbox_command(scoring_workspace, registry.resolve(), authority, name, image, candidate_kind)
    code, stdout, stderr = run_bounded(command, timeout=900, name=name)
    (output_root / "worker.stdout").write_bytes(stdout)
    (output_root / "worker.stderr").write_bytes(stderr)
    if code != 0:
        report = {"schema_version":"cardbench.magic.card-pvp.check.v1","task_id":"cardbench/magic/card","passed":False,"pilot_card_passed":False,"cards_passed":0,"cards_total":291,"printings_total":306,"cards_credited":0,"coverage_complete":False,"full_reward":False,"harbor_reward":0.0,"pilot_reward":0.0,"error":"candidate compile or Sunhome reference trace failed","diagnostics":stderr.decode(errors="replace")[-12000:]}
    else:
        report = parse_report(stdout)
        report["candidate"] = {"kind":candidate_kind,"card_definition":"RAV-SUNHOME-FORTRESS","source_sha256":candidate_hash,"materialized_workspace_sha256":manifest_data["workspace_sha256"]}
        report["candidate_kind"] = candidate_kind
        report["starting_workspace_sha256"] = manifest_data["workspace_sha256"]
        report["sealed_authority"] = False
        report["worker_image"] = image
        report["sandbox"] = {"network":"none","rootfs":"read-only","pids_limit":128}
        report["harbor_reward"] = 0.0
        report["pilot_reward"] = 0.0
        report["full_reward"] = False
    (output_root / "result.json").write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    (output_root / "reward.txt").write_text("0.00000000\n", encoding="utf-8")
    print(f"candidate_passed={report.get('pilot_card_passed', False)} cards={report.get('cards_passed',0)}/291 coverage_complete=false")
    return 0 if report.get("passed") is True and report.get("pilot_card_passed") is True else 1


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--workspace", required=True, type=Path)
    parser.add_argument("--manifest", required=True, type=Path)
    parser.add_argument("--output-root", required=True, type=Path)
    parser.add_argument("--candidate-kind", choices=["agent", "reference"], default="agent")
    args = parser.parse_args()
    try:
        return score(args.workspace.resolve(), args.manifest.resolve(), args.output_root.resolve(), args.candidate_kind)
    except Exception as error:
        args.output_root.mkdir(parents=True, exist_ok=True)
        report = {"schema_version":"cardbench.magic.card-pvp.check.v1","task_id":"cardbench/magic/card","passed":False,"pilot_card_passed":False,"cards_passed":0,"cards_total":291,"printings_total":306,"cards_credited":0,"coverage_complete":False,"full_reward":False,"harbor_reward":0.0,"pilot_reward":0.0,"error":str(error)}
        (args.output_root / "result.json").write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
        (args.output_root / "reward.txt").write_text("0.00000000\n", encoding="utf-8")
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
