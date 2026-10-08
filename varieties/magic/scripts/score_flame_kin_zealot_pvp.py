#!/usr/bin/env python3
"""Run an isolated Flame-Kin Zealot candidate and validate its full-game report."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import selectors
import shutil
import subprocess
import time
import uuid
from pathlib import Path

from materialize_flame_kin_zealot_task import verify

REPO = Path(__file__).resolve().parents[3]
TARGET = Path("varieties/magic/sets/ravnica_city_of_guilds/src/cards/flame_kin_zealot.rs")
AUTHORITY = Path("varieties/magic/session/src/bin/rav_flame_kin_zealot_pvp.rs")
BIN = "rav-flame-kin-zealot-pvp"
CARD_ID = "RAV-FLAME-KIN-ZEALOT"
DEFAULT_IMAGE = "rust@sha256:d7482085ff5b415f84dba5647ae71606650bdef00db7aeb69f4b3d170c3e4082"


class WorkerFailure(RuntimeError):
    pass


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def require(condition: bool, message: str) -> None:
    if not condition:
        raise WorkerFailure("candidate_protocol_error: " + message)


def validate_trace(trace: object) -> None:
    require(isinstance(trace, dict), "Flame-Kin Zealot trace is malformed")
    assert isinstance(trace, dict)
    require(trace.get("id") == "rav-flame-kin-zealot-etb-team-pump-haste-full-game", "wrong Flame-Kin Zealot trace identity")
    require(trace.get("passed") is True and trace.get("terminal") == "Winner(PlayerId(0))" and trace.get("winner_seat") == 0, "Flame-Kin Zealot did not finish as a seat-zero PvP win")
    require(isinstance(trace.get("seed"), int) and trace["seed"] >= 0, "Flame-Kin Zealot seed is invalid")
    require(isinstance(trace.get("turns"), int) and trace["turns"] > 0 and trace["turns"] <= 60, "Flame-Kin Zealot trace exceeded its turn bound")
    require(isinstance(trace.get("accepted_moves"), int) and trace["accepted_moves"] > 0 and trace.get("rejected_moves") == 0, "Flame-Kin Zealot trace has invalid policy move counts")

    zealot = trace.get("zealot")
    trigger = trace.get("trigger")
    membership = trace.get("resolution_membership")
    require(all(isinstance(item, dict) for item in (zealot, trigger, membership)), "Flame-Kin Zealot mechanics are missing")
    assert isinstance(zealot, dict) and isinstance(trigger, dict) and isinstance(membership, dict)
    require(zealot.get("definition_id") == CARD_ID and zealot.get("collector_number") == 206 and zealot.get("mana_cost") == "{1}{R}{R}{W}", "printed Flame-Kin Zealot identity or cost differs")
    require(zealot.get("base_power") == 2 and zealot.get("base_toughness") == 2 and zealot.get("power_after_trigger") == 3 and zealot.get("toughness_after_trigger") == 3 and zealot.get("haste_after_trigger") is True, "Flame-Kin Zealot body did not receive exactly +1/+1 and Haste")
    paid = zealot.get("mana_paid")
    require(isinstance(paid, list) and len(paid) == 4 and paid.count("Red") >= 2 and paid.count("White") >= 1, "Flame-Kin Zealot exact colored cost was not paid")
    require(trigger.get("id") == "etb-team-pump-haste" and trigger.get("mandatory") is True and trigger.get("target_count") == 0 and trigger.get("stacked") is True and trigger.get("resolved") is True, "mandatory target-free ETB trigger did not resolve")

    before = membership.get("controller_creatures_before")
    after = membership.get("controller_creatures_after")
    opponent_before = membership.get("opponent_creatures_before")
    opponent_after = membership.get("opponent_creatures_after")
    require(isinstance(before, list) and len(before) >= 2 and isinstance(after, list) and isinstance(opponent_before, list) and len(opponent_before) >= 1 and isinstance(opponent_after, list), "resolution-time creature snapshots are missing")
    before_by_id = {state.get("object"): state for state in before if isinstance(state, dict)}
    after_by_id = {state.get("object"): state for state in after if isinstance(state, dict)}
    require(len(before_by_id) == len(before) and len(after_by_id) == len(after) and set(before_by_id) == set(after_by_id), "controller creature membership changed during the ETB resolution")
    require(isinstance(membership.get("preexisting_helper_object"), int) and membership["preexisting_helper_object"] in before_by_id, "pre-existing allied creature was not present at resolution")
    for object_id, old in before_by_id.items():
        new = after_by_id[object_id]
        require(new.get("power") == old.get("power") + 1 and new.get("toughness") == old.get("toughness") + 1 and new.get("haste") is True, "a controller creature at resolution did not receive exactly +1/+1 and Haste")
    require(membership.get("all_controller_creatures_received_plus_one_plus_one_and_haste") is True, "controller team pump receipt is absent")
    require(opponent_before == opponent_after and membership.get("opposing_creatures_unchanged") is True, "Flame-Kin Zealot changed opposing creatures")

    events = trace.get("events")
    require(isinstance(events, list) and all(isinstance(event, dict) for event in events), "Flame-Kin Zealot projected event stream is malformed")
    sequences = [event.get("sequence") for event in events]
    require(all(isinstance(sequence, int) for sequence in sequences) and sequences == sorted(sequences) and len(set(sequences)) == len(sequences), "Flame-Kin Zealot event sequence is invalid")
    source = zealot.get("object")
    require(isinstance(source, int), "Flame-Kin Zealot object identity is missing")

    def find(kind: str, *, ability: str | None = None, zone: str | None = None, seat: int | None = None, source_object: bool = False) -> dict | None:
        for event in events:
            if event.get("kind") != kind:
                continue
            facts = event.get("facts", {})
            if not isinstance(facts, dict):
                continue
            if ability is not None and facts.get("ability") != ability:
                continue
            if zone is not None and facts.get("zone") != zone:
                continue
            if seat is not None and seat not in facts.get("seats", []):
                continue
            if source_object and source not in facts.get("objects", []):
                continue
            return event
        return None

    payment = find("SpellManaPaid", seat=0, source_object=True)
    cast = find("SpellCast", seat=0, source_object=True)
    resolved = find("SpellResolved", source_object=True)
    entry = find("CardMoved", zone="Battlefield", source_object=True)
    stacked = find("TriggeredAbilityStacked", seat=0, ability="etb-team-pump-haste", source_object=True)
    ability = find("AbilityResolved", ability="etb-team-pump-haste", source_object=True)
    attack = find("AttackersDeclared", seat=0, source_object=True)
    damage = find("DamageDealtToPlayer", seat=1, source_object=True)
    ended = find("GameEnded", seat=0)
    require(all(isinstance(event, dict) for event in (payment, cast, resolved, entry, stacked, ability, attack, damage, ended)), "cast, ETB, combat, or terminal events are missing")
    assert all(isinstance(event, dict) for event in (payment, cast, resolved, entry, stacked, ability, attack, damage, ended))
    facts = payment.get("facts", {})
    require(isinstance(facts, dict) and facts.get("mana_colors") == paid and len(paid) == 4, "projected spell payment differs from the trace payment")
    order = [event["sequence"] for event in (payment, cast, resolved, entry, stacked, ability, attack, damage, ended)]
    require(order == sorted(order) and len(set(order)) == len(order), "Flame-Kin Zealot cast, trigger, combat, and terminal event order is invalid")
    require(entry.get("turn") == attack.get("turn"), "newly entered Flame-Kin Zealot did not attack in the same turn")
    damage_facts = damage.get("facts", {})
    require(isinstance(damage_facts, dict) and isinstance(damage_facts.get("amount"), int) and damage_facts["amount"] > 0, "same-turn Flame-Kin Zealot attack dealt no player damage")
    require(trigger.get("stack_sequence") == stacked["sequence"] and trigger.get("resolve_sequence") == ability["sequence"] and trigger.get("same_turn_attack_sequence") == attack["sequence"] and trigger.get("same_turn_damage_sequence") == damage["sequence"], "mechanic receipts do not match the ordered event stream")


def validate_report(report: object) -> None:
    require(isinstance(report, dict), "Flame-Kin Zealot authority report is malformed")
    assert isinstance(report, dict)
    expected_card = {"collector_number": 206, "definition_id": CARD_ID, "name": "Flame-Kin Zealot"}
    require(report.get("schema_version") == "cardbench.magic.card-pvp.check.v1" and report.get("task_id") == "cardbench/magic/card", "Flame-Kin Zealot report schema or task id differs")
    require(report.get("passed") is True and report.get("pilot_card_passed") is True and report.get("deterministic_replay") is True, "Flame-Kin Zealot reference/replay failed")
    require(report.get("cards_passed") == 1 and report.get("cards_credited") == 0 and report.get("cards_total") == 291 and report.get("printings_total") == 306 and report.get("coverage_complete") is False and report.get("full_reward") is False, "Flame-Kin Zealot coverage or credit accounting differs")
    require(report.get("harbor_reward") == 0.0 and report.get("pilot_reward") == 0.0 and report.get("viewer_scope") == "Judge" and report.get("card") == expected_card, "Flame-Kin Zealot diagnostic rewards or identity differ")
    candidate = report.get("candidate")
    require(isinstance(candidate, dict) and candidate.get("card_definition") == CARD_ID, "Flame-Kin Zealot candidate identity differs")
    validation = report.get("candidate_source_validation")
    require(isinstance(validation, dict), "Flame-Kin Zealot source guard attestation is missing")
    assert isinstance(validation, dict)
    for key in ("normalized_literal_source_contract_matched", "ast_literal_shape_matched", "compile_valid_injected_side_effect_fixture_compiled", "compile_valid_injected_side_effect_candidate_rejected", "guard_ran_before_candidate_function_calls"):
        require(validation.get(key) is True, f"Flame-Kin Zealot source guard attestation failed: {key}")
    traces = report.get("traces")
    require(isinstance(traces, list) and len(traces) == 1, "exactly one Flame-Kin Zealot full-game trace is required")
    validate_trace(traces[0])
    breakdown = report.get("per_card_breakdown")
    require(isinstance(breakdown, list) and sum(1 for card in breakdown if isinstance(card, dict) and card.get("definition_id") == CARD_ID and card.get("trace_status") == "passed" and card.get("credited") is False) == 1, "Flame-Kin Zealot catalog row or zero-credit status is missing")
    mutation = report.get("mutation_sensitivity")
    required_mutations = (
        "printed_cast_cost_mutation_detected_in_game",
        "base_power_mutation_detected_in_game",
        "base_toughness_mutation_detected_in_game",
        "missing_etb_trigger_mutation_detected_in_game",
        "wrong_etb_condition_mutation_detected_in_game",
        "team_pump_removed_mutation_detected_in_game",
        "pump_amount_changed_mutation_detected_in_game",
        "haste_grant_removed_mutation_detected_in_game",
    )
    require(isinstance(mutation, dict) and all(mutation.get(key) is True for key in required_mutations) and mutation.get("sealed_mutation_authority") is False, "Flame-Kin Zealot semantic mutation coverage is incomplete")


def run_worker(workspace: Path, registry: Path, authority: Path, name: str, image: str, candidate_kind: str) -> tuple[int, bytes, bytes]:
    if authority.is_symlink() or not authority.is_file():
        raise FileNotFoundError(f"trusted Flame-Kin Zealot authority missing: {authority}")
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
        "cp -R /submission/. /work/ && cp /trusted-authority.rs /work/varieties/magic/session/src/bin/rav_flame_kin_zealot_pvp.rs && cd /work && cargo run --offline --locked -j 2 --manifest-path varieties/magic/Cargo.toml -p cardbench-magic-session --bin " + BIN + " --" + flag,
    ]
    process = subprocess.Popen(command, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    assert process.stdout is not None and process.stderr is not None
    selector = selectors.DefaultSelector()
    selector.register(process.stdout, selectors.EVENT_READ, "stdout")
    selector.register(process.stderr, selectors.EVENT_READ, "stderr")
    stdout, stderr = bytearray(), bytearray()
    started = time.monotonic()
    try:
        while selector.get_map() or process.poll() is None:
            if time.monotonic() - started > 900:
                raise WorkerFailure("candidate_timeout: Flame-Kin Zealot worker exceeded its deadline")
            for key, _ in selector.select(0.2):
                chunk = os.read(key.fileobj.fileno(), 65536)
                if not chunk:
                    selector.unregister(key.fileobj)
                    continue
                (stdout if key.data == "stdout" else stderr).extend(chunk)
                if len(stdout) + len(stderr) > 2 * 1024 * 1024:
                    raise WorkerFailure("candidate_output_limit: Flame-Kin Zealot worker output exceeded 2 MiB")
        return process.wait(), bytes(stdout), bytes(stderr)
    except BaseException:
        process.kill()
        process.wait()
        subprocess.run(["docker", "rm", "-f", name], capture_output=True, check=False)
        raise


def parse_report(stdout: bytes) -> dict:
    for line in reversed(stdout.decode(errors="replace").splitlines()):
        try:
            value = json.loads(line)
        except json.JSONDecodeError:
            continue
        if isinstance(value, dict) and value.get("schema_version") == "cardbench.magic.card-pvp.check.v1":
            validate_report(value)
            return value
    raise WorkerFailure("candidate_protocol_error: no valid Flame-Kin Zealot authority report")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--validate-report", type=Path)
    parser.add_argument("--workspace", type=Path)
    parser.add_argument("--manifest", type=Path)
    parser.add_argument("--output-root", type=Path)
    parser.add_argument("--candidate-kind", choices=("agent", "reference"), default="agent")
    args = parser.parse_args()
    if args.validate_report:
        validate_report(json.loads(args.validate_report.read_text(encoding="utf-8")))
        print("Flame-Kin Zealot report validates; rewards and credit remain zero")
        return 0
    if not args.workspace or not args.manifest or not args.output_root:
        parser.error("--workspace, --manifest, and --output-root are required unless --validate-report is used")
    output = args.output_root.resolve()
    output.mkdir(parents=True, exist_ok=True)
    workspace = args.workspace.resolve()
    manifest_path = args.manifest.resolve()
    manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    image = os.environ.get("CARDBENCH_MAGIC_WORKER_IMAGE", DEFAULT_IMAGE)
    if not (image.startswith("rust@sha256:") or image.startswith("sha256:")):
        raise ValueError("CARDBENCH_MAGIC_WORKER_IMAGE must be an immutable digest reference")
    registry = Path.home() / ".cargo" / "registry"
    authority = REPO / AUTHORITY
    if not registry.is_dir() or authority.is_symlink() or not authority.is_file() or not shutil.which("docker"):
        raise FileNotFoundError("offline Cargo registry, trusted authority, or Docker is unavailable")
    verify(workspace, manifest_path)
    snapshot = output / "candidate-workspace"
    if snapshot.is_symlink():
        raise WorkerFailure("candidate_snapshot_path_is_symlink")
    if snapshot.exists():
        shutil.rmtree(snapshot)
    snapshot.mkdir(parents=True)
    for relative in sorted(manifest["files"]):
        source = workspace / relative
        destination = snapshot / relative
        if source.is_symlink() or not source.is_file():
            raise WorkerFailure(f"candidate_workspace_source_missing: {relative}")
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(source, destination)
    source_digest = sha256((snapshot / TARGET).read_bytes())
    container = f"cardbench-magic-flame-kin-zealot-{uuid.uuid4().hex[:16]}"
    returncode, stdout, stderr = run_worker(snapshot, registry.resolve(), authority, container, image, args.candidate_kind)
    (output / "worker.stdout").write_bytes(stdout)
    (output / "worker.stderr").write_bytes(stderr)
    if returncode != 0:
        failure = {
            "schema_version": "cardbench.magic.card-pvp.check.v1", "task_id": "cardbench/magic/card",
            "candidate": {"kind": args.candidate_kind, "card_definition": CARD_ID, "source_sha256": source_digest},
            "passed": False, "pilot_card_passed": False, "cards_passed": 0, "cards_total": 291,
            "printings_total": 306, "cards_credited": 0, "coverage_complete": False, "full_reward": False,
            "harbor_reward": 0.0, "pilot_reward": 0.0, "sealed_authority": False,
            "error": "candidate compile or Flame-Kin Zealot reference trace failed",
            "diagnostics": stderr.decode(errors="replace")[-12000:],
        }
        (output / "result.json").write_text(json.dumps(failure, indent=2) + "\n", encoding="utf-8")
        (output / "reward.txt").write_text("0.00000000\n", encoding="utf-8")
        print(json.dumps(failure))
        return 1
    report = parse_report(stdout)
    report["candidate"].update({"kind": args.candidate_kind, "card_definition": CARD_ID, "source_sha256": source_digest, "materialized_workspace_sha256": manifest["workspace_sha256"]})
    report.update({"sealed_authority": False, "worker_image": image, "sandbox": {"network": "none", "rootfs": "read-only", "pids_limit": 128}, "harbor_reward": 0.0, "pilot_reward": 0.0, "cards_credited": 0})
    (output / "result.json").write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    (output / "reward.txt").write_text("0.00000000\n", encoding="utf-8")
    print(json.dumps(report))
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except Exception as error:
        raise SystemExit(f"Flame-Kin Zealot scoring failed: {error}")
