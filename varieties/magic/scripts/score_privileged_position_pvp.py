#!/usr/bin/env python3
"""Score an isolated Privileged Position submission against its trusted PvP authority."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import selectors
import shutil
import subprocess
import sys
import time
import uuid
from pathlib import Path

from score_watchwolf_pvp import WorkerFailure, make_scoring_snapshot
from materialize_privileged_position_task import verify

REPO = Path(__file__).resolve().parents[3]
AUTHORITY_SOURCE = Path("varieties/magic/session/src/bin/rav_privileged_position_pvp.rs")
TARGET = Path("varieties/magic/sets/ravnica_city_of_guilds/src/cards/privileged_position.rs")
DEFAULT_WORKER_IMAGE = "rust@sha256:d7482085ff5b415f84dba5647ae71606650bdef00db7aeb69f4b3d170c3e4082"
MAX_OUTPUT_BYTES = 2 * 1024 * 1024
MAX_SEED = 256


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def _int(value: object) -> bool:
    return type(value) is int


def _facts(event: dict) -> dict:
    facts = event.get("facts", {})
    return facts if isinstance(facts, dict) else {}


def _has_object(event: dict, object_id: int, zone: str | None = None) -> bool:
    facts = _facts(event)
    return object_id in facts.get("objects", []) and (zone is None or facts.get("zone") == zone)


def validate_report(report: dict) -> None:
    if (
        report.get("schema_version") != "cardbench.magic.card-pvp.check.v1"
        or report.get("task_id") != "cardbench/magic/card"
        or report.get("expansion") != "RAV"
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
        or report.get("card") != {"collector_number": 251, "definition_id": "RAV-PRIVILEGED-POSITION", "name": "Privileged Position"}
    ):
        raise WorkerFailure("candidate_protocol_error: incomplete Privileged Position report or nonzero credit/reward")
    candidate = report.get("candidate")
    if not isinstance(candidate, dict) or candidate.get("card_definition") != "RAV-PRIVILEGED-POSITION":
        raise WorkerFailure("candidate_protocol_error: wrong Privileged Position candidate identity")
    guard = report.get("candidate_source_validation")
    if (
        not isinstance(guard, dict)
        or guard.get("normalized_literal_source_and_ast_checked_before_candidate_calls") is not True
        or guard.get("compile_valid_side_effect_candidate_by_construction") is not True
        or guard.get("compile_valid_injected_side_effect_candidate_rejected_before_candidate_calls") is not True
        or guard.get("candidate_functions_called_before_source_validation") is not False
        or guard.get("side_effect_fixture_executed") is not False
    ):
        raise WorkerFailure("candidate_protocol_error: source guard or malicious-candidate rejection attestation is missing")

    traces = report.get("traces")
    if not isinstance(traces, list) or len(traces) != 1:
        raise WorkerFailure("candidate_protocol_error: exactly one Privileged Position full-game trace is required")
    trace = traces[0]
    if (
        not isinstance(trace, dict)
        or trace.get("id") != "rav-privileged-position-opponent-only-hexproof"
        or trace.get("passed") is not True
        or not _int(trace.get("seed")) or not 0 <= trace["seed"] <= MAX_SEED
        or trace.get("winner_seat") != 0
        or trace.get("terminal") != "Winner(PlayerId(0))"
        or not _int(trace.get("terminal_sequence"))
        or not _int(trace.get("turns")) or trace["turns"] <= 0
        or not _int(trace.get("accepted_moves")) or trace["accepted_moves"] <= 0
        or trace.get("rejected_moves") != 0
        or trace.get("policies") != ["rav.privileged-position-hexproof-pilot.v1", "rav.privileged-position-putrefy-opponent.v1"]
        or trace.get("decks") != ["rav_privileged_position_pilot", "rav_putrefy_opponent"]
    ):
        raise WorkerFailure("candidate_protocol_error: Privileged Position full-game terminal trace differs")

    mechanics = trace.get("mechanics")
    if not isinstance(mechanics, dict):
        raise WorkerFailure("candidate_protocol_error: Privileged Position mechanics are missing")
    position = mechanics.get("privileged_position")
    wolf = mechanics.get("protected_watchwolf")
    opponent = mechanics.get("opponent_putrefy_attempt")
    controller = mechanics.get("controller_gather_courage")
    full_game = mechanics.get("full_game")
    if not all(isinstance(item, dict) for item in (position, wolf, opponent, controller, full_game)):
        raise WorkerFailure("candidate_protocol_error: source, target challenge, or terminal mechanics are missing")
    assert isinstance(position, dict) and isinstance(wolf, dict) and isinstance(opponent, dict) and isinstance(controller, dict) and isinstance(full_game, dict)
    object_ids = [position.get("object"), wolf.get("object"), opponent.get("object"), controller.get("object")]
    if (
        any(not _int(value) or value <= 0 for value in object_ids)
        or position.get("source_incarnation", 0) <= 0
        or position.get("is_enchantment") is not True
        or position.get("mana_cost") != "{2}{G/W}{G/W}{G/W}"
        or not isinstance(position.get("payment"), list) or len(position["payment"]) != 5
        or any(color not in {"Green", "White"} for color in position["payment"])
        or not isinstance(position.get("hybrid_payment"), list) or len(position["hybrid_payment"]) != 3
        or any(color not in {"Green", "White"} for color in position["hybrid_payment"])
        or not isinstance(position.get("generic_payment"), list) or len(position["generic_payment"]) != 2
        or sorted(position["payment"]) != sorted(position["hybrid_payment"] + position["generic_payment"])
        or wolf.get("controller_seat") != 0 or wolf.get("base_power") != 3 or wolf.get("base_toughness") != 3
        or wolf.get("hexproof_from_position") is not True
        or opponent.get("actor_seat") != 1 or opponent.get("mana_cost") != "{1}{B}{G}"
        or opponent.get("target") != wolf.get("object")
        or not str(opponent.get("error", "")).startswith("IllegalTarget")
        or opponent.get("rejected_before_payment") is not True
        or opponent.get("hand_stack_mana_and_event_state_unchanged") is not True
        or controller.get("actor_seat") != 0 or controller.get("mana_cost") != "{G}"
        or controller.get("payment") != ["Green"]
        or controller.get("target") != wolf.get("object")
        or controller.get("resolved") is not True
        or controller.get("target_power_after_resolution") != 5
        or controller.get("target_toughness_after_resolution") != 5
        or controller.get("target_still_has_hexproof") is not True
        or full_game.get("winner_seat") != 0
        or not _int(full_game.get("watchwolf_attack_sequence"))
        or not _int(full_game.get("watchwolf_combat_damage_sequence"))
    ):
        raise WorkerFailure("candidate_protocol_error: hybrid identity, opponent-only rejection, or controller target contrast differs")

    events = trace.get("events")
    if not isinstance(events, list) or len(events) < 20 or any(not isinstance(event, dict) for event in events):
        raise WorkerFailure("candidate_protocol_error: complete projected game event trace is missing")
    sequences = [event.get("sequence") for event in events]
    if any(not _int(sequence) or sequence < 0 for sequence in sequences) or sequences != list(range(len(events))):
        raise WorkerFailure("candidate_protocol_error: projected event sequence is malformed or incomplete")
    event_indices = trace.get("event_indices")
    if not isinstance(event_indices, dict):
        raise WorkerFailure("candidate_protocol_error: relevant event indices are missing")
    required_indices = ("watchwolf_cast", "position_payment", "position_cast", "position_resolved", "position_entered", "gather_payment", "gather_cast", "gather_resolved")
    if any(not _int(event_indices.get(key)) for key in required_indices):
        raise WorkerFailure("candidate_protocol_error: relevant spell event indices are incomplete")
    wolf_cast, position_paid, position_cast, position_resolved, position_entered, gather_paid, gather_cast, gather_resolved = (events[event_indices[key]] for key in required_indices)
    if not (
        wolf_cast.get("kind") == "SpellCast" and _facts(wolf_cast).get("seats") == [0] and _has_object(wolf_cast, wolf["object"])
        and position_paid.get("kind") == "SpellManaPaid" and _facts(position_paid).get("seats") == [0] and _facts(position_paid).get("objects") == [position["object"]]
        and _facts(position_paid).get("amount") == 5 and _facts(position_paid).get("mana_colors") == position["payment"]
        and position_cast.get("kind") == "SpellCast" and _facts(position_cast).get("seats") == [0] and _has_object(position_cast, position["object"])
        and position_resolved.get("kind") == "SpellResolved" and _has_object(position_resolved, position["object"])
        and position_entered.get("kind") == "CardMoved" and _has_object(position_entered, position["object"], "Battlefield")
        and gather_paid.get("kind") == "SpellManaPaid" and _facts(gather_paid).get("seats") == [0] and _facts(gather_paid).get("objects") == [controller["object"]]
        and _facts(gather_paid).get("amount") == 1 and _facts(gather_paid).get("mana_colors") == ["Green"]
        and gather_cast.get("kind") == "SpellCast" and _facts(gather_cast).get("seats") == [0] and _has_object(gather_cast, controller["object"])
        and gather_resolved.get("kind") == "SpellResolved" and _has_object(gather_resolved, controller["object"])
        and event_indices["watchwolf_cast"] < event_indices["position_payment"] < event_indices["position_cast"] < event_indices["position_resolved"] < event_indices["position_entered"] < event_indices["gather_payment"] < event_indices["gather_cast"] < event_indices["gather_resolved"]
    ):
        raise WorkerFailure("candidate_protocol_error: full trace does not show Watchwolf, Position resolution, and same-target controller cast in order")
    terminal = events[trace["terminal_sequence"]]
    attack_seq = full_game["watchwolf_attack_sequence"]
    damage_seq = full_game["watchwolf_combat_damage_sequence"]
    if (
        terminal.get("kind") != "GameEnded" or _facts(terminal).get("seats") != [0]
        or attack_seq >= damage_seq or damage_seq >= trace["terminal_sequence"]
        or events[attack_seq].get("kind") != "AttackersDeclared"
        or _facts(events[attack_seq]).get("seats") != [0]
        or wolf["object"] not in _facts(events[attack_seq]).get("objects", [])
        or events[damage_seq].get("kind") != "DamageDealtToPlayer"
        or _facts(events[damage_seq]).get("seats") != [1]
        or wolf["object"] not in _facts(events[damage_seq]).get("objects", [])
    ):
        raise WorkerFailure("candidate_protocol_error: Watchwolf combat and ordinary terminal win order differs")
    mutations = report.get("mutation_sensitivity")
    required_mutations = (
        "static_binding_removal_detected", "wrong_static_source_scope_detected",
        "shroud_broadening_detected_by_controller_target", "wrong_static_keyword_detected",
        "hybrid_cost_mutation_detected", "card_type_mutation_detected",
        "compile_valid_side_effect_candidate_rejected_before_candidate_calls",
    )
    if not isinstance(mutations, dict) or any(mutations.get(key) is not True for key in required_mutations) or mutations.get("sealed_mutation_authority") is not False:
        raise WorkerFailure("candidate_protocol_error: Privileged Position source/semantic mutations are incomplete")


def parse_report(stdout: bytes) -> dict:
    for line in reversed(stdout.decode(errors="replace").splitlines()):
        try:
            report = json.loads(line)
        except json.JSONDecodeError:
            continue
        if isinstance(report, dict) and report.get("schema_version") == "cardbench.magic.card-pvp.check.v1":
            validate_report(report)
            return report
    raise WorkerFailure("candidate_protocol_error: no valid Privileged Position result")


def sandbox_command(workspace: Path, registry: Path, authority: Path, name: str, image: str, candidate_kind: str) -> list[str]:
    if authority.is_symlink() or not authority.is_file():
        raise WorkerFailure("trusted Privileged Position authority source is missing")
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
        "--env", "CARGO_BUILD_JOBS=2", "--env", "RAYON_NUM_THREADS=2",
        image, "sh", "-c",
        "cp -R /submission/. /work/ && "
        "cp /trusted-authority.rs /work/varieties/magic/session/src/bin/rav_privileged_position_pvp.rs && "
        "cd /work && cargo run --offline --locked -j 2 --manifest-path varieties/magic/Cargo.toml "
        "-p cardbench-magic-session --bin rav-privileged-position-pvp --" + candidate_flag,
    ]


def run_bounded(command: list[str], *, timeout: int, name: str) -> tuple[int, bytes, bytes]:
    process = subprocess.Popen(command, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    assert process.stdout is not None and process.stderr is not None
    selector = selectors.DefaultSelector()
    selector.register(process.stdout, selectors.EVENT_READ, "stdout")
    selector.register(process.stderr, selectors.EVENT_READ, "stderr")
    stdout = bytearray()
    stderr = bytearray()
    start = time.monotonic()
    try:
        while selector.get_map() or process.poll() is None:
            if time.monotonic() - start > timeout:
                raise WorkerFailure("candidate_timeout: Docker worker exceeded its deadline")
            for key, _ in selector.select(0.2):
                chunk = os.read(key.fileobj.fileno(), 65536)
                if not chunk:
                    selector.unregister(key.fileobj)
                    continue
                (stdout if key.data == "stdout" else stderr).extend(chunk)
                if len(stdout) + len(stderr) > MAX_OUTPUT_BYTES:
                    raise WorkerFailure("candidate_output_limit: worker output exceeded 2 MiB")
        return process.wait(), bytes(stdout), bytes(stderr)
    except BaseException:
        process.kill()
        process.wait()
        subprocess.run(["docker", "rm", "-f", name], capture_output=True, check=False)
        raise


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
        raise FileNotFoundError(f"trusted Privileged Position authority is missing: {authority}")
    if not shutil.which("docker"):
        raise FileNotFoundError("Docker is required for Magic candidate isolation")
    verify(workspace, manifest)
    snapshot = make_scoring_snapshot(workspace, manifest_data["files"], output_root / "candidate-workspace")
    candidate_hash = sha256((snapshot / TARGET).read_bytes())
    name = f"cardbench-magic-privileged-position-{uuid.uuid4().hex[:16]}"
    code, stdout, stderr = run_bounded(sandbox_command(snapshot, registry.resolve(), authority, name, image, candidate_kind), timeout=900, name=name)
    (output_root / "worker.stdout").write_bytes(stdout)
    (output_root / "worker.stderr").write_bytes(stderr)
    if code == 0:
        result = parse_report(stdout)
        result["candidate"] = {"kind": candidate_kind, "card_definition": "RAV-PRIVILEGED-POSITION", "source_sha256": candidate_hash, "materialized_workspace_sha256": manifest_data["workspace_sha256"]}
        result["candidate_kind"] = candidate_kind
        result["starting_workspace_sha256"] = manifest_data["workspace_sha256"]
        result["sealed_authority"] = False
        result["worker_image"] = image
        result["sandbox"] = {"network": "none", "rootfs": "read-only", "pids_limit": 128}
        result["harbor_reward"] = 0.0
        result["full_reward"] = False
    else:
        result = {
            "schema_version": "cardbench.magic.card-pvp.check.v1", "task_id": "cardbench/magic/card",
            "candidate": {"kind": candidate_kind, "source_sha256": candidate_hash},
            "passed": False, "pilot_card_passed": False, "cards_passed": 0, "cards_total": 291,
            "printings_total": 306, "cards_credited": 0, "coverage_complete": False,
            "full_reward": False, "harbor_reward": 0.0, "pilot_reward": 0.0, "sealed_authority": False,
            "worker_image": image, "sandbox": {"network": "none", "rootfs": "read-only", "pids_limit": 128},
            "error": "candidate compile or Privileged Position authority failed",
            "diagnostics": stderr.decode(errors="replace")[-12000:],
        }
    (output_root / "result.json").write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
    (output_root / "reward.txt").write_text(f"{result['harbor_reward']:.8f}\n", encoding="utf-8")
    return 0 if result.get("passed") is True and result.get("pilot_card_passed") is True else 1


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
        result = {
            "schema_version": "cardbench.magic.card-pvp.check.v1", "task_id": "cardbench/magic/card",
            "passed": False, "pilot_card_passed": False, "cards_passed": 0, "cards_total": 291,
            "printings_total": 306, "cards_credited": 0, "coverage_complete": False,
            "full_reward": False, "harbor_reward": 0.0, "sealed_authority": False, "error": str(error),
        }
        (args.output_root / "result.json").write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
        (args.output_root / "reward.txt").write_text("0.00000000\n", encoding="utf-8")
        print(f"Magic candidate verifier failed: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
