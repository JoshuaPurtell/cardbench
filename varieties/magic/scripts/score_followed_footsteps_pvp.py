#!/usr/bin/env python3
"""Validate and optionally score an isolated Followed Footsteps submission."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import shutil
import subprocess
import uuid
from pathlib import Path

from materialize_followed_footsteps_task import verify

REPO = Path(__file__).resolve().parents[3]
TARGET = Path("varieties/magic/sets/ravnica_city_of_guilds/src/cards/followed_footsteps.rs")
AUTHORITY = Path("varieties/magic/session/src/bin/rav_followed_footsteps_pvp.rs")
BIN = "rav-followed-footsteps-pvp"
CARD_ID = "RAV-FOLLOWED-FOOTSTEPS"
CARD_NAME = "Followed Footsteps"
DEFAULT_IMAGE = "rust@sha256:d7482085ff5b415f84dba5647ae71606650bdef00db7aeb69f4b3d170c3e4082"


class WorkerFailure(RuntimeError):
    pass


def validate_report(report: dict) -> None:
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
        or report.get("card") != {"collector_number": 51, "definition_id": CARD_ID, "name": CARD_NAME}
    ):
        raise WorkerFailure("candidate_protocol_error: incomplete Followed Footsteps single-card report")
    traces = report.get("traces")
    if not isinstance(traces, list) or len(traces) != 1 or not isinstance(traces[0], dict):
        raise WorkerFailure("candidate_protocol_error: exactly one Followed Footsteps full-game trace is required")
    trace = traces[0]
    if (
        trace.get("id") != "rav-followed-footsteps-aura-controller-upkeep-token-copy-full-game"
        or trace.get("passed") is not True
        or not isinstance(trace.get("seed"), int)
        or trace["seed"] < 0
        or trace.get("terminal") != "Winner(PlayerId(0))"
        or trace.get("winner_seat") != 0
        or trace.get("rejected_moves") != 0
        or not isinstance(trace.get("turns"), int)
        or trace["turns"] <= 0
        or not isinstance(trace.get("accepted_moves"), int)
        or trace["accepted_moves"] <= 0
    ):
        raise WorkerFailure("candidate_protocol_error: Followed Footsteps did not complete a clean PvP win")
    mechanics = trace.get("mechanics")
    if not isinstance(mechanics, dict):
        raise WorkerFailure("candidate_protocol_error: Followed Footsteps mechanics are missing")
    payment = mechanics.get("paid_mana_colors")
    active_upkeeps = mechanics.get("trigger_active_upkeep_seats")
    controllers = mechanics.get("trigger_stack_controllers")
    incarnations = mechanics.get("trigger_stack_incarnations")
    receipts = mechanics.get("token_copy_receipts")
    tokens = mechanics.get("token_states")
    attacks = mechanics.get("tattered_drake_attacks")
    damage = mechanics.get("tattered_drake_damage_packets")
    terminal_life = mechanics.get("terminal_life")
    trigger_count = mechanics.get("trigger_stack_count")
    if (
        mechanics.get("definition_id") != CARD_ID
        or mechanics.get("mana_cost") != "{3}{U}{U}"
        or not isinstance(payment, list)
        or len(payment) != 5
        or payment.count("Blue") != 2
        or mechanics.get("attached_creature_definition_id") != "RAV-WATCHWOLF"
        or mechanics.get("attached_creature_controller_seat") != 1
        or mechanics.get("aura_controller_seat") != 0
        or mechanics.get("attached_to_exact_creature_incarnation") is not True
        or not isinstance(trigger_count, int)
        or trigger_count < 2
        or mechanics.get("trigger_ability_id") != "aura-controller-upkeep-token-copy"
        or not isinstance(controllers, list)
        or controllers != [0] * trigger_count
        or not isinstance(active_upkeeps, list)
        or active_upkeeps != [0] * trigger_count
        or not isinstance(incarnations, list)
        or incarnations != [mechanics.get("source_incarnation")] * trigger_count
        or mechanics.get("trigger_resolution_count") != trigger_count
        or mechanics.get("created_token_count") != trigger_count
        or not isinstance(receipts, list)
        or len(receipts) != trigger_count
        or not isinstance(tokens, list)
        or len(tokens) != trigger_count
    ):
        raise WorkerFailure("candidate_protocol_error: Aura upkeep/control scope, payment, or repeated trigger count differs")
    source = mechanics.get("source_object")
    source_incarnation = mechanics.get("source_incarnation")
    target = mechanics.get("attached_creature_object")
    target_incarnation = mechanics.get("attached_creature_incarnation")
    if not all(isinstance(item, int) and item > 0 for item in (source, source_incarnation, target, target_incarnation)):
        raise WorkerFailure("candidate_protocol_error: Aura or enchanted-creature object provenance is invalid")
    token_ids = set()
    for receipt in receipts:
        if (
            not isinstance(receipt, dict)
            or receipt.get("source") != target
            or receipt.get("source_incarnation") != target_incarnation
            or not isinstance(receipt.get("target"), int)
            or receipt["target"] <= 0
            or not isinstance(receipt.get("target_incarnation"), int)
            or receipt["target_incarnation"] <= 0
        ):
            raise WorkerFailure("candidate_protocol_error: token copy source/target incarnation receipt is invalid")
        token_ids.add(receipt["target"])
    for token in tokens:
        if (
            not isinstance(token, dict)
            or token.get("object") not in token_ids
            or token.get("controller_seat") != 0
            or token.get("zone") != "Some(Battlefield)"
            or token.get("incarnation", 0) <= 0
            or token.get("copied_definition_id") != "RAV-WATCHWOLF"
            or token.get("power") != 3
            or token.get("toughness") != 3
            or token.get("is_token") is not True
            or token.get("copied_from") != target
            or token.get("copied_from_incarnation") != target_incarnation
        ):
            raise WorkerFailure("candidate_protocol_error: generated token is not the exact copied Watchwolf")
    if (
        not isinstance(attacks, int)
        or attacks <= 0
        or not isinstance(damage, list)
        or not damage
        or any(amount != 2 for amount in damage)
        or not isinstance(terminal_life, list)
        or len(terminal_life) != 2
        or terminal_life[0] <= 0
        or terminal_life[1] != 0
        or mechanics.get("terminal_pilot_win") is not True
    ):
        raise WorkerFailure("candidate_protocol_error: Tattered Drake combat or terminal game result differs")
    ordered = trace.get("ordered_event_sequence")
    if not isinstance(ordered, list) or ordered != sorted(ordered) or len(set(ordered)) != len(ordered):
        raise WorkerFailure("candidate_protocol_error: Followed Footsteps event order receipt is invalid")
    event_order = mechanics.get("event_order")
    if not isinstance(event_order, dict) or event_order.get("game_end") != ordered[-1]:
        raise WorkerFailure("candidate_protocol_error: terminal event ordering is missing")
    events = trace.get("events")
    if not isinstance(events, list) or not all(isinstance(event, dict) for event in events):
        raise WorkerFailure("candidate_protocol_error: projected event sequence is malformed")
    kinds = [event.get("kind") for event in events]
    if (
        kinds.count("TriggeredAbilityStacked") < trigger_count
        or kinds.count("AbilityResolved") < trigger_count
        or kinds.count("TokenCreated") < trigger_count
        or kinds.count("PermanentCopied") < trigger_count
        or kinds.count("GameEnded") != 1
    ):
        raise WorkerFailure("candidate_protocol_error: upkeep, token-copy, or terminal event receipts are missing")
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
        "printed_cost_mutation_detected_in_game",
        "aura_attachment_binding_removal_detected_in_game",
        "upkeep_trigger_removal_detected_in_game",
        "wrong_upkeep_controller_scope_mutation_detected_in_game",
        "token_copy_effect_removal_detected_in_game",
    ):
        if not isinstance(mutations, dict) or mutations.get(key) is not True:
            raise WorkerFailure(f"candidate_protocol_error: Followed Footsteps mutation escaped: {key}")


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
        "cp -R /submission/. /work/ && cp /trusted-authority.rs /work/varieties/magic/session/src/bin/rav_followed_footsteps_pvp.rs && cd /work && cargo run --offline --locked -j 2 --manifest-path varieties/magic/Cargo.toml -p cardbench-magic-session --bin " + BIN + " --" + flag,
    ]
    return subprocess.run(command, capture_output=True, timeout=900, check=False)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--validate-report", type=Path)
    parser.add_argument("--workspace", type=Path)
    parser.add_argument("--manifest", type=Path)
    parser.add_argument("--output-root", type=Path)
    parser.add_argument("--candidate-kind", choices=["agent", "reference"], default="agent")
    args = parser.parse_args()
    if args.validate_report:
        validate_report(json.loads(args.validate_report.read_text(encoding="utf-8")))
        print("Followed Footsteps report validates; rewards and credit remain zero")
        return 0
    if not args.workspace or not args.manifest or not args.output_root:
        parser.error("--workspace, --manifest, and --output-root are required unless --validate-report is used")
    output = args.output_root.resolve()
    output.mkdir(parents=True, exist_ok=True)
    manifest = json.loads(args.manifest.read_text(encoding="utf-8"))
    image = os.environ.get("CARDBENCH_MAGIC_WORKER_IMAGE", DEFAULT_IMAGE)
    if not (image.startswith("rust@sha256:") or image.startswith("sha256:")):
        raise ValueError("CARDBENCH_MAGIC_WORKER_IMAGE must be an immutable digest reference")
    registry = Path.home() / ".cargo" / "registry"
    authority = REPO / AUTHORITY
    if not registry.is_dir() or not authority.is_file() or authority.is_symlink() or not shutil.which("docker"):
        raise FileNotFoundError("offline Cargo registry, trusted authority, or Docker is unavailable")
    verify(args.workspace.resolve(), args.manifest.resolve())
    candidate_workspace = output / "candidate-workspace"
    if candidate_workspace.exists():
        shutil.rmtree(candidate_workspace)
    for relative in sorted(manifest["files"]):
        source = args.workspace.resolve() / Path(relative)
        destination = candidate_workspace / Path(relative)
        if source.is_symlink() or not source.is_file():
            raise WorkerFailure(f"candidate_workspace_source_missing: {relative}")
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(source, destination)
    source_hash = hashlib.sha256((candidate_workspace / TARGET).read_bytes()).hexdigest()
    name = f"cardbench-magic-followed-footsteps-{uuid.uuid4().hex[:16]}"
    process = run_worker(candidate_workspace, registry.resolve(), authority, name, image, args.candidate_kind)
    (output / "worker.stdout").write_bytes(process.stdout)
    (output / "worker.stderr").write_bytes(process.stderr)
    if process.returncode != 0:
        raise WorkerFailure("candidate_compile_or_reference_failed: " + process.stderr.decode(errors="replace")[-12000:])
    report = None
    for line in reversed(process.stdout.decode(errors="replace").splitlines()):
        try:
            value = json.loads(line)
        except json.JSONDecodeError:
            continue
        if isinstance(value, dict) and value.get("schema_version") == "cardbench.magic.card-pvp.check.v1":
            report = value
            break
    if report is None:
        raise WorkerFailure("candidate_protocol_error: no Followed Footsteps PvP report")
    validate_report(report)
    report["candidate"] = {"kind": args.candidate_kind, "card_definition": CARD_ID, "source_sha256": source_hash, "materialized_workspace_sha256": manifest["workspace_sha256"]}
    report["candidate_kind"] = args.candidate_kind
    report["sealed_authority"] = False
    report["worker_image"] = image
    report["sandbox"] = {"network": "none", "rootfs": "read-only", "pids_limit": 128}
    report["harbor_reward"] = 0.0
    report["pilot_reward"] = 0.0
    (output / "result.json").write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    (output / "reward.txt").write_text("0.00000000\n", encoding="utf-8")
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except Exception as error:
        raise SystemExit(f"Followed Footsteps scoring failed: {error}")
