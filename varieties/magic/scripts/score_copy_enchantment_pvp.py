#!/usr/bin/env python3
"""Validate and optionally score an isolated Copy Enchantment submission."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import shutil
import subprocess
import uuid
from pathlib import Path

from materialize_copy_enchantment_task import verify

REPO = Path(__file__).resolve().parents[3]
TARGET = Path("varieties/magic/sets/ravnica_city_of_guilds/src/cards/copy_enchantment.rs")
AUTHORITY = Path("varieties/magic/session/src/bin/rav_copy_enchantment_pvp.rs")
BIN = "rav-copy-enchantment-pvp"
CARD_ID = "RAV-COPY-ENCHANTMENT"
CARD_NAME = "Copy Enchantment"
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
        or report.get("card") != {"collector_number": 42, "definition_id": CARD_ID, "name": CARD_NAME}
    ):
        raise WorkerFailure("candidate_protocol_error: incomplete Copy Enchantment single-card report")
    traces = report.get("traces")
    if not isinstance(traces, list) or len(traces) != 1 or not isinstance(traces[0], dict):
        raise WorkerFailure("candidate_protocol_error: exactly one Copy Enchantment full-game trace is required")
    trace = traces[0]
    if (
        trace.get("id") != "rav-copy-enchantment-entry-copy-aura-and-decline-full-game"
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
        raise WorkerFailure("candidate_protocol_error: Copy Enchantment did not complete a clean PvP win")
    mechanics = trace.get("copy_enchantment")
    if not isinstance(mechanics, dict):
        raise WorkerFailure("candidate_protocol_error: Copy Enchantment mechanics are missing")
    copies = mechanics.get("copy_spell_objects")
    payments = mechanics.get("paid_mana_colors")
    source_selections = mechanics.get("source_selections")
    aura_selections = mechanics.get("aura_attachment_selections")
    copy_receipts = mechanics.get("permanent_copied_receipts")
    attacks = mechanics.get("watchwolf_attacks")
    damage = mechanics.get("watchwolf_damage_packets")
    terminal_life = mechanics.get("terminal_life")
    if (
        mechanics.get("definition_id") != CARD_ID
        or mechanics.get("mana_cost") != "{2}{U}"
        or not isinstance(copies, list)
        or len(copies) != 2
        or copies[0] == copies[1]
        or not isinstance(payments, list)
        or len(payments) != 2
        or not all(isinstance(payment, list) and len(payment) == 3 and payment.count("Blue") == 1 for payment in payments)
        or mechanics.get("source_choice_count") != 2
        or mechanics.get("aura_attachment_choice_count") != 1
        or not isinstance(source_selections, list)
        or len(source_selections) != 2
        or not isinstance(source_selections[0], list)
        or len(source_selections[0]) != 1
        or source_selections[1] != []
        or not isinstance(aura_selections, list)
        or len(aura_selections) != 1
        or not isinstance(aura_selections[0], list)
        or len(aura_selections[0]) != 1
        or mechanics.get("copied_aura_definition_id") != "RAV-FISTS-OF-IRONWOOD"
        or mechanics.get("declined_copy_definition_id") != CARD_ID
        or mechanics.get("copied_aura_attached_to") != aura_selections[0][0]
        or mechanics.get("source_aura_attached_to") != mechanics.get("copied_aura_attached_to")
        or not isinstance(copy_receipts, list)
        or len(copy_receipts) != 1
    ):
        raise WorkerFailure("candidate_protocol_error: copy/decline choices, payment, or Aura attachment differs")
    receipt = copy_receipts[0]
    if (
        not isinstance(receipt, dict)
        or receipt.get("source") != source_selections[0][0]
        or receipt.get("target") != copies[0]
        or not isinstance(receipt.get("source_incarnation"), int)
        or receipt["source_incarnation"] <= 0
        or not isinstance(receipt.get("target_incarnation"), int)
        or receipt["target_incarnation"] <= 0
    ):
        raise WorkerFailure("candidate_protocol_error: PermanentCopied source/target incarnation receipt is invalid")
    if (
        mechanics.get("copied_aura_etb_trigger_stacks") != 2
        or mechanics.get("copied_aura_etb_trigger_resolutions") != 2
        or mechanics.get("saproling_token_events") != 4
        or mechanics.get("watchwolf_definition_id") != "RAV-WATCHWOLF"
        or mechanics.get("watchwolf_received_trample") is not True
        or mechanics.get("watchwolf_power") != 3
        or mechanics.get("watchwolf_toughness") != 3
        or not isinstance(attacks, int)
        or attacks <= 0
        or not isinstance(damage, list)
        or not damage
        or any(amount != 3 for amount in damage)
        or not isinstance(terminal_life, list)
        or len(terminal_life) != 2
        or terminal_life[0] <= 0
        or terminal_life[1] != 0
        or mechanics.get("terminal_pilot_win") is not True
    ):
        raise WorkerFailure("candidate_protocol_error: copied Aura trigger, battlefield state, combat, or terminal result differs")
    ordered = trace.get("ordered_event_sequence")
    if not isinstance(ordered, list) or ordered != sorted(ordered) or len(set(ordered)) != len(ordered):
        raise WorkerFailure("candidate_protocol_error: event ordering receipt is invalid")
    event_order = mechanics.get("event_order")
    if not isinstance(event_order, dict) or event_order.get("game_end") != ordered[-1]:
        raise WorkerFailure("candidate_protocol_error: terminal event ordering is missing")
    events = trace.get("events")
    if not isinstance(events, list) or not all(isinstance(event, dict) for event in events):
        raise WorkerFailure("candidate_protocol_error: projected event sequence is malformed")
    kinds = [event.get("kind") for event in events]
    if kinds.count("PermanentCopied") != 1 or kinds.count("GameEnded") != 1:
        raise WorkerFailure("candidate_protocol_error: copy or terminal event receipts are missing")
    guard = report.get("candidate_source_validation", {})
    if (
        guard.get("normalized_literal_source_and_ast_checked_before_candidate_calls") is not True
        or guard.get("compile_valid_injected_side_effect_fixture_parsed") is not True
        or guard.get("compile_valid_injected_side_effect_candidate_rejected") is not True
        or guard.get("injected_candidate_calls_before_rejection") != 0
        or guard.get("side_effect_sentinel_absent_after_rejection") is not True
    ):
        raise WorkerFailure("candidate_protocol_error: source guard did not reject side effects before candidate evaluation")
    mutations = report.get("mutation_sensitivity", {})
    for key in (
        "printed_cast_cost_mutation_detected_in_game",
        "printed_enchantment_type_mutation_detected_in_game",
        "entry_copy_binding_removal_detected_in_game",
        "wrong_copyable_type_scope_mutation_detected_in_game",
    ):
        if not isinstance(mutations, dict) or mutations.get(key) is not True:
            raise WorkerFailure(f"candidate_protocol_error: Copy Enchantment mutation escaped: {key}")


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
        "cp -R /submission/. /work/ && cp /trusted-authority.rs /work/varieties/magic/session/src/bin/rav_copy_enchantment_pvp.rs && cd /work && cargo run --offline --locked -j 2 --manifest-path varieties/magic/Cargo.toml -p cardbench-magic-session --bin " + BIN + " --" + flag,
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
        print("Copy Enchantment report validates; rewards and credit remain zero")
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
    name = f"cardbench-magic-copy-enchantment-{uuid.uuid4().hex[:16]}"
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
        raise WorkerFailure("candidate_protocol_error: no Copy Enchantment PvP report")
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
        raise SystemExit(f"Copy Enchantment scoring failed: {error}")
