#!/usr/bin/env python3
"""Score Dimir Infiltrator in an isolated, offline PvP worker."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import shutil
import uuid
from pathlib import Path

from score_watchwolf_pvp import WorkerFailure, make_scoring_snapshot, run_bounded
from materialize_dimir_infiltrator_task import verify

REPO = Path(__file__).resolve().parents[3]
AUTHORITY_SOURCE = Path("varieties/magic/session/src/bin/rav_dimir_infiltrator_pvp.rs")
TARGET = Path("varieties/magic/sets/ravnica_city_of_guilds/src/cards/dimir_infiltrator.rs")
DEFAULT_WORKER_IMAGE = "rust@sha256:d7482085ff5b415f84dba5647ae71606650bdef00db7aeb69f4b3d170c3e4082"


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def validate_report(report: dict) -> None:
    expected_card = {
        "collector_number": 203,
        "printings": 1,
        "definition_id": "RAV-DIMIR-INFILTRATOR",
        "name": "Dimir Infiltrator",
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
        raise WorkerFailure("candidate_protocol_error: incomplete Dimir Infiltrator single-card result")
    candidate = report.get("candidate")
    if not isinstance(candidate, dict) or candidate.get("card_definition") != "RAV-DIMIR-INFILTRATOR":
        raise WorkerFailure("candidate_protocol_error: wrong Dimir Infiltrator candidate identity")
    traces = report.get("traces")
    if not isinstance(traces, list) or len(traces) != 1:
        raise WorkerFailure("candidate_protocol_error: exactly one Dimir Infiltrator trace is required")
    trace = traces[0]
    if (
        not isinstance(trace, dict)
        or trace.get("trace_id") != "rav-dimir-infiltrator-transmute-unblockable-full-game"
        or trace.get("passed") is not True
        or trace.get("terminal") != "Winner(PlayerId(0))"
        or trace.get("winner_seat") != 0
        or trace.get("rejected_moves") != 0
        or not isinstance(trace.get("seed"), int)
        or not isinstance(trace.get("turns"), int)
        or trace["turns"] <= 0
    ):
        raise WorkerFailure("candidate_protocol_error: Dimir Infiltrator duel was not a clean terminal PvP game")
    transmute = trace.get("transmute")
    combat = trace.get("combat")
    if (
        not isinstance(transmute, dict)
        or not isinstance(combat, dict)
        or transmute.get("cost") != "{1}{U}{B}"
        or transmute.get("mana_value") != 2
        or transmute.get("selected_definition") != "RAV-DIMIR-SIGNET"
        or transmute.get("selected_mana_value") != 2
        or transmute.get("discarded_to_graveyard") is not True
        or transmute.get("revealed") is not True
        or transmute.get("put_into_hand") is not True
        or transmute.get("private_to_controller") is not True
        or transmute.get("library_shuffled") is not True
        or combat.get("infiltrators_cast", 0) < 2
        or combat.get("live_unblockable_infiltrators", 0) < 1
        or combat.get("available_carven_caryatid") is not True
        or combat.get("block_assignments_to_infiltrator") != 0
        or combat.get("combat_damage_to_opponent", 0) < 20
    ):
        raise WorkerFailure("candidate_protocol_error: Transmute search or unblockable combat facts differ")
    source, selected = transmute.get("source"), transmute.get("selected")
    if not isinstance(source, int) or not isinstance(selected, int) or source == selected:
        raise WorkerFailure("candidate_protocol_error: Transmute did not identify distinct source and result objects")
    order = trace.get("ordered_event_sequence")
    events = trace.get("events")
    expected_kinds = [
        "AbilityManaPaid", "DiscardedAsAbilityCost", "CardMoved", "AbilityActivated",
        "DecisionOpened", "DecisionCompleted", "CardRevealed", "CardMoved",
        "LibrarySearchResolved", "LibraryShuffled", "Transmuted", "AbilityResolved",
    ]
    if (
        not isinstance(order, list)
        or len(order) != len(expected_kinds)
        or any(not isinstance(index, int) for index in order)
        or order != sorted(set(order))
        or not isinstance(events, list)
        or len(events) != trace.get("event_count")
    ):
        raise WorkerFailure("candidate_protocol_error: Dimir Infiltrator event sequence is incomplete")
    by_sequence = {event.get("sequence"): event for event in events if isinstance(event, dict)}
    if len(by_sequence) != len(events):
        raise WorkerFailure("candidate_protocol_error: Dimir Infiltrator projected event sequence is malformed")
    selected_events = [by_sequence.get(index) for index in order]
    if [event.get("kind") if isinstance(event, dict) else None for event in selected_events] != expected_kinds:
        raise WorkerFailure("candidate_protocol_error: Transmute cost, search, and resolution event order differs")
    paid, discard, source_move, activated, opened, completed, reveal, result_move, search, shuffle, transmuted, resolved = selected_events
    facts = [event.get("facts", {}) for event in selected_events]
    if (
        facts[0].get("objects") != [source]
        or facts[0].get("seats") != [0]
        or facts[0].get("ability") != "transmute"
        or facts[0].get("mana_cost_generic") != 1
        or facts[0].get("mana_cost_colors") != ["Blue", "Black"]
        or f"source: ObjectId({source})" not in discard.get("canonical", "")
        or f"card: ObjectId({source})" not in discard.get("canonical", "")
        or facts[2].get("objects") != [source]
        or facts[2].get("zone") != "Graveyard"
        or facts[3].get("objects") != [source]
        or facts[3].get("seats") != [0]
        or facts[3].get("ability") != "transmute"
    ):
        raise WorkerFailure("candidate_protocol_error: exact Transmute source, payment, or discard evidence differs")
    if (
        "kind: LibrarySearch" not in opened.get("canonical", "")
        or "visibility: Private" not in opened.get("canonical", "")
        or "kind: LibrarySearch" not in completed.get("canonical", "")
        or f"card: ObjectId({selected})" not in reveal.get("canonical", "")
        or "RAV-DIMIR-SIGNET" not in reveal.get("canonical", "")
        or facts[7].get("objects") != [selected]
        or facts[7].get("zone") != "Hand"
        or f"source: ObjectId({source})" not in search.get("canonical", "")
        or f"found: Some(ObjectId({selected}))" not in search.get("canonical", "")
        or facts[11].get("objects") != [source]
        or facts[11].get("ability") != "transmute"
    ):
        raise WorkerFailure("candidate_protocol_error: private Transmute choice, selected card, or resolution differs")
    mutations = report.get("mutation_sensitivity")
    required = (
        "wrong_mana_cost_detected",
        "wrong_power_detected",
        "wrong_toughness_detected",
        "unblockable_removed_detected",
        "transmute_removed_detected",
        "transmute_cost_changed_detected",
    )
    if not isinstance(mutations, dict) or any(mutations.get(key) is not True for key in required):
        raise WorkerFailure("candidate_protocol_error: Dimir Infiltrator semantic mutations escaped")


def parse_report(stdout: bytes) -> dict:
    for line in reversed(stdout.decode(errors="replace").splitlines()):
        try:
            value = json.loads(line)
        except json.JSONDecodeError:
            continue
        if isinstance(value, dict) and value.get("schema_version") == "cardbench.magic.card-pvp.check.v1":
            validate_report(value)
            return value
    raise WorkerFailure("candidate_protocol_error: no valid Dimir Infiltrator PvP report")


def sandbox_command(workspace: Path, registry: Path, authority: Path, name: str, image: str, candidate_kind: str) -> list[str]:
    if authority.is_symlink() or not authority.is_file():
        raise WorkerFailure("trusted Dimir Infiltrator authority is missing")
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
        "cp /trusted-authority.rs /work/varieties/magic/session/src/bin/rav_dimir_infiltrator_pvp.rs && "
        "cd /work && cargo run --offline --locked -j 2 --manifest-path varieties/magic/Cargo.toml "
        "-p cardbench-magic-session --bin rav-dimir-infiltrator-pvp --" + candidate_flag,
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
        raise FileNotFoundError(f"trusted Dimir Infiltrator authority is missing: {authority}")
    if not shutil.which("docker"):
        raise FileNotFoundError("Docker is required for Magic candidate isolation")
    verify(workspace, manifest)
    snapshot = make_scoring_snapshot(workspace, manifest_data["files"], output_root / "candidate-workspace")
    candidate_hash = sha256((snapshot / TARGET).read_bytes())
    name = f"cardbench-magic-dimir-infiltrator-{uuid.uuid4().hex[:16]}"
    command = sandbox_command(snapshot, registry.resolve(), authority, name, image, candidate_kind)
    code, stdout, stderr = run_bounded(command, timeout=900, name=name)
    (output_root / "worker.stdout").write_bytes(stdout)
    (output_root / "worker.stderr").write_bytes(stderr)
    if code != 0:
        report = {"schema_version": "cardbench.magic.card-pvp.check.v1", "task_id": "cardbench/magic/card", "passed": False, "pilot_card_passed": False, "cards_passed": 0, "cards_total": 291, "printings_total": 306, "cards_credited": 0, "coverage_complete": False, "full_reward": False, "harbor_reward": 0.0, "pilot_reward": 0.0, "error": "candidate compile or Dimir Infiltrator trusted trace failed", "diagnostics": stderr.decode(errors="replace")[-12000:]}
    else:
        report = parse_report(stdout)
        report["candidate"] = {"kind": candidate_kind, "card_definition": "RAV-DIMIR-INFILTRATOR", "source_sha256": candidate_hash, "materialized_workspace_sha256": manifest_data["workspace_sha256"]}
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
