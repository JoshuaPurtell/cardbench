#!/usr/bin/env python3
"""Score the isolated Sadistic Augermage private-choice authority."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import shutil
import uuid
from pathlib import Path

from score_watchwolf_pvp import WorkerFailure, make_scoring_snapshot, run_bounded
from materialize_sadistic_augermage_task import verify

REPO = Path(__file__).resolve().parents[3]
AUTHORITY_SOURCE = Path("varieties/magic/session/src/bin/rav_sadistic_augermage_pvp.rs")
TARGET = Path("varieties/magic/sets/ravnica_city_of_guilds/src/cards/sadistic_augermage.rs")
DEFAULT_WORKER_IMAGE = "rust@sha256:d7482085ff5b415f84dba5647ae71606650bdef00db7aeb69f4b3d170c3e4082"


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def validate_report(report: dict) -> None:
    expected_card = {
        "collector_number": 103,
        "printings": 1,
        "definition_id": "RAV-SADISTIC-AUGERMAGE",
        "name": "Sadistic Augermage",
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
        raise WorkerFailure("candidate_protocol_error: incomplete Sadistic Augermage single-card result")
    candidate = report.get("candidate")
    if not isinstance(candidate, dict) or candidate.get("card_definition") != expected_card["definition_id"]:
        raise WorkerFailure("candidate_protocol_error: wrong Sadistic Augermage candidate identity")
    traces = report.get("traces")
    if not isinstance(traces, list) or len(traces) != 1:
        raise WorkerFailure("candidate_protocol_error: one Sadistic Augermage trigger trace is required")
    trace = traces[0]
    source, trigger, choices = (trace.get(key) for key in ("source", "trigger", "choices"))
    if (
        not isinstance(trace, dict)
        or trace.get("trace_id") != "rav-sadistic-augermage-source-death-private-library-top"
        or trace.get("passed") is not True
        or trace.get("terminal") != "EffectResolved"
        or not isinstance(source, dict)
        or source.get("definition_id") != expected_card["definition_id"]
        or source.get("mana_cost") != "{2}{B}"
        or source.get("power") != 3
        or source.get("toughness") != 1
        or source.get("died") is not True
        or not isinstance(trigger, dict)
        or trigger.get("ability") != "source-dies-each-player-puts-card-on-library-top"
        or trigger.get("condition") != "Dies"
        or trigger.get("mandatory") is not True
        or trigger.get("players_chose_in_apnap_order") != [0, 1]
        or not isinstance(choices, dict)
        or choices.get("private_choice_seats") != [0, 1]
        or choices.get("private_to_each_chooser") != [True, True]
        or choices.get("hidden_from_nonchooser") != [True, True]
        or choices.get("candidate_counts_by_chooser") != [2, 2]
        or choices.get("candidates_belong_to_chooser") != [True, True]
        or choices.get("chosen_cards_remained_in_hand_until_both_choices") is not True
        or choices.get("chosen_card_owners_match_seats") is not True
        or choices.get("chosen_cards_are_each_library_top") is not True
        or choices.get("unchosen_cards_remained_in_hand") is not True
        or choices.get("prior_library_cards_remained_below_new_top") is not True
        or choices.get("discard_receipts") != 0
    ):
        raise WorkerFailure("candidate_protocol_error: identity, choice privacy, ownership, or library-top result differs")
    events = trace.get("events")
    indices = trace.get("ordered_event_indices")
    if not isinstance(events, list) or not isinstance(indices, dict):
        raise WorkerFailure("candidate_protocol_error: Augermage trace events are missing")
    order = [
        indices.get("trigger_stacked"),
        indices.get("p0_private_choice_opened"),
        indices.get("p0_choice_completed"),
        indices.get("p1_private_choice_opened"),
        indices.get("p1_choice_completed"),
        indices.get("p0_card_moved_to_library"),
        indices.get("p1_card_moved_to_library"),
        indices.get("trigger_resolved"),
    ]
    if any(not isinstance(index, int) or index < 0 or index >= len(events) for index in order):
        raise WorkerFailure("candidate_protocol_error: Augermage event-order indexes are invalid")
    if order != sorted(order) or len(set(order)) != len(order):
        raise WorkerFailure("candidate_protocol_error: choices, library moves, and trigger resolution are out of order")
    expected_kinds = [
        "TriggeredAbilityStacked",
        "DecisionOpened",
        "DecisionCompleted",
        "DecisionOpened",
        "DecisionCompleted",
        "CardMoved",
        "CardMoved",
        "AbilityResolved",
    ]
    if [events[index].get("kind") for index in order] != expected_kinds:
        raise WorkerFailure("candidate_protocol_error: Augermage's ordered event receipts differ")
    mutations = report.get("mutation_sensitivity")
    required = (
        "wrong_mana_cost_detected",
        "wrong_power_detected",
        "wrong_toughness_detected",
        "trigger_removal_detected",
        "wrong_trigger_condition_detected",
        "optional_trigger_detected",
        "effect_removal_detected",
    )
    if not isinstance(mutations, dict) or any(mutations.get(key) is not True for key in required):
        raise WorkerFailure("candidate_protocol_error: Sadistic Augermage semantic mutations escaped")


def parse_report(stdout: bytes) -> dict:
    for line in reversed(stdout.decode(errors="replace").splitlines()):
        try:
            value = json.loads(line)
        except json.JSONDecodeError:
            continue
        if isinstance(value, dict) and value.get("schema_version") == "cardbench.magic.card-pvp.check.v1":
            validate_report(value)
            return value
    raise WorkerFailure("candidate_protocol_error: no valid Sadistic Augermage PvP report")


def sandbox_command(workspace: Path, registry: Path, authority: Path, name: str, image: str, candidate_kind: str) -> list[str]:
    if authority.is_symlink() or not authority.is_file():
        raise WorkerFailure("trusted Sadistic Augermage authority is missing")
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
        "cp /trusted-authority.rs /work/varieties/magic/session/src/bin/rav_sadistic_augermage_pvp.rs && "
        "cd /work && cargo run --offline --locked -j 2 --manifest-path varieties/magic/Cargo.toml "
        "-p cardbench-magic-session --bin rav-sadistic-augermage-pvp --" + candidate_flag,
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
        raise FileNotFoundError(f"trusted Sadistic Augermage authority is missing: {authority}")
    if not shutil.which("docker"):
        raise FileNotFoundError("Docker is required for Magic candidate isolation")
    verify(workspace, manifest)
    snapshot = make_scoring_snapshot(workspace, manifest_data["files"], output_root / "candidate-workspace")
    candidate_hash = sha256((snapshot / TARGET).read_bytes())
    name = f"cardbench-magic-sadistic-augermage-{uuid.uuid4().hex[:16]}"
    command = sandbox_command(snapshot, registry.resolve(), authority, name, image, candidate_kind)
    code, stdout, stderr = run_bounded(command, timeout=900, name=name)
    (output_root / "worker.stdout").write_bytes(stdout)
    (output_root / "worker.stderr").write_bytes(stderr)
    if code != 0:
        report = {"schema_version": "cardbench.magic.card-pvp.check.v1", "task_id": "cardbench/magic/card", "passed": False, "pilot_card_passed": False, "cards_passed": 0, "cards_total": 291, "printings_total": 306, "cards_credited": 0, "coverage_complete": False, "full_reward": False, "harbor_reward": 0.0, "pilot_reward": 0.0, "error": "candidate compile or Sadistic Augermage trusted trace failed", "diagnostics": stderr.decode(errors="replace")[-12000:]}
    else:
        report = parse_report(stdout)
        report["candidate"] = {"kind": candidate_kind, "card_definition": "RAV-SADISTIC-AUGERMAGE", "source_sha256": candidate_hash, "materialized_workspace_sha256": manifest_data["workspace_sha256"]}
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

