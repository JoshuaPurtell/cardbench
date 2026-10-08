#!/usr/bin/env python3
"""Run Golgari Thug in a bounded offline worker and validate its full-game trace."""

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
MATERIALIZER = Path(__file__).with_name("materialize_golgari_thug_task.py")
TARGET = Path("varieties/magic/sets/ravnica_city_of_guilds/src/cards/golgari_thug.rs")
AUTHORITY = Path("varieties/magic/session/src/bin/rav_golgari_thug_pvp.rs")
IMAGE = "rust@sha256:d7482085ff5b415f84dba5647ae71606650bdef00db7aeb69f4b3d170c3e4082"
MAX_OUTPUT = 2 * 1024 * 1024


class WorkerFailure(RuntimeError):
    pass


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def verify_workspace(workspace: Path, manifest: Path) -> None:
    subprocess.run([sys.executable, str(MATERIALIZER), "--workspace", str(workspace), "--manifest", str(manifest), "--verify"], check=True, cwd=REPO)


def snapshot_workspace(workspace: Path, files: dict[str, str], target_root: Path) -> Path:
    if target_root.is_symlink():
        raise WorkerFailure("candidate_snapshot_path_is_symlink")
    if target_root.exists():
        shutil.rmtree(target_root)
    target_root.mkdir(parents=True)
    for relative in sorted(files):
        source = workspace / relative
        if source.is_symlink() or not source.is_file():
            raise WorkerFailure(f"candidate_snapshot_source_missing: {relative}")
        destination = target_root / relative
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(source, destination)
    return target_root


def run_bounded(command: list[str], timeout: int, container: str) -> tuple[int, bytes, bytes]:
    process = subprocess.Popen(command, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    assert process.stdout is not None and process.stderr is not None
    selector = selectors.DefaultSelector()
    selector.register(process.stdout, selectors.EVENT_READ, "stdout")
    selector.register(process.stderr, selectors.EVENT_READ, "stderr")
    out, err = bytearray(), bytearray()
    started = time.monotonic()
    try:
        while selector.get_map() or process.poll() is None:
            if time.monotonic() - started > timeout:
                raise WorkerFailure("candidate_timeout: Docker worker exceeded its deadline")
            for key, _ in selector.select(0.2):
                chunk = os.read(key.fileobj.fileno(), 65536)
                if not chunk:
                    selector.unregister(key.fileobj)
                    continue
                (out if key.data == "stdout" else err).extend(chunk)
                if len(out) + len(err) > MAX_OUTPUT:
                    raise WorkerFailure("candidate_output_limit: worker output exceeded 2 MiB")
        return process.wait(), bytes(out), bytes(err)
    except BaseException:
        process.kill()
        process.wait()
        subprocess.run(["docker", "rm", "-f", container], capture_output=True, check=False)
        raise


def sandbox_command(workspace: Path, registry: Path, authority: Path, container: str, image: str, candidate_kind: str) -> list[str]:
    if authority.is_symlink() or not authority.is_file():
        raise WorkerFailure("trusted_golgari_thug_authority_missing")
    candidate = " --candidate" if candidate_kind == "agent" else ""
    return [
        "docker", "run", "--rm", "--name", container, "--network", "none", "--read-only",
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
        "cp /trusted-authority.rs /work/varieties/magic/session/src/bin/rav_golgari_thug_pvp.rs && "
        "cd /work && cargo run --offline --locked -j 2 --manifest-path varieties/magic/Cargo.toml "
        "-p cardbench-magic-session --bin rav-golgari-thug-pvp --" + candidate,
    ]


def validate_trace(trace: object) -> None:
    if (not isinstance(trace, dict)
        or trace.get("id") != "rav-golgari-thug-dies-target-dredge-four-terminal-game"
        or trace.get("passed") is not True
        or trace.get("terminal") != "Winner(PlayerId(0))"
        or trace.get("winner_seat") != 0
        or trace.get("rejected_moves") != 0):
        raise WorkerFailure("candidate_protocol_error: Golgari Thug did not reach the expected terminal duel result")
    if (not isinstance(trace.get("seed"), int) or trace["seed"] < 12
        or not isinstance(trace.get("digest"), str) or not trace["digest"]):
        raise WorkerFailure("candidate_protocol_error: Golgari Thug trace lacks its deterministic seed or replay digest")
    if (not isinstance(trace.get("turns"), int) or not 0 < trace["turns"] <= 120
        or not isinstance(trace.get("accepted_moves"), int) or not 0 < trace["accepted_moves"] <= 8000):
        raise WorkerFailure("candidate_protocol_error: Golgari Thug game bounds or accepted-move receipt is invalid")
    thug, combat, trigger, dredge = (trace.get(key) for key in ("thug", "combat", "trigger", "dredge"))
    thug_id, brownscale_id, watchwolf_id = (trace.get(key) for key in ("thug_object", "brownscale_object", "watchwolf_object"))
    if (not all(isinstance(value, int) for value in (thug_id, brownscale_id, watchwolf_id))
        or not all(isinstance(value, dict) for value in (thug, combat, trigger, dredge))):
        raise WorkerFailure("candidate_protocol_error: Golgari Thug object identities or mechanic details are missing")
    paid_colors = thug.get("paid_colors")
    if (thug.get("mana_cost") != "{1}{B}" or thug.get("power") != 1 or thug.get("toughness") != 1
        or thug.get("dredge") != 4 or thug.get("printed_subtypes") != ["Human", "Warrior"]
        or thug.get("subtypes_asserted") is not False or not isinstance(paid_colors, list)
        or len(paid_colors) != 2 or "Black" not in paid_colors):
        raise WorkerFailure("candidate_protocol_error: Golgari Thug printed identity/cost/characteristics differ")
    if (combat.get("setup_object") != brownscale_id or combat.get("source_object") != thug_id
        or combat.get("source_incarnation") is None or combat.get("setup_watchwolf_damage") != 3
        or combat.get("setup_brownscale_damage") != 2 or combat.get("brownscale_died") is not True
        or combat.get("watchwolf_damage") != 3 or combat.get("thug_damage") != 1
        or combat.get("thug_died") is not True):
        raise WorkerFailure("candidate_protocol_error: exact Brownscale/Thug combat deaths and damage are missing")
    if (trigger.get("actor_seat") != 0 or trigger.get("source_object") != thug_id
        or trigger.get("target_object") != brownscale_id
        or trigger.get("target_moved_to_library_top") != brownscale_id
        or trigger.get("source_incarnation") != combat.get("source_incarnation")):
        raise WorkerFailure("candidate_protocol_error: Thug controller did not choose the exact graveyard target")
    mills = dredge.get("mill_objects")
    if (dredge.get("card_object") != thug_id or dredge.get("count") != 4
        or dredge.get("top_target_milled_first") is not True
        or dredge.get("same_object_returned_to_hand") is not True
        or dredge.get("same_object_recast") is not True
        or not isinstance(mills, list) or len(mills) != 4
        or len(set(mills)) != 4 or mills[0] != brownscale_id or thug_id in mills):
        raise WorkerFailure("candidate_protocol_error: exact Dredge 4 mill order, return, and recast are missing")
    events = trace.get("events")
    if not isinstance(events, list) or not all(isinstance(event, dict) for event in events):
        raise WorkerFailure("candidate_protocol_error: projected Golgari Thug events are malformed")
    sequences = [event.get("sequence") for event in events]
    if any(not isinstance(sequence, int) for sequence in sequences) or sequences != sorted(sequences) or len(set(sequences)) != len(sequences):
        raise WorkerFailure("candidate_protocol_error: projected event sequence is malformed")
    if trace.get("ordered_event_sequence") != sequences:
        raise WorkerFailure("candidate_protocol_error: ordered event sequence differs from projected receipts")
    names = ("brownscale_cast", "brownscale_resolved", "brownscale_watchwolf_attack", "brownscale_block", "brownscale_damage", "brownscale_death", "thug_cast", "thug_resolved", "watchwolf_attack", "thug_block", "thug_damage", "thug_death", "trigger_stacked", "trigger_resolved", "target_to_library_top", "thug_returned_to_hand", "dredged", "thug_recast", "thug_recast_resolved", "terminal")
    positions = trace.get("event_positions")
    if (not isinstance(positions, dict) or any(not isinstance(positions.get(name), int) for name in names)
        or len({positions[name] for name in names}) != len(names)):
        raise WorkerFailure("candidate_protocol_error: Golgari Thug ordered event positions are missing")
    ordered = [positions[name] for name in names]
    if ordered != sorted(ordered):
        raise WorkerFailure("candidate_protocol_error: combat, trigger target, Dredge, recast, and terminal order differs")
    by_sequence = {event["sequence"]: event for event in events}
    expected_kinds = {
        "brownscale_cast": "SpellCast", "brownscale_resolved": "SpellResolved",
        "brownscale_watchwolf_attack": "AttackersDeclared", "brownscale_block": "BlockersDeclared",
        "brownscale_damage": "DamageDealtToPermanent", "brownscale_death": "CardMoved",
        "thug_cast": "SpellCast", "thug_resolved": "SpellResolved", "watchwolf_attack": "AttackersDeclared",
        "thug_block": "BlockersDeclared", "thug_damage": "DamageDealtToPermanent", "thug_death": "CardMoved",
        "trigger_stacked": "TriggeredAbilityStacked", "trigger_resolved": "AbilityResolved",
        "target_to_library_top": "CardMoved", "thug_returned_to_hand": "CardMoved", "dredged": "Dredged",
        "thug_recast": "SpellCast", "thug_recast_resolved": "SpellResolved", "terminal": "GameEnded",
    }
    receipts = {}
    for name, kind in expected_kinds.items():
        receipt = by_sequence.get(positions[name])
        if receipt is None or receipt.get("kind") != kind:
            raise WorkerFailure(f"candidate_protocol_error: ordered {name} receipt does not match {kind}")
        receipts[name] = receipt
    def canonical(name: str) -> str:
        return str(receipts[name].get("canonical", ""))

    def has_move(name: str, object_id: int, zone: str) -> bool:
        line = canonical(name)
        return f"card: ObjectId({object_id})" in line and f"to: {zone}" in line

    brownscale_watchwolf_hit = [str(event.get("canonical", "")) for event in events
        if event.get("kind") == "DamageDealtToPermanent"
        and f"source: ObjectId({watchwolf_id})" in str(event.get("canonical", ""))
        and f"permanent: ObjectId({brownscale_id})" in str(event.get("canonical", ""))]
    brownscale_return_hit = [str(event.get("canonical", "")) for event in events
        if event.get("kind") == "DamageDealtToPermanent"
        and f"source: ObjectId({brownscale_id})" in str(event.get("canonical", ""))
        and f"permanent: ObjectId({watchwolf_id})" in str(event.get("canonical", ""))]
    thug_watchwolf_hit = [str(event.get("canonical", "")) for event in events
        if event.get("kind") == "DamageDealtToPermanent"
        and f"source: ObjectId({thug_id})" in str(event.get("canonical", ""))
        and f"permanent: ObjectId({watchwolf_id})" in str(event.get("canonical", ""))]
    watchwolf_thug_hit = canonical("thug_damage")
    thug_spell_payments = [str(event.get("canonical", "")) for event in events
        if event.get("kind") == "SpellManaPaid"
        and f"card: ObjectId({thug_id})" in str(event.get("canonical", ""))]
    if (not has_move("brownscale_death", brownscale_id, "Graveyard")
        or not has_move("thug_death", thug_id, "Graveyard")
        or not has_move("target_to_library_top", brownscale_id, "Library")
        or not has_move("thug_returned_to_hand", thug_id, "Hand")
        or f"source: ObjectId({watchwolf_id})" not in watchwolf_thug_hit
        or f"permanent: ObjectId({thug_id})" not in watchwolf_thug_hit
        or "amount: 3" not in watchwolf_thug_hit
        or not any(f"amount: 3" in line for line in brownscale_watchwolf_hit)
        or not any(f"amount: 2" in line for line in brownscale_return_hit)
        or not any(f"amount: 1" in line for line in thug_watchwolf_hit)
        or len(thug_spell_payments) != 2
        or any("Black" not in line for line in thug_spell_payments)
        or f"count: 4" not in canonical("dredged")
        or f"card: ObjectId({thug_id})" not in canonical("dredged")
        or f"source: ObjectId({thug_id})" not in canonical("trigger_stacked")
        or f"source: ObjectId({thug_id})" not in canonical("trigger_resolved")
        or f"source_incarnation: {combat['source_incarnation']}" not in canonical("trigger_stacked")
        or f"source_incarnation: {combat['source_incarnation']}" not in canonical("trigger_resolved")
        or f"card: ObjectId({thug_id})" not in canonical("thug_cast")
        or f"card: ObjectId({thug_id})" not in canonical("thug_recast")):
        raise WorkerFailure("candidate_protocol_error: exact combat, zone, trigger-source, or Dredge provenance is missing")
    mill_sequences = []
    for mill in mills:
        matches = [event.get("sequence") for event in events if event.get("kind") == "CardMoved"
            and event.get("sequence", -1) > positions["target_to_library_top"]
            and event.get("sequence", -1) < positions["thug_returned_to_hand"]
            and "to: Graveyard" in str(event.get("canonical", ""))
            and f"card: ObjectId({mill})" in str(event.get("canonical", ""))]
        if not matches:
            raise WorkerFailure("candidate_protocol_error: an exact object in the four-card Dredge mill is missing")
        mill_sequences.append(matches[0])
    if mill_sequences != sorted(mill_sequences) or mill_sequences[0] <= positions["target_to_library_top"]:
        raise WorkerFailure("candidate_protocol_error: the four Dredge cards are not milled in order after the target reaches library top")


def validate_report(report: dict) -> None:
    card = {"collector_number": 87, "definition_id": "RAV-GOLGARI-THUG", "name": "Golgari Thug"}
    if (report.get("schema_version") != "cardbench.magic.card-pvp.check.v1" or report.get("task_id") != "cardbench/magic/card"
        or report.get("passed") is not True or report.get("pilot_card_passed") is not True or report.get("cards_passed") != 1
        or report.get("cards_credited") != 0 or report.get("cards_total") != 291 or report.get("printings_total") != 306
        or report.get("coverage_complete") is not False or report.get("full_reward") is not False
        or report.get("harbor_reward") != 0.0 or report.get("pilot_reward") != 0.0
        or report.get("viewer_scope") != "Judge" or report.get("card") != card):
        raise WorkerFailure("candidate_protocol_error: incomplete Golgari Thug zero-credit diagnostic report")
    source = report.get("candidate_source_validation")
    if (not isinstance(source, dict) or source.get("candidate_ast_contract_matched") is not True
        or source.get("compile_valid_injected_side_effect_candidate_rejected") is not True
        or source.get("guard_ran_before_candidate_function_calls") is not True):
        raise WorkerFailure("candidate_protocol_error: Golgari Thug source-gate attestations are incomplete")
    traces = report.get("traces")
    if not isinstance(traces, list) or len(traces) != 1:
        raise WorkerFailure("candidate_protocol_error: exactly one Golgari Thug full-game trace is required")
    validate_trace(traces[0])
    breakdown = report.get("per_card_breakdown")
    if not isinstance(breakdown, list) or sum(1 for row in breakdown if isinstance(row, dict)
        and row.get("collector_number") == 87 and row.get("name") == "Golgari Thug"
        and row.get("definition_id") == "RAV-GOLGARI-THUG" and row.get("trace_status") == "passed" and row.get("credited") is False) != 1:
        raise WorkerFailure("candidate_protocol_error: Golgari Thug RAV catalog identity/zero-credit row is missing")
    required = ("wrong_identity_mutation_detected", "wrong_mana_cost_mutation_detected", "wrong_creature_body_mutation_detected", "missing_dredge_mutation_detected", "wrong_dredge_count_mutation_detected", "graveyard_trigger_removed_mutation_detected", "graveyard_trigger_condition_changed_mutation_detected", "graveyard_trigger_target_changed_mutation_detected", "graveyard_trigger_effect_changed_mutation_detected", "compile_valid_candidate_side_effect_rejected_before_execution")
    mutation = report.get("mutation_sensitivity")
    if not isinstance(mutation, dict) or any(mutation.get(key) is not True for key in required):
        raise WorkerFailure("candidate_protocol_error: Golgari Thug semantic mutation controls are incomplete")


def parse_report(stdout: bytes) -> dict:
    for line in reversed(stdout.decode(errors="replace").splitlines()):
        try:
            report = json.loads(line)
        except json.JSONDecodeError:
            continue
        if report.get("schema_version") == "cardbench.magic.card-pvp.check.v1":
            validate_report(report)
            return report
    raise WorkerFailure("candidate_protocol_error: no valid Golgari Thug PvP report")


def score(workspace: Path, manifest: Path, output_root: Path, candidate_kind: str) -> int:
    output_root.mkdir(parents=True, exist_ok=True)
    data = json.loads(manifest.read_text(encoding="utf-8"))
    image = os.environ.get("CARDBENCH_MAGIC_WORKER_IMAGE", IMAGE)
    if not (image.startswith("rust@sha256:") or image.startswith("sha256:")):
        raise ValueError("CARDBENCH_MAGIC_WORKER_IMAGE must be an immutable digest reference")
    registry = Path.home() / ".cargo" / "registry"
    if not registry.is_dir():
        raise FileNotFoundError(f"offline Cargo registry cache is missing: {registry}")
    authority = REPO / AUTHORITY
    if authority.is_symlink() or not authority.is_file():
        raise FileNotFoundError(f"trusted Golgari Thug authority is missing: {authority}")
    if not shutil.which("docker"):
        raise FileNotFoundError("Docker is required for Magic candidate isolation")
    verify_workspace(workspace, manifest)
    snapshot = snapshot_workspace(workspace, data["files"], output_root / "candidate-workspace")
    digest = sha256((snapshot / TARGET).read_bytes())
    container = f"cardbench-magic-golgari-thug-{uuid.uuid4().hex[:16]}"
    code, stdout, stderr = run_bounded(sandbox_command(snapshot, registry.resolve(), authority, container, image, candidate_kind), 900, container)
    (output_root / "worker.stdout").write_bytes(stdout)
    (output_root / "worker.stderr").write_bytes(stderr)
    if code != 0:
        result = {"schema_version": "cardbench.magic.card-pvp.check.v1", "task_id": "cardbench/magic/card", "candidate": {"kind": candidate_kind, "source_sha256": digest}, "passed": False, "pilot_card_passed": False, "cards_passed": 0, "cards_total": 291, "printings_total": 306, "cards_credited": 0, "coverage_complete": False, "full_reward": False, "harbor_reward": 0.0, "pilot_reward": 0.0, "sealed_authority": False, "worker_image": image, "sandbox": {"network": "none", "rootfs": "read-only", "pids_limit": 128}, "error": "candidate compile or Golgari Thug reference trace failed", "diagnostics": stderr.decode(errors="replace")[-12000:]}
        (output_root / "result.json").write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
        print(json.dumps(result))
        return 1
    report = parse_report(stdout)
    report["candidate"].update({"kind": candidate_kind, "source_sha256": digest})
    report.update({"sealed_authority": False, "worker_image": image, "sandbox": {"network": "none", "rootfs": "read-only", "pids_limit": 128}})
    (output_root / "result.json").write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    print(json.dumps(report))
    return 0


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--workspace", required=True, type=Path)
    parser.add_argument("--manifest", required=True, type=Path)
    parser.add_argument("--output-root", required=True, type=Path)
    parser.add_argument("--candidate-kind", choices=("agent", "reference"), default="agent")
    args = parser.parse_args()
    return score(args.workspace.resolve(), args.manifest.resolve(), args.output_root.resolve(), args.candidate_kind)


if __name__ == "__main__":
    raise SystemExit(main())
