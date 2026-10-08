#!/usr/bin/env python3
"""Run Stinkweed Imp in a bounded offline worker and validate its full-game trace."""

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
MATERIALIZER = Path(__file__).with_name("materialize_stinkweed_imp_task.py")
TARGET = Path("varieties/magic/sets/ravnica_city_of_guilds/src/cards/stinkweed_imp.rs")
AUTHORITY = Path("varieties/magic/session/src/bin/rav_stinkweed_imp_pvp.rs")
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
        raise WorkerFailure("trusted_stinkweed_imp_authority_missing")
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
        "cp /trusted-authority.rs /work/varieties/magic/session/src/bin/rav_stinkweed_imp_pvp.rs && "
        "cd /work && cargo run --offline --locked -j 2 --manifest-path varieties/magic/Cargo.toml "
        "-p cardbench-magic-session --bin rav-stinkweed-imp-pvp --" + candidate,
    ]


def validate_trace(trace: object) -> None:
    if not isinstance(trace, dict) or trace.get("id") != "rav-stinkweed-imp-dredge-five-destroys-damaged-creature" or trace.get("passed") is not True or trace.get("terminal") != "Winner(PlayerId(0))" or trace.get("winner_seat") != 0 or trace.get("rejected_moves") != 0:
        raise WorkerFailure("candidate_protocol_error: Stinkweed Imp did not reach the expected terminal duel result")
    if trace.get("seed") != 0 or not isinstance(trace.get("digest"), str) or not trace["digest"]:
        raise WorkerFailure("candidate_protocol_error: Stinkweed Imp trace lacks fixed seed or replay digest")
    if not isinstance(trace.get("turns"), int) or not 0 < trace["turns"] <= 80 or not isinstance(trace.get("accepted_moves"), int) or trace["accepted_moves"] <= 0:
        raise WorkerFailure("candidate_protocol_error: Stinkweed Imp game bounds or accepted-move receipt is invalid")
    imp, combat, dredge = trace.get("imp"), trace.get("combat"), trace.get("dredge")
    if not isinstance(trace.get("imp_object"), int) or not isinstance(trace.get("target_object"), int) or not all(isinstance(x, dict) for x in (imp, combat, dredge)):
        raise WorkerFailure("candidate_protocol_error: Stinkweed Imp identity or mechanic details are missing")
    if (imp.get("mana_cost") != "{2}{B}" or imp.get("power") != 1 or imp.get("toughness") != 2
        or imp.get("flying") is not True or imp.get("dredge") != 5
        or imp.get("printed_subtype") != "Imp" or imp.get("subtype_asserted") is not False
        or not isinstance(imp.get("paid_colors"), list) or len(imp["paid_colors"]) != 3 or "Black" not in imp["paid_colors"]):
        raise WorkerFailure("candidate_protocol_error: Stinkweed Imp identity/cost/characteristics differ")
    if (combat.get("damaged_target_object") != trace["target_object"] or combat.get("destroyed_target_object") != trace["target_object"]
        or combat.get("damage_amount") != 1 or combat.get("target_graveyard") is not True
        or combat.get("source_object") != trace["imp_object"] or not isinstance(combat.get("source_incarnation"), int)):
        raise WorkerFailure("candidate_protocol_error: combat damage did not destroy the exact Watchwolf recipient")
    mills = dredge.get("mill_objects")
    if (dredge.get("card_object") != trace["imp_object"] or dredge.get("count") != 5
        or dredge.get("same_object_returned_to_hand") is not True or dredge.get("same_object_recast") is not True
        or not isinstance(mills, list) or len(mills) != 5 or len(set(mills)) != 5
        or dredge.get("library_before") != dredge.get("library_after", -1) + 5):
        raise WorkerFailure("candidate_protocol_error: exact Dredge 5 replacement and same-object return/recast are missing")
    events = trace.get("events")
    if not isinstance(events, list) or not all(isinstance(event, dict) for event in events):
        raise WorkerFailure("candidate_protocol_error: Stinkweed Imp projected events are malformed")
    sequences = [event.get("sequence") for event in events]
    if any(not isinstance(sequence, int) for sequence in sequences) or sequences != sorted(sequences) or len(set(sequences)) != len(sequences):
        raise WorkerFailure("candidate_protocol_error: projected event sequence is malformed")
    if trace.get("ordered_event_sequence") != sequences:
        raise WorkerFailure("candidate_protocol_error: ordered event sequence differs from projected receipts")
    names = ("imp_cast", "imp_first_resolved", "watchwolf_attack", "imp_block", "imp_damage_to_watchwolf", "watchwolf_damage_to_imp", "imp_death", "trigger_stacked", "trigger_resolved", "target_destroyed", "target_graveyard", "imp_returned_to_hand", "dredged", "imp_recast", "imp_recast_resolved", "terminal")
    positions = trace.get("event_positions")
    if not isinstance(positions, dict) or any(not isinstance(positions.get(name), int) for name in names):
        raise WorkerFailure("candidate_protocol_error: Stinkweed Imp ordered event positions are missing")
    ordered = [positions[name] for name in names]
    if ordered != sorted(ordered) or len(set(ordered)) != len(ordered):
        raise WorkerFailure("candidate_protocol_error: combat, trigger, Dredge, and terminal event order differs")
    kinds = {event.get("kind") for event in events}
    required_kinds = {"SpellCast", "SpellResolved", "AttackersDeclared", "BlockersDeclared", "DamageDealtToPermanent", "TriggeredAbilityStacked", "AbilityResolved", "CardDestroyed", "Dredged", "CardMoved", "GameEnded"}
    if not required_kinds <= kinds:
        raise WorkerFailure("candidate_protocol_error: projected combat/Dredge terminal receipts are incomplete")
    by_sequence = {event["sequence"]: event for event in events}
    expected_kinds = {
        "imp_cast": "SpellCast", "imp_first_resolved": "SpellResolved", "watchwolf_attack": "AttackersDeclared",
        "imp_block": "BlockersDeclared", "imp_damage_to_watchwolf": "DamageDealtToPermanent",
        "watchwolf_damage_to_imp": "DamageDealtToPermanent", "imp_death": "CardMoved",
        "trigger_stacked": "TriggeredAbilityStacked", "trigger_resolved": "AbilityResolved",
        "target_destroyed": "CardDestroyed", "target_graveyard": "CardMoved",
        "imp_returned_to_hand": "CardMoved", "dredged": "Dredged", "imp_recast": "SpellCast",
        "imp_recast_resolved": "SpellResolved", "terminal": "GameEnded",
    }
    receipt_by_name: dict[str, dict] = {}
    for name, kind in expected_kinds.items():
        receipt = by_sequence.get(positions[name])
        if receipt is None or receipt.get("kind") != kind:
            raise WorkerFailure(f"candidate_protocol_error: ordered {name} receipt does not match {kind}")
        receipt_by_name[name] = receipt
    canonical = "\n".join(str(event.get("canonical", "")) for event in events)
    imp_id, target_id = trace["imp_object"], trace["target_object"]
    damage_receipt = str(receipt_by_name["imp_damage_to_watchwolf"].get("canonical", ""))
    destroy_receipt = str(receipt_by_name["target_destroyed"].get("canonical", ""))
    stack_receipt = str(receipt_by_name["trigger_stacked"].get("canonical", ""))
    resolved_receipt = str(receipt_by_name["trigger_resolved"].get("canonical", ""))
    dredged_receipt = str(receipt_by_name["dredged"].get("canonical", ""))
    if (f"ObjectId({imp_id})" not in canonical or f"ObjectId({target_id})" not in canonical
        or "destroy-combat-damaged-creature" not in canonical or "count: 5" not in dredged_receipt
        or f"card: ObjectId({imp_id})" not in dredged_receipt
        or f"source: ObjectId({imp_id})" not in damage_receipt
        or f"permanent: ObjectId({target_id})" not in damage_receipt or "amount: 1" not in damage_receipt
        or f"source: ObjectId({imp_id})" not in destroy_receipt or f"card: ObjectId({target_id})" not in destroy_receipt
        or f"card: ObjectId({target_id})" not in str(receipt_by_name["target_graveyard"].get("canonical", ""))
        or f"card: ObjectId({imp_id})" not in str(receipt_by_name["imp_death"].get("canonical", ""))
        or f"card: ObjectId({imp_id})" not in str(receipt_by_name["imp_returned_to_hand"].get("canonical", ""))
        or f"card: ObjectId({imp_id})" not in str(receipt_by_name["imp_recast"].get("canonical", ""))):
        raise WorkerFailure("candidate_protocol_error: exact Imp/Watchwolf/Dredge event provenance is missing")
    source_incarnation = combat["source_incarnation"]
    for label, receipt in (("stack", stack_receipt), ("resolution", resolved_receipt)):
        if f"source: ObjectId({imp_id})" not in receipt or f"source_incarnation: {source_incarnation}" not in receipt:
            raise WorkerFailure(f"candidate_protocol_error: Stinkweed Imp source incarnation is missing from trigger {label}")
    for mill in mills:
        if not any(event.get("kind") == "CardMoved" and "to: Graveyard" in str(event.get("canonical", ""))
            and f"card: ObjectId({mill})" in str(event.get("canonical", "")) for event in events):
            raise WorkerFailure("candidate_protocol_error: exact library-top object is missing from the Dredge mill receipts")
    moved_to_graveyard = [event for event in events if event.get("kind") == "CardMoved" and "Graveyard" in str(event.get("canonical", ""))]
    if len(moved_to_graveyard) < 7:
        raise WorkerFailure("candidate_protocol_error: five Dredge mill moves plus combat deaths are not visible")


def validate_report(report: dict) -> None:
    card = {"collector_number": 107, "definition_id": "RAV-STINKWEED-IMP", "name": "Stinkweed Imp"}
    if (report.get("schema_version") != "cardbench.magic.card-pvp.check.v1" or report.get("task_id") != "cardbench/magic/card"
        or report.get("passed") is not True or report.get("pilot_card_passed") is not True or report.get("cards_passed") != 1
        or report.get("cards_credited") != 0 or report.get("cards_total") != 291 or report.get("printings_total") != 306
        or report.get("coverage_complete") is not False or report.get("full_reward") is not False
        or report.get("harbor_reward") != 0.0 or report.get("pilot_reward") != 0.0
        or report.get("viewer_scope") != "Judge" or report.get("card") != card):
        raise WorkerFailure("candidate_protocol_error: incomplete Stinkweed Imp zero-credit diagnostic report")
    source = report.get("candidate_source_validation")
    if (not isinstance(source, dict) or source.get("candidate_ast_contract_matched") is not True
        or source.get("compile_valid_injected_side_effect_candidate_rejected") is not True
        or source.get("guard_ran_before_candidate_function_calls") is not True):
        raise WorkerFailure("candidate_protocol_error: Stinkweed Imp source-gate attestations are incomplete")
    traces = report.get("traces")
    if not isinstance(traces, list) or len(traces) != 1:
        raise WorkerFailure("candidate_protocol_error: exactly one Stinkweed Imp full-game trace is required")
    validate_trace(traces[0])
    breakdown = report.get("per_card_breakdown")
    if not isinstance(breakdown, list) or sum(1 for row in breakdown if isinstance(row, dict)
        and row.get("collector_number") == 107 and row.get("name") == "Stinkweed Imp"
        and row.get("definition_id") == "RAV-STINKWEED-IMP" and row.get("trace_status") == "passed" and row.get("credited") is False) != 1:
        raise WorkerFailure("candidate_protocol_error: Stinkweed Imp RAV catalog identity/zero-credit row is missing")
    required = ("wrong_mana_cost_detected_in_game", "wrong_body_detected_in_game", "flying_removed_detected_in_game", "dredge_removed_detected_in_game", "wrong_dredge_count_detected_in_game", "combat_trigger_removed_detected_in_game", "combat_trigger_condition_changed_detected_in_game", "combat_trigger_effect_changed_detected_in_game", "compile_valid_candidate_side_effect_rejected_before_execution")
    mutation = report.get("mutation_sensitivity")
    if not isinstance(mutation, dict) or any(mutation.get(key) is not True for key in required):
        raise WorkerFailure("candidate_protocol_error: Stinkweed Imp semantic mutation controls are incomplete")


def parse_report(stdout: bytes) -> dict:
    for line in reversed(stdout.decode(errors="replace").splitlines()):
        try:
            report = json.loads(line)
        except json.JSONDecodeError:
            continue
        if report.get("schema_version") == "cardbench.magic.card-pvp.check.v1":
            validate_report(report)
            return report
    raise WorkerFailure("candidate_protocol_error: no valid Stinkweed Imp PvP report")


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
        raise FileNotFoundError(f"trusted Stinkweed Imp authority is missing: {authority}")
    if not shutil.which("docker"):
        raise FileNotFoundError("Docker is required for Magic candidate isolation")
    verify_workspace(workspace, manifest)
    snapshot = snapshot_workspace(workspace, data["files"], output_root / "candidate-workspace")
    digest = sha256((snapshot / TARGET).read_bytes())
    container = f"cardbench-magic-stinkweed-imp-{uuid.uuid4().hex[:16]}"
    code, stdout, stderr = run_bounded(sandbox_command(snapshot, registry.resolve(), authority, container, image, candidate_kind), 900, container)
    (output_root / "worker.stdout").write_bytes(stdout)
    (output_root / "worker.stderr").write_bytes(stderr)
    if code != 0:
        result = {"schema_version": "cardbench.magic.card-pvp.check.v1", "task_id": "cardbench/magic/card", "candidate": {"kind": candidate_kind, "source_sha256": digest}, "passed": False, "pilot_card_passed": False, "cards_passed": 0, "cards_total": 291, "printings_total": 306, "cards_credited": 0, "coverage_complete": False, "full_reward": False, "harbor_reward": 0.0, "pilot_reward": 0.0, "sealed_authority": False, "worker_image": image, "sandbox": {"network": "none", "rootfs": "read-only", "pids_limit": 128}, "error": "candidate compile or Stinkweed Imp reference trace failed", "diagnostics": stderr.decode(errors="replace")[-12000:]}
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
