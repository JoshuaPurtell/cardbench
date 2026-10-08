#!/usr/bin/env python3
"""Run Indentured Oaf in a bounded offline candidate worker."""

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

REPO = Path(__file__).resolve().parents[3]
MATERIALIZER = Path(__file__).with_name("materialize_indentured_oaf_task.py")
TARGET = Path("varieties/magic/sets/ravnica_city_of_guilds/src/cards/indentured_oaf.rs")
AUTHORITY = Path("varieties/magic/session/src/bin/rav_indentured_oaf_pvp.rs")
DEFAULT_WORKER_IMAGE = "rust@sha256:d7482085ff5b415f84dba5647ae71606650bdef00db7aeb69f4b3d170c3e4082"
MAX_OUTPUT_BYTES = 2 * 1024 * 1024


class WorkerFailure(RuntimeError):
    pass


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def verify_workspace(workspace: Path, manifest: Path) -> None:
    subprocess.run(
        [sys.executable, str(MATERIALIZER), "--workspace", str(workspace), "--manifest", str(manifest), "--verify"],
        check=True,
        cwd=REPO,
    )


def run_bounded(command: list[str], *, timeout: int, name: str) -> tuple[int, bytes, bytes]:
    proc = subprocess.Popen(command, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    assert proc.stdout is not None and proc.stderr is not None
    selector = selectors.DefaultSelector()
    selector.register(proc.stdout, selectors.EVENT_READ, "stdout")
    selector.register(proc.stderr, selectors.EVENT_READ, "stderr")
    output, diagnostics = bytearray(), bytearray()
    started = time.monotonic()
    try:
        while selector.get_map() or proc.poll() is None:
            if time.monotonic() - started > timeout:
                raise WorkerFailure("candidate_timeout: Docker worker exceeded its deadline")
            for key, _ in selector.select(0.2):
                chunk = os.read(key.fileobj.fileno(), 65536)
                if not chunk:
                    selector.unregister(key.fileobj)
                    continue
                (output if key.data == "stdout" else diagnostics).extend(chunk)
                if len(output) + len(diagnostics) > MAX_OUTPUT_BYTES:
                    raise WorkerFailure("candidate_output_limit: worker output exceeded 2 MiB")
        return proc.wait(), bytes(output), bytes(diagnostics)
    except BaseException:
        proc.kill()
        proc.wait()
        subprocess.run(["docker", "rm", "-f", name], capture_output=True, check=False)
        raise


def sandbox_command(workspace: Path, registry: Path, authority: Path, name: str, image: str, candidate_kind: str) -> list[str]:
    if authority.is_symlink() or not authority.is_file():
        raise WorkerFailure("trusted_authority_source_missing")
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
        "cp /trusted-authority.rs /work/varieties/magic/session/src/bin/rav_indentured_oaf_pvp.rs && "
        "cd /work && cargo run --offline --locked -j 2 --manifest-path varieties/magic/Cargo.toml "
        "-p cardbench-magic-session --bin rav-indentured-oaf-pvp --" + candidate_flag,
    ]


def parse_report(stdout: bytes) -> dict:
    for line in reversed(stdout.decode(errors="replace").splitlines()):
        try:
            value = json.loads(line)
        except json.JSONDecodeError:
            continue
        if value.get("schema_version") == "cardbench.magic.card-pvp.check.v1":
            validate_report(value)
            return value
    raise WorkerFailure("candidate_protocol_error: no valid Indentured Oaf PvP result")


def validate_report(report: dict) -> None:
    expected_card = {"collector_number": 133, "definition_id": "RAV-INDENTURED-OAF", "name": "Indentured Oaf"}
    if (
        report.get("task_id") != "cardbench/magic/card"
        or report.get("passed") is not True
        or report.get("pilot_card_passed") is not True
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
        or report.get("qualification_status") != "unreviewed_diagnostic"
    ):
        raise WorkerFailure("candidate_protocol_error: incomplete Indentured Oaf zero-credit report")
    candidate = report.get("candidate")
    if not isinstance(candidate, dict) or candidate.get("card_definition") != "RAV-INDENTURED-OAF":
        raise WorkerFailure("candidate_protocol_error: wrong Indentured Oaf candidate identity")
    traces = report.get("traces")
    if not isinstance(traces, list) or len(traces) != 1 or traces[0].get("trace_id") != "rav-indentured-oaf-source-bound-red-creature-prevention-full-game":
        raise WorkerFailure("candidate_protocol_error: exactly one Indentured Oaf full-game trace is required")
    trace = traces[0]
    mechanics = trace.get("mechanics")
    if (
        trace.get("terminal") != "Winner(PlayerId(0))"
        or trace.get("winner_seat") != 0
        or trace.get("rejected_moves") != 0
        or not isinstance(mechanics, dict)
        or mechanics.get("definition_id") != "RAV-INDENTURED-OAF"
        or mechanics.get("printed_cost") != "{3}{R}"
        or mechanics.get("power") != 4
        or mechanics.get("toughness") != 3
        or not isinstance(mechanics.get("mana_payment"), list)
        or len(mechanics["mana_payment"]) != 4
        or any(color != "Red" for color in mechanics["mana_payment"])
        or mechanics.get("target_mana_cost") != "{R/W}"
        or mechanics.get("target_hybrid_payment") != ["Red"]
        or mechanics.get("prevented_source") != mechanics.get("oaf_object")
        or mechanics.get("target_definition_id") != "RAV-BOROS-RECRUIT"
        or not isinstance(mechanics.get("target_object"), int)
        or mechanics.get("target_object") != mechanics.get("prevented_target")
        or mechanics.get("target_is_red_creature") is not True
        or mechanics.get("prevented_damage") != 4
        or mechanics.get("damage_dealt_to_target") != 0
        or mechanics.get("damage_dealt_back_to_oaf") != 1
        or mechanics.get("target_marked_damage_at_terminal") != 0
        or mechanics.get("target_zone_at_terminal") != "Battlefield"
        or mechanics.get("source_bound_direction") != "Oaf-to-red-creature"
        or mechanics.get("opposite_direction_damage_not_prevented") is not True
        or mechanics.get("oaf_player_damage_total") != 20
        or mechanics.get("oaf_player_damage_packets") != [4, 4, 4, 4, 4]
        or not isinstance(mechanics.get("oaf_incarnation"), int)
    ):
        raise WorkerFailure("candidate_protocol_error: Indentured Oaf printed characteristics or damage direction differs")
    events = trace.get("events")
    if not isinstance(events, list) or len(events) < 16:
        raise WorkerFailure("candidate_protocol_error: Indentured Oaf combat and terminal events are missing")
    required = [
        "SpellManaPaid", "SpellCast", "CardMoved",
        "SpellManaPaid", "SpellCast", "SpellResolved", "CardMoved", "ObjectIncarnationAdvanced",
        "AttackersDeclared", "BlockersDeclared",
        "StepBegan", "DamageDealtToPermanent", "StepBegan", "DamagePrevented",
        "DamageDealtToPlayer", "GameEnded",
    ]
    cursor = 0
    for event in events:
        if cursor < len(required) and event.get("kind") == required[cursor]:
            cursor += 1
    if cursor != len(required):
        raise WorkerFailure("candidate_protocol_error: Indentured Oaf combat prevention event order is incomplete")
    blocker, oaf = mechanics["target_object"], mechanics["oaf_object"]
    blocker_mana = next((event for event in events if event.get("sequence") == trace.get("blocker_mana_sequence")), None)
    blocker_cast = next((event for event in events if event.get("sequence") == trace.get("blocker_cast_sequence")), None)
    oaf_mana = next((event for event in events if event.get("sequence") == trace.get("oaf_mana_sequence")), None)
    oaf_cast = next((event for event in events if event.get("sequence") == trace.get("oaf_cast_sequence")), None)
    target_move = next((event for event in events if event.get("kind") == "CardMoved" and f"card: ObjectId({blocker})" in event.get("canonical", "")), None)
    oaf_move = next((event for event in events if event.get("kind") == "CardMoved" and f"card: ObjectId({oaf})" in event.get("canonical", "")), None)
    attackers = next((event for event in events if event.get("kind") == "AttackersDeclared"), None)
    blockers = next((event for event in events if event.get("kind") == "BlockersDeclared"), None)
    prevent = next((event for event in events if event.get("kind") == "DamagePrevented"), None)
    retaliation = next((event for event in events if event.get("kind") == "DamageDealtToPermanent"), None)
    first_strike_step = next((event for event in events if event.get("kind") == "StepBegan" and "FirstStrikeCombatDamage" in event.get("canonical", "")), None)
    normal_damage_step = next((event for event in events if event.get("kind") == "StepBegan" and "step: CombatDamage" in event.get("canonical", "")), None)
    final_damage = next((event for event in events if event.get("kind") == "DamageDealtToPlayer"), None)
    terminal = next((event for event in events if event.get("kind") == "GameEnded"), None)
    if (
        blocker_mana is None
        or blocker_mana.get("kind") != "SpellManaPaid"
        or f"player: PlayerId(1), card: ObjectId({blocker})" not in blocker_mana.get("canonical", "")
        or "colors: [Red]" not in blocker_mana.get("canonical", "")
        or blocker_cast is None
        or blocker_cast.get("kind") != "SpellCast"
        or f"player: PlayerId(1), card: ObjectId({blocker})" not in blocker_cast.get("canonical", "")
        or target_move is None
        or "to: Battlefield" not in target_move.get("canonical", "")
        or oaf_mana is None
        or oaf_mana.get("kind") != "SpellManaPaid"
        or f"player: PlayerId(0), card: ObjectId({oaf})" not in oaf_mana.get("canonical", "")
        or "colors: [Red, Red, Red, Red]" not in oaf_mana.get("canonical", "")
        or oaf_cast is None
        or oaf_cast.get("kind") != "SpellCast"
        or f"player: PlayerId(0), card: ObjectId({oaf})" not in oaf_cast.get("canonical", "")
        or oaf_move is None
        or "to: Battlefield" not in oaf_move.get("canonical", "")
        or attackers is None
        or f"player: PlayerId(0), attackers: [ObjectId({oaf})]" not in attackers.get("canonical", "")
        or blockers is None
        or f"player: PlayerId(1), assignments: [(ObjectId({oaf}), ObjectId({blocker}))]" not in blockers.get("canonical", "")
        or first_strike_step is None
        or normal_damage_step is None
        or prevent is None
        or f"source: ObjectId({oaf})" not in prevent.get("canonical", "")
        or f"permanent: ObjectId({blocker})" not in prevent.get("canonical", "")
        or "amount: 4" not in prevent.get("canonical", "")
        or retaliation is None
        or f"source: ObjectId({blocker})" not in retaliation.get("canonical", "")
        or f"permanent: ObjectId({oaf})" not in retaliation.get("canonical", "")
        or "amount: 1" not in retaliation.get("canonical", "")
        or final_damage is None
        or f"source: ObjectId({oaf})" not in final_damage.get("canonical", "")
        or "player: PlayerId(1), amount: 4" not in final_damage.get("canonical", "")
        or terminal is None
        or "winner: Some(PlayerId(0))" not in terminal.get("canonical", "")
        or not (blocker_mana["sequence"] < blocker_cast["sequence"] < oaf_mana["sequence"] < oaf_cast["sequence"] < first_strike_step["sequence"] < retaliation["sequence"] < normal_damage_step["sequence"] < prevent["sequence"])
    ):
        raise WorkerFailure("candidate_protocol_error: source/target direction or prevented and reciprocal amount is incorrect")
    mutations = report.get("mutation_sensitivity")
    mutation_keys = (
        "printed_cast_cost_mutation_detected_in_game",
        "base_power_mutation_detected_in_game",
        "lethal_toughness_mutation_detected_in_game",
        "missing_prevention_mutation_detected_in_game",
        "wrong_prevention_color_mutation_detected_in_game",
        "reversed_prevention_direction_mutation_detected_in_game",
    )
    if not isinstance(mutations, dict) or any(mutations.get(key) is not True for key in mutation_keys):
        raise WorkerFailure("candidate_protocol_error: all Indentured Oaf gameplay mutations must be detected")
    guard = report.get("candidate_source_validation")
    guard_keys = (
        "normalized_literal_source_and_ast_checked_before_candidate_calls",
        "compile_valid_injected_side_effect_fixture_compiled",
        "compile_valid_injected_side_effect_candidate_rejected",
        "injected_candidate_calls_before_rejection",
        "side_effect_sentinel_absent_after_rejection",
        "guard_ran_before_candidate_function_calls",
    )
    if (
        not isinstance(guard, dict)
        or any(guard.get(key) is not True for key in guard_keys if key != "injected_candidate_calls_before_rejection")
        or guard.get("injected_candidate_calls_before_rejection") != 0
    ):
        raise WorkerFailure("candidate_protocol_error: source guard did not reject side effects before candidate calls")


def score(workspace: Path, manifest: Path, output_root: Path, candidate_kind: str) -> int:
    output_root.mkdir(parents=True, exist_ok=True)
    manifest_data = json.loads(manifest.read_text(encoding="utf-8"))
    worker_image = os.environ.get("CARDBENCH_MAGIC_WORKER_IMAGE", DEFAULT_WORKER_IMAGE)
    if not (worker_image.startswith("rust@sha256:") or worker_image.startswith("sha256:")):
        raise ValueError("CARDBENCH_MAGIC_WORKER_IMAGE must be an immutable digest reference")
    cargo_registry = Path.home() / ".cargo" / "registry"
    if not cargo_registry.is_dir():
        raise FileNotFoundError(f"offline Cargo registry cache is missing: {cargo_registry}")
    authority = REPO / AUTHORITY
    if authority.is_symlink() or not authority.is_file():
        raise FileNotFoundError(f"trusted Indentured Oaf authority is missing: {authority}")
    if not shutil.which("docker"):
        raise FileNotFoundError("Docker is required for Magic candidate isolation")
    verify_workspace(workspace, manifest)
    candidate_hash = sha256((workspace / TARGET).read_bytes())
    name = f"cardbench-magic-indentured-oaf-{uuid.uuid4().hex[:16]}"
    command = sandbox_command(workspace, cargo_registry.resolve(), authority, name, worker_image, candidate_kind)
    code, stdout, stderr = run_bounded(command, timeout=900, name=name)
    (output_root / "worker.stdout").write_bytes(stdout)
    (output_root / "worker.stderr").write_bytes(stderr)
    if code != 0:
        result = {
            "schema_version": "cardbench.magic.card-pvp.check.v1",
            "task_id": "cardbench/magic/card",
            "candidate": {"kind": candidate_kind, "source_sha256": candidate_hash},
            "passed": False,
            "pilot_card_passed": False,
            "cards_passed": 0,
            "cards_total": 291,
            "printings_total": 306,
            "cards_credited": 0,
            "coverage_complete": False,
            "full_reward": False,
            "harbor_reward": 0.0,
            "pilot_reward": 0.0,
            "sealed_authority": False,
            "worker_image": worker_image,
            "sandbox": {"network": "none", "rootfs": "read-only", "pids_limit": 128},
            "error": "candidate compile or Indentured Oaf reference trace failed",
            "diagnostics": stderr.decode(errors="replace")[-12000:],
        }
    else:
        result = parse_report(stdout)
        result["candidate"] = {
            "kind": candidate_kind,
            "card_definition": "RAV-INDENTURED-OAF",
            "source_sha256": candidate_hash,
            "materialized_workspace_sha256": manifest_data["workspace_sha256"],
        }
        result["candidate_kind"] = candidate_kind
        result["starting_workspace_sha256"] = manifest_data["workspace_sha256"]
        result["sealed_authority"] = False
        result["worker_image"] = worker_image
        result["sandbox"] = {"network": "none", "rootfs": "read-only", "pids_limit": 128}
        result["harbor_reward"] = 0.0
        result["pilot_reward"] = 0.0
        result["full_reward"] = False
    (output_root / "result.json").write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
    (output_root / "reward.txt").write_text(f"{result['harbor_reward']:.8f}\n", encoding="utf-8")
    print(f"candidate_passed={result.get('pilot_card_passed', False)} cards={result.get('cards_passed', 0)}/291 coverage_complete=false")
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
        error_result = {
            "schema_version": "cardbench.magic.card-pvp.check.v1",
            "task_id": "cardbench/magic/card",
            "candidate": {"kind": args.candidate_kind},
            "passed": False,
            "pilot_card_passed": False,
            "cards_passed": 0,
            "cards_total": 291,
            "printings_total": 306,
            "cards_credited": 0,
            "coverage_complete": False,
            "full_reward": False,
            "harbor_reward": 0.0,
            "pilot_reward": 0.0,
            "qualification_status": "unreviewed_diagnostic",
            "error": str(error),
        }
        (args.output_root / "result.json").write_text(json.dumps(error_result, indent=2) + "\n", encoding="utf-8")
        (args.output_root / "reward.txt").write_text("0.00000000\n", encoding="utf-8")
        print(f"candidate_passed=false cards=0/291 error={error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
