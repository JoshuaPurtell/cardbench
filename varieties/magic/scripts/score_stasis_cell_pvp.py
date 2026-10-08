#!/usr/bin/env python3
"""Validate and optionally run the isolated Stasis Cell PvP task."""

from __future__ import annotations

import argparse
import json
import os
import shutil
import subprocess
import uuid
from pathlib import Path

from materialize_stasis_cell_task import verify

REPO = Path(__file__).resolve().parents[3]
TARGET = Path("varieties/magic/sets/ravnica_city_of_guilds/src/cards/stasis_cell.rs")
AUTHORITY = Path("varieties/magic/session/src/bin/rav_stasis_cell_pvp.rs")
BIN = "rav-stasis-cell-pvp"
CARD_ID = "RAV-STASIS-CELL"
CARD_NAME = "Stasis Cell"
DEFAULT_IMAGE = "rust@sha256:d7482085ff5b415f84dba5647ae71606650bdef00db7aeb69f4b3d170c3e4082"


class WorkerFailure(RuntimeError):
    pass


def validate_report(report: dict) -> None:
    if (
        report.get("schema_version") != "cardbench.magic.card-pvp.check.v1"
        or report.get("task_id") != "cardbench/magic/card"
        or report.get("passed") is not True
        or report.get("pilot_card_passed") is not True
        or report.get("deterministic_replay") is not True
        or report.get("cards_passed") != 1
        or report.get("cards_credited") != 0
        or report.get("cards_total") != 291
        or report.get("printings_total") != 306
        or report.get("coverage_complete") is not False
        or report.get("full_reward") is not False
        or report.get("harbor_reward") != 0.0
        or report.get("pilot_reward") != 0.0
        or report.get("viewer_scope") != "Judge"
        or report.get("card") != {"collector_number": 66, "definition_id": CARD_ID, "name": CARD_NAME}
    ):
        raise WorkerFailure("candidate_protocol_error: incomplete Stasis Cell single-card report")
    traces = report.get("traces")
    if not isinstance(traces, list) or len(traces) != 1 or not isinstance(traces[0], dict):
        raise WorkerFailure("candidate_protocol_error: exactly one Stasis Cell full-game trace is required")
    trace = traces[0]
    if (
        trace.get("id") != "rav-stasis-cell-colored-reattachment-combat-restriction-full-game"
        or trace.get("passed") is not True
        or not isinstance(trace.get("seed"), int)
        or trace.get("terminal") != "Winner(PlayerId(0))"
        or trace.get("winner_seat") != 0
        or trace.get("rejected_moves") != 0
        or not isinstance(trace.get("turns"), int)
        or trace["turns"] <= 0
        or not isinstance(trace.get("accepted_moves"), int)
        or trace["accepted_moves"] <= 0
    ):
        raise WorkerFailure("candidate_protocol_error: Stasis Cell did not complete a clean PvP win")
    mechanics = trace.get("mechanics")
    if not isinstance(mechanics, dict):
        raise WorkerFailure("candidate_protocol_error: Stasis Cell mechanics are missing")
    paid_colors = mechanics.get("paid_mana_colors")
    incarnations = mechanics.get("attachment_incarnations")
    observations = mechanics.get("declaration_observations")
    if (
        mechanics.get("definition_id") != CARD_ID
        or mechanics.get("mana_cost") != "{4}{U}"
        or not isinstance(paid_colors, list)
        or len(paid_colors) != 5
        or "Blue" not in paid_colors
        or mechanics.get("first_target_definition") != "RAV-DIMIR-GUILDMAGE"
        or mechanics.get("second_target_definition") != "RAV-WATCHWOLF"
        or mechanics.get("final_attachment_object") != mechanics.get("second_target_object")
        or mechanics.get("reattachment_cost") != "{3}{U}"
        or "with_colors" not in mechanics.get("reattachment_mana_cost_receipt", "")
        or mechanics.get("reattachment_resolved") is not True
        or mechanics.get("nonmana_ability_resolved_while_attached") is not True
        or mechanics.get("first_target_restored_after_reattachment") is not True
        or mechanics.get("second_target_cannot_attack_or_block") is not True
        or not isinstance(incarnations, list)
        or len(incarnations) != 1
        or not all(isinstance(value, int) and value > 0 for value in incarnations[0])
        or not isinstance(observations, list)
    ):
        raise WorkerFailure("candidate_protocol_error: Stasis Cell payment, attachment, restriction, or reattachment differs")
    attack = next((item for item in observations if isinstance(item, dict) and item.get("phase") == "declare-attackers" and item.get("definition") == "RAV-DIMIR-GUILDMAGE"), None)
    block = next((item for item in observations if isinstance(item, dict) and item.get("phase") == "declare-blockers" and item.get("definition") == "RAV-WATCHWOLF"), None)
    if not attack or attack.get("can_attack") is not False or attack.get("has_attack_block_restriction") is not True or attack.get("nonmana_activated_abilities_suppressed") is not False:
        raise WorkerFailure("candidate_protocol_error: enchanted creature attack legality or ability availability is wrong")
    if not block or block.get("can_block") is not False or block.get("has_attack_block_restriction") is not True:
        raise WorkerFailure("candidate_protocol_error: reattached creature can still block")
    order = trace.get("ordered_event_sequence")
    if not isinstance(order, list) or len(order) != 6 or order != sorted(order) or len(set(order)) != len(order):
        raise WorkerFailure("candidate_protocol_error: Stasis Cell event order receipt is invalid")
    events = trace.get("events")
    if not isinstance(events, list) or not all(isinstance(event, dict) for event in events):
        raise WorkerFailure("candidate_protocol_error: Stasis Cell projected events are malformed")
    kinds = [event.get("kind") for event in events]
    if (
        kinds.count("GameEnded") != 1
        or kinds.count("AbilityResolved") < 2
        or kinds.count("AuraAttached") < 1
        or kinds.count("AbilityManaPaid") < 1
    ):
        raise WorkerFailure("candidate_protocol_error: Stasis Cell typed event receipts are missing")
    guard = report.get("candidate_source_validation", {})
    if (
        guard.get("normalized_literal_source_and_ast_checked_before_candidate_calls") is not True
        or guard.get("compile_valid_injected_side_effect_fixture_parsed") is not True
        or guard.get("compile_valid_injected_side_effect_candidate_rejected") is not True
        or guard.get("injected_candidate_calls_before_rejection") != 0
        or guard.get("side_effect_sentinel_absent_after_rejection") is not True
    ):
        raise WorkerFailure("candidate_protocol_error: source guard did not reject injection before candidate evaluation")
    mutations = report.get("mutation_sensitivity", {})
    for key in (
        "attack_block_restriction_removed_detected_in_game",
        "activated_ability_suppression_added_detected_in_game",
        "blue_reattachment_cost_removed_detected_in_game",
        "reattachment_effect_removed_detected_in_game",
    ):
        if not isinstance(mutations, dict) or mutations.get(key) is not True:
            raise WorkerFailure(f"candidate_protocol_error: Stasis Cell mutation escaped: {key}")


def run_worker(workspace: Path, registry: Path, authority: Path, name: str, image: str, candidate_kind: str) -> subprocess.CompletedProcess[bytes]:
    flag = " --candidate" if candidate_kind == "agent" else ""
    command = [
        "docker", "run", "--rm", "--name", name, "--network", "none", "--read-only",
        "--cap-drop", "ALL", "--security-opt", "no-new-privileges", "--pids-limit", "128",
        "--memory", "3g", "--cpus", "2", "--user", "1000:1000",
        "--tmpfs", "/work:rw,exec,size=2147483648,uid=1000,gid=1000",
        "--tmpfs", "/tmp:rw,noexec,size=67108864,uid=1000,gid=1000",
        "--mount", f"type=bind,src={workspace},dst=/submission,readonly",
        "--mount", f"type=bind,src={registry},dst=/usr/local/cargo/registry,readonly",
        "--mount", f"type=bind,src={authority.resolve()},dst=/trusted-authority.rs,readonly",
        "--env", "HOME=/tmp/home", "--env", "CARGO_HOME=/usr/local/cargo", "--env", "RUSTUP_HOME=/usr/local/rustup",
        "--env", "CARGO_NET_OFFLINE=true", "--env", "CARGO_BUILD_JOBS=2", "--env", "RAYON_NUM_THREADS=2",
        image, "sh", "-c",
        "cp -R /submission/. /work/ && cp /trusted-authority.rs /work/varieties/magic/session/src/bin/rav_stasis_cell_pvp.rs && cd /work && cargo run --offline --locked -j 2 --manifest-path varieties/magic/Cargo.toml -p cardbench-magic-session --bin " + BIN + " --" + flag,
    ]
    return subprocess.run(command, capture_output=True, timeout=900, check=False)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--validate-report", type=Path)
    parser.add_argument("--workspace", type=Path)
    parser.add_argument("--manifest", type=Path)
    parser.add_argument("--output-root", type=Path)
    parser.add_argument("--candidate-kind", choices=["agent", "reference"], default="agent")
    parser.add_argument("--image", default=DEFAULT_IMAGE)
    parser.add_argument("--cargo-registry", type=Path, default=Path.home() / ".cargo" / "registry")
    args = parser.parse_args()
    try:
        if args.validate_report:
            validate_report(json.loads(args.validate_report.read_text(encoding="utf-8")))
            return 0
        if not args.workspace or not args.manifest:
            parser.error("--workspace and --manifest are required unless --validate-report is used")
        verify(args.workspace.resolve(), args.manifest.resolve())
        output_root = (args.output_root or Path("/tmp/cardbench-stasis-cell-output")).resolve()
        output_root.mkdir(parents=True, exist_ok=True)
        result = run_worker(
            args.workspace.resolve(), args.cargo_registry.resolve(), REPO / AUTHORITY,
            f"cardbench-stasis-cell-{uuid.uuid4().hex[:12]}", args.image, args.candidate_kind,
        )
        (output_root / "stdout.txt").write_bytes(result.stdout)
        (output_root / "stderr.txt").write_bytes(result.stderr)
        if result.returncode != 0:
            raise WorkerFailure(f"candidate_protocol_error: isolated Stasis Cell worker exited {result.returncode}")
        reports = [line for line in result.stdout.decode(errors="replace").splitlines() if line.startswith("{")]
        if not reports:
            raise WorkerFailure("candidate_protocol_error: Stasis Cell worker produced no JSON report")
        report = json.loads(reports[-1])
        validate_report(report)
        (output_root / "result.json").write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
        (output_root / "reward.txt").write_text("0.00000000\n", encoding="utf-8")
        return 0
    except (OSError, json.JSONDecodeError, ValueError, WorkerFailure) as error:
        print(error)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
