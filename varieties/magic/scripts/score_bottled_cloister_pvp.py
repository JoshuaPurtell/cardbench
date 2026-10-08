#!/usr/bin/env python3
"""Score an isolated Bottled Cloister submission against its trusted trace."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import shutil
import uuid
from pathlib import Path

from score_watchwolf_pvp import WorkerFailure, make_scoring_snapshot, run_bounded
from materialize_bottled_cloister_task import verify

REPO = Path(__file__).resolve().parents[3]
AUTHORITY_SOURCE = Path("varieties/magic/session/src/bin/rav_bottled_cloister_pvp.rs")
TARGET = Path("varieties/magic/sets/ravnica_city_of_guilds/src/cards/bottled_cloister.rs")
DEFAULT_WORKER_IMAGE = "rust@sha256:d7482085ff5b415f84dba5647ae71606650bdef00db7aeb69f4b3d170c3e4082"


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


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
        or report.get("card") != {
            "collector_number": 256,
            "definition_id": "RAV-BOTTLED-CLOISTER",
            "name": "Bottled Cloister",
        }
    ):
        raise WorkerFailure("candidate_protocol_error: incomplete Bottled Cloister single-card report")
    traces = report.get("traces")
    if not isinstance(traces, list) or len(traces) != 1:
        raise WorkerFailure("candidate_protocol_error: one Bottled Cloister trace is required")
    trace = traces[0]
    if (
        not isinstance(trace, dict)
        or trace.get("id") != "rav-bottled-cloister-opponent-upkeep-linked-hand-lifecycle"
        or trace.get("passed") is not True
        or trace.get("terminal") != "Winner(PlayerId(0))"
        or trace.get("winner_seat") != 0
        or trace.get("rejected_moves") != 0
    ):
        raise WorkerFailure("candidate_protocol_error: Bottled Cloister full-game terminal trace differs")

    mechanics = trace.get("mechanics")
    if not isinstance(mechanics, dict):
        raise WorkerFailure("candidate_protocol_error: Bottled Cloister mechanics are missing")
    source = mechanics.get("cloister_object")
    incarnation = mechanics.get("cloister_source_incarnation")
    paid = mechanics.get("paid_mana")
    exile = mechanics.get("opponent_upkeep_exile")
    returned = mechanics.get("controller_upkeep_return_then_draw")
    finisher = mechanics.get("watchwolf")
    if (
        not isinstance(source, int)
        or not isinstance(incarnation, int)
        or not isinstance(paid, list)
        or len(paid) != 4
        or not isinstance(exile, dict)
        or not isinstance(returned, dict)
        or not isinstance(finisher, dict)
        or mechanics.get("source_incarnation_consistent") is not True
    ):
        raise WorkerFailure("candidate_protocol_error: linked-hand source identity or cast payment is incomplete")

    exiled = exile.get("exiled_cards")
    if (
        exile.get("ability") != "opponent-upkeep-exile-controller-hand-linked"
        or exile.get("controller_hand_empty_after_resolution") is not True
        or exile.get("hidden_from_opponent_view") is not True
        or exile.get("source_object") != source
        or exile.get("source_incarnation") != incarnation
        or not isinstance(exiled, list)
        or len(exiled) < 2
        or exile.get("exiled_count") != len(exiled)
        or any(not isinstance(card, dict) or not isinstance(card.get("object"), int) or not isinstance(card.get("definition_id"), str) for card in exiled)
    ):
        raise WorkerFailure("candidate_protocol_error: current hand was not privately linked to the Cloister incarnation")
    exiled_ids = [card["object"] for card in exiled]
    if len(set(exiled_ids)) != len(exiled_ids):
        raise WorkerFailure("candidate_protocol_error: exiled hand group contains duplicate objects")

    returned_ids = returned.get("returned_objects")
    drawn = returned.get("drawn_object")
    if (
        returned.get("ability") != "controller-upkeep-return-linked-hand-then-draw"
        or returned.get("source_object") != source
        or returned.get("source_incarnation") != incarnation
        or not isinstance(returned_ids, list)
        or not all(isinstance(value, int) for value in returned_ids)
        or len(returned_ids) != len(exiled_ids)
        or set(returned_ids) != set(exiled_ids)
        or returned.get("returned_same_linked_group") is not True
        or not isinstance(drawn, int)
        or drawn in exiled_ids
        or not isinstance(returned.get("drawn_definition_id"), str)
        or returned.get("draw_after_return_before_ability_resolves") is not True
        or not isinstance(returned.get("library_size_before_effect_draw"), int)
        or returned.get("library_size_after_effect_draw") != returned.get("library_size_before_effect_draw") - 1
    ):
        raise WorkerFailure("candidate_protocol_error: linked hand did not return before the controller upkeep draw")

    wolf = finisher.get("object")
    if (
        not isinstance(wolf, int)
        or finisher.get("mana_cost") != "{G}{W}"
        or not isinstance(finisher.get("payment"), list)
        or len(finisher.get("payment", [])) != 2
        or any(not isinstance(color, str) for color in finisher.get("payment", []))
        or set(finisher.get("payment", [])) != {"Green", "White"}
        or finisher.get("power") != 3
        or finisher.get("toughness") != 3
        or finisher.get("attacked") is not True
        or finisher.get("damage") != 3
    ):
        raise WorkerFailure("candidate_protocol_error: full-game Watchwolf finish is incomplete")

    events = trace.get("events")
    if not isinstance(events, list) or not all(isinstance(event, dict) for event in events):
        raise WorkerFailure("candidate_protocol_error: Bottled Cloister event projection is malformed")
    sequences = [event.get("sequence") for event in events]
    if any(not isinstance(value, int) for value in sequences) or sequences != sorted(sequences):
        raise WorkerFailure("candidate_protocol_error: event sequences must be strictly ordered")
    expected_kinds = (
        ["SpellManaPaid", "SpellCast", "SpellResolved", "CardMoved", "TriggeredAbilityStacked"]
        + ["CardMoved"] * len(exiled_ids)
        + ["HandExiledWithSource", "AbilityResolved", "TriggeredAbilityStacked"]
        + ["CardMoved"] * len(exiled_ids)
        + ["LinkedHandExileReturned", "CardMoved", "AbilityResolved", "SpellManaPaid", "SpellCast", "SpellResolved", "CardMoved", "AttackersDeclared", "DamageDealtToPlayer", "GameEnded"]
    )
    if [event.get("kind") for event in events] != expected_kinds:
        raise WorkerFailure("candidate_protocol_error: linked exile/return and draw events are missing or reordered")

    def has_object(event: dict, obj: int, *, zone: str | None = None) -> bool:
        facts = event.get("facts", {})
        return (
            isinstance(facts, dict)
            and facts.get("objects") == [obj]
            and (zone is None or facts.get("zone") == zone)
        )

    cloister_paid, cloister_cast, cloister_resolved, cloister_entered = events[:4]
    exile_trigger = events[4]
    exile_move_start = 5
    exile_move_end = exile_move_start + len(exiled_ids)
    exile_moves = events[exile_move_start:exile_move_end]
    exile_receipt, exile_resolved = events[exile_move_end:exile_move_end + 2]
    return_trigger = events[exile_move_end + 2]
    return_move_start = exile_move_end + 3
    return_move_end = return_move_start + len(exiled_ids)
    return_moves = events[return_move_start:return_move_end]
    return_receipt, draw_move, return_resolved = events[return_move_end:return_move_end + 3]
    wolf_paid, wolf_cast, wolf_resolved, wolf_entered, attack, damage, terminal = events[-7:]
    receipt_exile = exile_receipt.get("canonical", "")
    receipt_return = return_receipt.get("canonical", "")

    if not (
        all(has_object(event, source) for event in (cloister_paid, cloister_cast, cloister_resolved, cloister_entered))
        and cloister_paid.get("facts", {}).get("amount") == 4
        and exile_trigger.get("canonical", "").find("opponent-upkeep-exile-controller-hand-linked") >= 0
        and all(any(f"ObjectId({obj})" in move.get("canonical", "") and "to: Exile" in move.get("canonical", "") for move in exile_moves) for obj in exiled_ids)
        and all(f"ObjectId({obj})" in receipt_exile for obj in exiled_ids)
        and f"source: ObjectId({source})" in receipt_exile
        and f"source_incarnation: {incarnation}" in receipt_exile
        and exile_resolved.get("facts", {}).get("ability") == "opponent-upkeep-exile-controller-hand-linked"
        and exile_resolved.get("facts", {}).get("incarnation") == incarnation
        and return_trigger.get("canonical", "").find("controller-upkeep-return-linked-hand-then-draw") >= 0
        and {obj for obj in exiled_ids if any(has_object(event, obj, zone="Hand") for event in return_moves)} == set(exiled_ids)
        and all(f"ObjectId({obj})" in receipt_return for obj in exiled_ids)
        and f"source: ObjectId({source})" in receipt_return
        and f"source_incarnation: {incarnation}" in receipt_return
        and return_resolved.get("facts", {}).get("ability") == "controller-upkeep-return-linked-hand-then-draw"
        and return_resolved.get("facts", {}).get("incarnation") == incarnation
        and has_object(draw_move, drawn, zone="Hand")
        and draw_move.get("sequence", 0) < return_resolved.get("sequence", 0)
        and all(has_object(event, wolf) for event in (wolf_paid, wolf_cast, wolf_resolved, wolf_entered))
        and attack.get("facts", {}).get("seats") == [0]
        and wolf in attack.get("facts", {}).get("objects", [])
        and damage.get("facts", {}).get("objects") == [wolf]
        and damage.get("facts", {}).get("seats") == [1]
        and damage.get("facts", {}).get("amount") == 3
        and terminal.get("kind") == "GameEnded"
        and terminal.get("facts", {}).get("seats") == [0]
    ):
        raise WorkerFailure("candidate_protocol_error: ordered source-bound hand moves and terminal combat evidence differ")
    authority_sequences = (
        ("exile_trigger_stacked_sequence", exile_trigger),
        ("exile_receipt_sequence", exile_receipt),
        ("return_trigger_stacked_sequence", return_trigger),
        ("linked_return_sequence", return_receipt),
        ("draw_sequence", draw_move),
        ("terminal_sequence", terminal),
    )
    if any(trace.get(key) != event.get("sequence") for key, event in authority_sequences):
        raise WorkerFailure("candidate_protocol_error: linked-hand event sequence receipts do not match the event trace")
    mutations = report.get("mutation_sensitivity")
    required = (
        "opponent_upkeep_trigger_removal_detected",
        "controller_upkeep_trigger_removal_detected",
        "return_before_draw_order_mutation_detected",
        "upkeep_draw_removal_detected",
        "wrong_upkeep_condition_detected",
    )
    if not isinstance(mutations, dict) or any(mutations.get(key) is not True for key in required):
        raise WorkerFailure("candidate_protocol_error: linked-hand semantic mutation controls are incomplete")


def parse_report(stdout: bytes) -> dict:
    for line in reversed(stdout.decode(errors="replace").splitlines()):
        try:
            value = json.loads(line)
        except json.JSONDecodeError:
            continue
        if isinstance(value, dict) and value.get("schema_version") == "cardbench.magic.card-pvp.check.v1":
            validate_report(value)
            return value
    raise WorkerFailure("candidate_protocol_error: no valid Bottled Cloister PvP result")


def sandbox_command(workspace: Path, registry: Path, authority: Path, name: str, image: str, candidate_kind: str) -> list[str]:
    if authority.is_symlink() or not authority.is_file():
        raise WorkerFailure("trusted Bottled Cloister authority source is missing")
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
        "cp /trusted-authority.rs /work/varieties/magic/session/src/bin/rav_bottled_cloister_pvp.rs && "
        "cd /work && cargo run --offline --locked -j 2 --manifest-path varieties/magic/Cargo.toml "
        "-p cardbench-magic-session --bin rav-bottled-cloister-pvp --" + candidate_flag,
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
        raise FileNotFoundError(f"trusted Bottled Cloister authority source is missing: {authority}")
    if not shutil.which("docker"):
        raise FileNotFoundError("Docker is required for Magic candidate isolation")
    verify(workspace, manifest)
    scoring_workspace = make_scoring_snapshot(workspace, manifest_data["files"], output_root / "candidate-workspace")
    candidate_hash = sha256((scoring_workspace / TARGET).read_bytes())
    name = f"cardbench-magic-bottled-cloister-{uuid.uuid4().hex[:16]}"
    command = sandbox_command(scoring_workspace, registry.resolve(), authority, name, image, candidate_kind)
    code, stdout, stderr = run_bounded(command, timeout=900, name=name)
    (output_root / "worker.stdout").write_bytes(stdout)
    (output_root / "worker.stderr").write_bytes(stderr)
    if code != 0:
        report = {"schema_version":"cardbench.magic.card-pvp.check.v1","task_id":"cardbench/magic/card","passed":False,"pilot_card_passed":False,"cards_passed":0,"cards_total":291,"printings_total":306,"cards_credited":0,"coverage_complete":False,"full_reward":False,"harbor_reward":0.0,"pilot_reward":0.0,"error":"candidate compile or Bottled Cloister reference trace failed","diagnostics":stderr.decode(errors="replace")[-12000:]}
    else:
        report = parse_report(stdout)
        report["candidate"] = {"kind":candidate_kind,"card_definition":"RAV-BOTTLED-CLOISTER","source_sha256":candidate_hash,"materialized_workspace_sha256":manifest_data["workspace_sha256"]}
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
    parser.add_argument("--candidate-kind", choices=["reference", "agent"], default="agent")
    args = parser.parse_args()
    return score(args.workspace.resolve(), args.manifest.resolve(), args.output_root.resolve(), args.candidate_kind)


if __name__ == "__main__":
    raise SystemExit(main())
