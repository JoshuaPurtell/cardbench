#!/usr/bin/env python3
"""Run Galvanic Arc in an offline, resource-limited candidate worker."""

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
MATERIALIZER = Path(__file__).with_name("materialize_galvanic_arc_task.py")
TARGET = Path("varieties/magic/sets/ravnica_city_of_guilds/src/cards/galvanic_arc.rs")
AUTHORITY_SOURCE = Path("varieties/magic/session/src/bin/rav_galvanic_arc_pvp.rs")
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


def make_scoring_snapshot(workspace: Path, files: dict[str, str], snapshot: Path) -> Path:
    if snapshot.is_symlink():
        raise WorkerFailure("candidate_snapshot_path_is_symlink")
    if snapshot.exists():
        shutil.rmtree(snapshot)
    snapshot.mkdir(parents=True)
    for relative in sorted(files):
        source = workspace / Path(relative)
        if source.is_symlink() or not source.is_file():
            raise WorkerFailure(f"candidate_snapshot_source_missing: {relative}")
        destination = snapshot / Path(relative)
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(source, destination)
    return snapshot


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
                target = output if key.data == "stdout" else diagnostics
                target.extend(chunk)
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
        "cp /trusted-authority.rs /work/varieties/magic/session/src/bin/rav_galvanic_arc_pvp.rs && "
        "cd /work && cargo run --offline --locked -j 2 --manifest-path varieties/magic/Cargo.toml "
        "-p cardbench-magic-session --bin rav-galvanic-arc-pvp --" + candidate_flag,
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
    raise WorkerFailure("candidate_protocol_error: no valid Galvanic Arc PvP result")


def validate_report(report: dict) -> None:
    expected_card = {"collector_number": 126, "definition_id": "RAV-GALVANIC-ARC", "name": "Galvanic Arc"}
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
    ):
        raise WorkerFailure("candidate_protocol_error: incomplete Galvanic Arc zero-credit single-card report")
    candidate = report.get("candidate")
    if not isinstance(candidate, dict) or candidate.get("card_definition") != "RAV-GALVANIC-ARC":
        raise WorkerFailure("candidate_protocol_error: wrong Galvanic Arc candidate identity")
    traces = report.get("traces")
    if not isinstance(traces, list) or len(traces) != 1 or traces[0].get("trace_id") != "rav-galvanic-arc-etb-first-strike-full-game":
        raise WorkerFailure("candidate_protocol_error: exactly one Galvanic Arc full-game trace is required")
    trace = traces[0]
    if trace.get("terminal") != "Winner(PlayerId(0))" or trace.get("winner_seat") != 0 or trace.get("rejected_moves") != 0:
        raise WorkerFailure("candidate_protocol_error: shuffled two-player game did not terminate cleanly")
    aura = trace.get("aura")
    etb = trace.get("etb")
    combat = trace.get("combat")
    payment = trace.get("mana_paid")
    lives = trace.get("players_life_at_game_end")
    integer_fields = (trace.get("aura_object"), trace.get("attached_creature"), trace.get("etb_target"))
    if (
        not all(isinstance(value, int) for value in integer_fields)
        or len(set(integer_fields)) != 3
        or trace.get("mana_cost") != "{2}{R}"
        or not isinstance(payment, list) or len(payment) != 3 or payment.count("Red") != 1
        or aura != {"kind": "Aura", "target_kind": "Creature", "attached_to_controller": 0, "derived_keyword": "FirstStrike", "derived_keyword_present": True}
        or trace.get("etb_target_definition") != "RAV-WATCHWOLF"
        or etb != {"ability": "etb-deal-three", "mandatory": True, "target_kind": "opposing-creature", "damage": 3, "target_zone_after_resolution": "Graveyard"}
        or not isinstance(combat, dict)
        or combat.get("first_strike_step") is not True
        or combat.get("attacker") != trace.get("attached_creature")
        or not isinstance(combat.get("blocker"), int)
        or combat.get("attacker_damage") != 3
        or combat.get("blocker_removed_before_normal_damage") is not True
        or not isinstance(lives, list) or len(lives) != 2
        or lives[0].get("seat") != 0 or not isinstance(lives[0].get("life"), int) or lives[0]["life"] <= 0
        or lives[1].get("seat") != 1 or not isinstance(lives[1].get("life"), int) or lives[1]["life"] > 0
        or not isinstance(trace.get("shuffle_seed"), int)
    ):
        raise WorkerFailure("candidate_protocol_error: Galvanic Arc cost, attachment, ETB damage, combat, or terminal facts differ")
    events = trace.get("events")
    expected = [
        "SpellManaPaid", "SpellCast", "SpellResolved", "CardMoved", "AuraAttached",
        "TriggeredAbilityStacked", "DamageDealtToPermanent", "AbilityResolved",
        "StateBasedAction", "CardMoved", "AttackersDeclared", "BlockersDeclared",
        "StepBegan", "DamageDealtToPermanent", "StateBasedAction", "CardMoved",
        "StepBegan", "GameEnded",
    ]
    if not isinstance(events, list) or [event.get("kind") for event in events if isinstance(event, dict)] != expected:
        raise WorkerFailure("candidate_protocol_error: ordered Aura, trigger, and first-strike event trace differs")
    paid, cast, resolved, aura_entry, attached, stacked, damage, trigger_done, target_sba, target_move, attack, blocks, first_step, combat_damage, blocker_sba, blocker_move, normal_step, ended = events
    aura_id, host_id, target_id = integer_fields
    blocker_id = combat.get("blocker")
    if (
        paid.get("facts", {}).get("objects") != [aura_id]
        or paid.get("facts", {}).get("seats") != [0]
        or paid.get("facts", {}).get("mana_colors") != payment
        or cast.get("facts", {}).get("objects") != [aura_id]
        or resolved.get("facts", {}).get("objects") != [aura_id]
        or aura_entry.get("facts", {}).get("objects") != [aura_id]
        or "aura: ObjectId(" + str(aura_id) + ")" not in attached.get("canonical", "")
        or "target: ObjectId(" + str(host_id) + ")" not in attached.get("canonical", "")
        or stacked.get("facts", {}).get("objects") != [aura_id]
        or stacked.get("facts", {}).get("ability") != "etb-deal-three"
        or damage.get("facts", {}).get("objects") != [aura_id, target_id]
        or damage.get("facts", {}).get("amount") != 3
        or trigger_done.get("facts", {}).get("ability") != "etb-deal-three"
        or target_sba.get("facts", {}).get("objects") != [target_id]
        or target_move.get("facts", {}).get("objects") != [target_id]
        or target_move.get("facts", {}).get("zone") != "Graveyard"
        or attack.get("facts", {}).get("seats") != [0]
        or host_id not in attack.get("facts", {}).get("objects", [])
        or blocks.get("facts", {}).get("seats") != [1]
        or host_id not in blocks.get("facts", {}).get("objects", [])
        or blocker_id not in blocks.get("facts", {}).get("objects", [])
        or first_step.get("facts", {}).get("step") != "FirstStrikeCombatDamage"
        or combat_damage.get("facts", {}).get("objects") != [host_id, blocker_id]
        or combat_damage.get("facts", {}).get("amount") != 3
        or blocker_sba.get("facts", {}).get("objects") != [blocker_id]
        or blocker_move.get("facts", {}).get("objects") != [blocker_id]
        or blocker_move.get("facts", {}).get("zone") != "Graveyard"
        or normal_step.get("facts", {}).get("step") != "CombatDamage"
        or ended.get("facts", {}).get("seats") != [0]
    ):
        raise WorkerFailure("candidate_protocol_error: Galvanic Arc object identities or ordered event facts differ")
    sequence = [event.get("sequence") for event in events]
    if any(not isinstance(value, int) for value in sequence) or sequence != sorted(sequence):
        raise WorkerFailure("candidate_protocol_error: event sequence is malformed")
    mutations = report.get("mutation_sensitivity")
    required = (
        "wrong_damage_amount_mutation_detected_in_game",
        "etb_trigger_removal_detected_in_game",
        "first_strike_attachment_mutation_detected_in_game",
        "red_mana_symbol_mutation_detected_in_game",
    )
    if not isinstance(mutations, dict) or any(mutations.get(key) is not True for key in required):
        raise WorkerFailure("candidate_protocol_error: all four gameplay mutations must be detected")
    guard = report.get("candidate_source_validation")
    if not isinstance(guard, dict) or any(guard.get(key) is not True for key in (
        "normalized_literal_source_and_ast_checked_before_candidate_calls",
        "compile_valid_injected_side_effect_fixture_compiled_by_construction",
        "compile_valid_injected_side_effect_candidate_rejected",
        "guard_ran_before_candidate_function_calls",
    )) or guard.get("injected_candidate_calls_before_rejection") != 0:
        raise WorkerFailure("candidate_protocol_error: source guard did not reject side effects before candidate calls")
    rules = report.get("rules_authority")
    if not isinstance(rules, dict) or rules.get("comprehensive_rules") != "2026-08-07":
        raise WorkerFailure("candidate_protocol_error: Galvanic Arc rules date is missing")

def score(workspace: Path, manifest: Path, output_root: Path, candidate_kind: str) -> int:
    output_root.mkdir(parents=True, exist_ok=True)
    manifest_data = json.loads(manifest.read_text(encoding="utf-8"))
    worker_image = os.environ.get("CARDBENCH_MAGIC_WORKER_IMAGE", DEFAULT_WORKER_IMAGE)
    if not (worker_image.startswith("rust@sha256:") or worker_image.startswith("sha256:")):
        raise ValueError("CARDBENCH_MAGIC_WORKER_IMAGE must be an immutable digest reference")
    cargo_registry = Path.home() / ".cargo" / "registry"
    if not cargo_registry.is_dir():
        raise FileNotFoundError(f"offline Cargo registry cache is missing: {cargo_registry}")
    authority = REPO / AUTHORITY_SOURCE
    if authority.is_symlink() or not authority.is_file():
        raise FileNotFoundError(f"trusted Galvanic Arc authority is missing: {authority}")
    if not shutil.which("docker"):
        raise FileNotFoundError("Docker is required for Magic candidate isolation")
    verify_workspace(workspace, manifest)
    scoring_workspace = make_scoring_snapshot(workspace, manifest_data["files"], output_root / "candidate-workspace")
    candidate_hash = sha256((scoring_workspace / TARGET).read_bytes())
    name = f"cardbench-magic-galvanic-arc-{uuid.uuid4().hex[:16]}"
    command = sandbox_command(scoring_workspace, cargo_registry.resolve(), authority, name, worker_image, candidate_kind)
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
            "sealed_authority": False,
            "worker_image": worker_image,
            "sandbox": {"network": "none", "rootfs": "read-only", "pids_limit": 128},
            "error": "candidate compile or Galvanic Arc reference trace failed",
            "diagnostics": stderr.decode(errors="replace")[-12000:],
        }
    else:
        result = parse_report(stdout)
        result["candidate"] = {
            "kind": candidate_kind,
            "card_definition": "RAV-GALVANIC-ARC",
            "source_sha256": candidate_hash,
            "materialized_workspace_sha256": manifest_data["workspace_sha256"],
        }
        result["candidate_kind"] = candidate_kind
        result["starting_workspace_sha256"] = manifest_data["workspace_sha256"]
        result["sealed_authority"] = False
        result["worker_image"] = worker_image
        result["sandbox"] = {"network": "none", "rootfs": "read-only", "pids_limit": 128}
        result["harbor_reward"] = 0.0
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
        result = {
            "schema_version": "cardbench.magic.card-pvp.check.v1",
            "task_id": "cardbench/magic/card",
            "passed": False,
            "pilot_card_passed": False,
            "cards_passed": 0,
            "cards_total": 291,
            "printings_total": 306,
            "cards_credited": 0,
            "coverage_complete": False,
            "full_reward": False,
            "harbor_reward": 0.0,
            "sealed_authority": False,
            "error": str(error),
        }
        (args.output_root / "result.json").write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
        (args.output_root / "reward.txt").write_text("0.0\n", encoding="utf-8")
        print(f"Magic candidate verifier failed: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
