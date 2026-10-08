#!/usr/bin/env python3
"""Run the Darkblast pilot in a bounded offline worker and validate its full-game trace."""

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
MATERIALIZER = Path(__file__).with_name("materialize_darkblast_task.py")
TARGET = Path("varieties/magic/sets/ravnica_city_of_guilds/src/cards/darkblast.rs")
AUTHORITY = Path("varieties/magic/session/src/bin/rav_darkblast_pvp.rs")
IMAGE = "rust@sha256:d7482085ff5b415f84dba5647ae71606650bdef00db7aeb69f4b3d170c3e4082"
MAX_OUTPUT = 2 * 1024 * 1024


class WorkerFailure(RuntimeError):
    pass


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def verify_workspace(workspace: Path, manifest: Path) -> dict:
    subprocess.run([sys.executable, str(MATERIALIZER), "--workspace", str(workspace), "--manifest", str(manifest), "--verify"], check=True, cwd=REPO)
    return json.loads(manifest.read_text(encoding="utf-8"))


def snapshot_workspace(workspace: Path, files: dict[str, str], destination: Path) -> Path:
    if destination.is_symlink():
        raise WorkerFailure("candidate_snapshot_path_is_symlink")
    if destination.exists():
        shutil.rmtree(destination)
    destination.mkdir(parents=True)
    for relative in sorted(files):
        source = workspace / relative
        if source.is_symlink() or not source.is_file():
            raise WorkerFailure(f"candidate_snapshot_source_missing: {relative}")
        target = destination / relative
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(source, target)
    return destination


def run_bounded(command: list[str], timeout: int, container: str) -> tuple[int, bytes, bytes]:
    process = subprocess.Popen(command, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    assert process.stdout is not None and process.stderr is not None
    selector = selectors.DefaultSelector()
    selector.register(process.stdout, selectors.EVENT_READ, "stdout")
    selector.register(process.stderr, selectors.EVENT_READ, "stderr")
    stdout, stderr = bytearray(), bytearray()
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
                (stdout if key.data == "stdout" else stderr).extend(chunk)
                if len(stdout) + len(stderr) > MAX_OUTPUT:
                    raise WorkerFailure("candidate_output_limit: worker output exceeded 2 MiB")
        return process.wait(), bytes(stdout), bytes(stderr)
    except BaseException:
        process.kill()
        process.wait()
        subprocess.run(["docker", "rm", "-f", container], capture_output=True, check=False)
        raise


def sandbox_command(workspace: Path, registry: Path, authority: Path, container: str, image: str, candidate_kind: str) -> list[str]:
    if authority.is_symlink() or not authority.is_file():
        raise WorkerFailure("trusted_Darkblast_authority_missing")
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
        "cp /trusted-authority.rs /work/varieties/magic/session/src/bin/rav_darkblast_pvp.rs && "
        "cd /work && cargo run --offline --locked -j 2 --manifest-path varieties/magic/Cargo.toml "
        "-p cardbench-magic-session --bin rav-darkblast-pvp --" + candidate,
    ]


def _receipt(events: list[dict], positions: dict, name: str, kind: str) -> dict:
    sequence = positions.get(name)
    if not isinstance(sequence, int):
        raise WorkerFailure(f"candidate_protocol_error: Darkblast {name} event position is missing")
    receipt = next((event for event in events if event.get("sequence") == sequence), None)
    if not isinstance(receipt, dict) or receipt.get("kind") != kind:
        raise WorkerFailure(f"candidate_protocol_error: Darkblast {name} receipt is not {kind}")
    return receipt


def validate_trace(trace: object) -> None:
    if not isinstance(trace, dict) or trace.get("id") != "rav-darkblast-minus-one-dredge-three-terminal-pvp":
        raise WorkerFailure("candidate_protocol_error: Darkblast full-game trace is missing")
    if (trace.get("passed") is not True or trace.get("terminal") != "Winner(PlayerId(0))"
        or trace.get("winner_seat") != 0 or trace.get("rejected_moves") != 0
        or trace.get("deterministic_replay") is not True):
        raise WorkerFailure("candidate_protocol_error: Darkblast did not reach a clean deterministic terminal duel")
    if not isinstance(trace.get("seed"), int) or not 0 <= trace["seed"] <= 512:
        raise WorkerFailure("candidate_protocol_error: Darkblast seed is outside the bounded deterministic search")
    if not isinstance(trace.get("turns"), int) or not 0 < trace["turns"] <= 80 or not isinstance(trace.get("accepted_moves"), int) or not 0 < trace["accepted_moves"] <= 5000:
        raise WorkerFailure("candidate_protocol_error: Darkblast game bounds or accepted-move receipt is invalid")
    if trace.get("decks") != ["rav_darkblast_black_green_white", "rav_watchwolf_green_white"]:
        raise WorkerFailure("candidate_protocol_error: Darkblast did not use its declared legal 60-card PvP decks")
    darkblast = trace.get("darkblast")
    darkblast_object = trace.get("darkblast_object")
    if not isinstance(darkblast, dict) or not isinstance(darkblast_object, int) or darkblast.get("mana_cost") != "{B}" or darkblast.get("dredge") != 3:
        raise WorkerFailure("candidate_protocol_error: Darkblast printed identity, cost, or Dredge count differs")
    payments = darkblast.get("cast_payments")
    if payments != [["Black"], ["Black"]] or darkblast.get("paid_colors") != ["Black"]:
        raise WorkerFailure("candidate_protocol_error: both actual Darkblast casts must pay exactly {B}")
    opponent_objects = trace.get("opponent_watchwolf_objects")
    if not isinstance(opponent_objects, list) or len(opponent_objects) != 2 or len(set(opponent_objects)) != 2 or not all(isinstance(item, int) for item in opponent_objects):
        raise WorkerFailure("candidate_protocol_error: two distinct opposing Watchwolf objects are required")
    chosen = trace.get("chosen_targets")
    if not isinstance(chosen, list) or len(chosen) != 2 or any(not isinstance(item, dict) for item in chosen):
        raise WorkerFailure("candidate_protocol_error: both target choices are required")
    if {item.get("source") for item in chosen} != {darkblast_object} or {item.get("target") for item in chosen} != set(opponent_objects):
        raise WorkerFailure("candidate_protocol_error: Darkblast choices do not bind the same source to both opposing Watchwolves")
    modifiers = trace.get("modifier_observations")
    if not isinstance(modifiers, list) or len(modifiers) != 2 or any(not isinstance(item, dict) for item in modifiers):
        raise WorkerFailure("candidate_protocol_error: both target-bound modifier observations are required")
    if any(item.get("source") != darkblast_object or item.get("power") != 2 or item.get("toughness") != 2 for item in modifiers) or {item.get("target") for item in modifiers} != set(opponent_objects):
        raise WorkerFailure("candidate_protocol_error: Darkblast must make each exact Watchwolf a 2/2")
    dredge = trace.get("dredge")
    mills = dredge.get("mill_objects") if isinstance(dredge, dict) else None
    if (not isinstance(dredge, dict) or dredge.get("card_object") != darkblast_object or dredge.get("count") != 3
        or dredge.get("same_object_returned_to_hand") is not True or dredge.get("same_object_recast") is not True
        or not isinstance(mills, list) or len(mills) != 3 or len(set(mills)) != 3 or darkblast_object in mills):
        raise WorkerFailure("candidate_protocol_error: exact Dredge 3 mill, same-object return and recast are required")
    events = trace.get("events")
    if not isinstance(events, list) or not events or not all(isinstance(event, dict) for event in events):
        raise WorkerFailure("candidate_protocol_error: Darkblast projected events are malformed")
    sequences = [event.get("sequence") for event in events]
    if any(not isinstance(item, int) for item in sequences) or sequences != sorted(sequences) or len(set(sequences)) != len(sequences):
        raise WorkerFailure("candidate_protocol_error: Darkblast projected event sequence is malformed")
    if trace.get("ordered_event_sequence") != sequences:
        raise WorkerFailure("candidate_protocol_error: ordered event sequence differs from projected receipts")
    positions = trace.get("event_positions")
    if not isinstance(positions, dict):
        raise WorkerFailure("candidate_protocol_error: Darkblast event position map is missing")
    named = {
        "first_darkblast_cast": "SpellCast", "first_darkblast_resolved": "SpellResolved",
        "darkblast_to_hand": "CardMoved", "dredged": "Dredged", "darkblast_recast": "SpellCast",
        "darkblast_recast_resolved": "SpellResolved", "pilot_final_attack": "AttackersDeclared",
        "pilot_final_damage": "DamageDealtToPlayer", "terminal": "GameEnded",
    }
    receipts = {name: _receipt(events, positions, name, kind) for name, kind in named.items()}
    target_receipts = []
    for label in ("first", "second"):
        group = {
            f"{label}_target_cast": "SpellCast", f"{label}_target_resolved": "SpellResolved",
            f"{label}_modifier": "ContinuousEffectCreated", f"{label}_block": "BlockersDeclared",
            f"{label}_target_sba": "StateBasedAction", f"{label}_target_graveyard": "CardMoved",
        }
        target_receipts.append({name: _receipt(events, positions, name, kind) for name, kind in group.items()})
    required_order = [positions[name] for name in ("first_darkblast_cast", "first_darkblast_resolved", "darkblast_to_hand", "dredged", "darkblast_recast", "darkblast_recast_resolved")]
    if required_order != sorted(required_order):
        raise WorkerFailure("candidate_protocol_error: cast, Dredge return, and same-object recast order differs")
    for index, receipts_for_target in enumerate(target_receipts):
        prefix = "first" if index == 0 else "second"
        order = [positions[f"{prefix}_target_{suffix}"] for suffix in ("cast", "resolved")]
        order += [positions[f"{prefix}_modifier"], positions[f"{prefix}_block"], positions[f"{prefix}_target_sba"], positions[f"{prefix}_target_graveyard"]]
        if order != sorted(order):
            raise WorkerFailure("candidate_protocol_error: a Watchwolf's cast, modifier, block, lethal damage, and graveyard order differs")
    if positions["pilot_final_attack"] <= max(positions["first_target_graveyard"], positions["second_target_graveyard"]) or positions["pilot_final_damage"] <= positions["pilot_final_attack"] or positions["terminal"] <= positions["pilot_final_damage"]:
        raise WorkerFailure("candidate_protocol_error: final attack and normal terminal game order differs")
    canonical = {event["sequence"]: str(event.get("canonical", "")) for event in events}
    darkblast_text = f"ObjectId({darkblast_object})"
    for name in ("first_darkblast_cast", "first_darkblast_resolved", "darkblast_recast", "darkblast_recast_resolved"):
        if darkblast_text not in canonical[positions[name]]:
            raise WorkerFailure("candidate_protocol_error: Darkblast cast/resolution event does not cite its physical object")
    hand_text = canonical[positions["darkblast_to_hand"]]
    dredge_text = canonical[positions["dredged"]]
    if darkblast_text not in hand_text or "to: Hand" not in hand_text or darkblast_text not in dredge_text or "count: 3" not in dredge_text:
        raise WorkerFailure("candidate_protocol_error: Dredge return-to-hand receipt lacks the same object or exact count")
    dredged_at = positions["dredged"]
    for mill in mills:
        if not any(event.get("sequence", -1) < positions["darkblast_to_hand"] < dredged_at
            and event.get("kind") == "CardMoved"
            and f"ObjectId({mill})" in str(event.get("canonical", ""))
            and "to: Graveyard" in str(event.get("canonical", "")) for event in events):
            raise WorkerFailure("candidate_protocol_error: exact Dredge mill object lacks a preceding graveyard move receipt")
    for target, group in zip(opponent_objects, target_receipts, strict=True):
        target_text = f"ObjectId({target})"
        modifier = canonical[positions["first_modifier" if group is target_receipts[0] else "second_modifier"]]
        block = canonical[positions["first_block" if group is target_receipts[0] else "second_block"]]
        sba = canonical[positions["first_target_sba" if group is target_receipts[0] else "second_target_sba"]]
        graveyard = canonical[positions["first_target_graveyard" if group is target_receipts[0] else "second_target_graveyard"]]
        if darkblast_text not in modifier or target_text not in modifier or target_text not in block or target_text not in sba or target_text not in graveyard:
            raise WorkerFailure("candidate_protocol_error: target-specific modifier/combat/death provenance is missing")
        pilot_watchwolf = trace.get("pilot_watchwolf_object")
        if not isinstance(pilot_watchwolf, int):
            raise WorkerFailure("candidate_protocol_error: pilot Watchwolf object identity is missing")
        block_sequence = positions["first_block" if group is target_receipts[0] else "second_block"]
        sba_sequence = positions["first_target_sba" if group is target_receipts[0] else "second_target_sba"]
        pilot_damage = [event for event in events if event.get("kind") == "DamageDealtToPermanent"
            and block_sequence < event.get("sequence", -1) < sba_sequence
            and f"source: ObjectId({pilot_watchwolf})" in str(event.get("canonical", ""))
            and f"permanent: ObjectId({target})" in str(event.get("canonical", ""))
            and "amount: 3" in str(event.get("canonical", ""))]
        target_damage = [event for event in events if event.get("kind") == "DamageDealtToPermanent"
            and block_sequence < event.get("sequence", -1) < sba_sequence
            and f"source: ObjectId({target})" in str(event.get("canonical", ""))
            and f"permanent: ObjectId({pilot_watchwolf})" in str(event.get("canonical", ""))
            and "amount: 2" in str(event.get("canonical", ""))]
        if len(pilot_damage) != 1 or len(target_damage) != 1:
            raise WorkerFailure("candidate_protocol_error: each exact 2/2 target must exchange 3/2 combat damage before its lethal-damage state action")
    if not isinstance(trace.get("life_totals"), list) or len(trace["life_totals"]) != 2 or trace["life_totals"][0] <= 0 or trace["life_totals"][1] > 0:
        raise WorkerFailure("candidate_protocol_error: terminal life totals are inconsistent with the pilot win")


def validate_report(report: dict) -> None:
    card = {"collector_number": 82, "definition_id": "RAV-DARKBLAST", "name": "Darkblast"}
    if (report.get("schema_version") != "cardbench.magic.card-pvp.check.v1" or report.get("task_id") != "cardbench/magic/card"
        or report.get("passed") is not True or report.get("pilot_card_passed") is not True or report.get("replay_deterministic") is not True
        or report.get("cards_passed") != 1 or report.get("cards_credited") != 0 or report.get("cards_total") != 291
        or report.get("printings_total") != 306 or report.get("coverage_complete") is not False
        or report.get("full_reward") is not False or report.get("harbor_reward") != 0.0 or report.get("pilot_reward") != 0.0
        or report.get("viewer_scope") != "Judge" or report.get("card") != card):
        raise WorkerFailure("candidate_protocol_error: incomplete Darkblast zero-credit diagnostic report")
    source = report.get("candidate_source_validation")
    if not isinstance(source, dict) or source.get("candidate_ast_contract_matched") is not True or source.get("compile_valid_injected_side_effect_candidate_rejected") is not True or source.get("guard_ran_before_candidate_function_calls") is not True:
        raise WorkerFailure("candidate_protocol_error: Darkblast source-gate attestations are incomplete")
    rules = report.get("rules_authority")
    if not isinstance(rules, dict) or rules.get("no_provider_trial") is not True or rules.get("authority_cutoff") != "2026-09-24":
        raise WorkerFailure("candidate_protocol_error: Darkblast authority cutoff or no-provider boundary is missing")
    traces = report.get("traces")
    if not isinstance(traces, list) or len(traces) != 1:
        raise WorkerFailure("candidate_protocol_error: exactly one Darkblast full-game trace is required")
    validate_trace(traces[0])
    mutation = report.get("mutation_sensitivity")
    required = ("wrong_mana_cost_detected_in_game", "wrong_modifier_detected_in_game", "dredge_removed_detected_in_game", "wrong_dredge_count_detected_in_game", "compile_valid_candidate_side_effect_rejected_before_execution")
    if not isinstance(mutation, dict) or any(mutation.get(key) is not True for key in required) or mutation.get("sealed_mutation_authority") is not False:
        raise WorkerFailure("candidate_protocol_error: Darkblast semantic mutation controls are incomplete")
    breakdown = report.get("per_card_breakdown")
    if not isinstance(breakdown, list) or sum(1 for row in breakdown if isinstance(row, dict) and row.get("collector_number") == 82 and row.get("name") == "Darkblast" and row.get("definition_id") == "RAV-DARKBLAST" and row.get("trace_status") == "passed" and row.get("credited") is False) != 1:
        raise WorkerFailure("candidate_protocol_error: Darkblast catalog identity and zero-credit row are missing")


def parse_report(stdout: bytes) -> dict:
    for line in reversed(stdout.decode(errors="replace").splitlines()):
        try:
            report = json.loads(line)
        except json.JSONDecodeError:
            continue
        if report.get("schema_version") == "cardbench.magic.card-pvp.check.v1":
            validate_report(report)
            return report
    raise WorkerFailure("candidate_protocol_error: no valid Darkblast PvP report")


def score(workspace: Path, manifest_path: Path, output_root: Path, candidate_kind: str, timeout: int) -> int:
    output_root.mkdir(parents=True, exist_ok=True)
    manifest = verify_workspace(workspace.resolve(), manifest_path.resolve())
    registry = Path.home() / ".cargo" / "registry"
    if not registry.is_dir():
        raise FileNotFoundError(f"offline Cargo registry cache is missing: {registry}")
    if not shutil.which("docker"):
        raise FileNotFoundError("Docker is required for isolated Magic candidate scoring")
    image = os.environ.get("CARDBENCH_MAGIC_WORKER_IMAGE", IMAGE)
    if not (image.startswith("rust@sha256:") or image.startswith("sha256:")):
        raise ValueError("CARDBENCH_MAGIC_WORKER_IMAGE must be an immutable digest reference")
    container = f"cardbench-magic-darkblast-{uuid.uuid4().hex[:12]}"
    with __import__("tempfile").TemporaryDirectory(prefix="darkblast-candidate-") as temp:
        candidate = snapshot_workspace(workspace.resolve(), manifest["files"], Path(temp) / "workspace")
        authority = (REPO / AUTHORITY).resolve()
        command = sandbox_command(candidate, registry.resolve(), authority, container, image, candidate_kind)
        code, stdout, stderr = run_bounded(command, timeout, container)
    (output_root / "worker.stdout").write_bytes(stdout)
    (output_root / "worker.stderr").write_bytes(stderr)
    if code != 0:
        report = {"schema_version": "cardbench.magic.card-pvp.check.v1", "task_id": "cardbench/magic/card", "candidate": {"kind": candidate_kind, "source_sha256": sha256((workspace.resolve() / TARGET).read_bytes())}, "passed": False, "pilot_card_passed": False, "cards_passed": 0, "cards_total": 291, "printings_total": 306, "cards_credited": 0, "coverage_complete": False, "full_reward": False, "harbor_reward": 0.0, "pilot_reward": 0.0, "sealed_authority": False, "error": "candidate compile or Darkblast trace gate failed", "diagnostics": stderr.decode(errors="replace")[-12000:]}
    else:
        report = parse_report(stdout)
        report["candidate"] = {"kind": candidate_kind, "card_definition": "RAV-DARKBLAST", "source_sha256": sha256((workspace.resolve() / TARGET).read_bytes()), "materialized_workspace_sha256": manifest["workspace_sha256"]}
        report["candidate_kind"] = candidate_kind
        report["starting_workspace_sha256"] = manifest["workspace_sha256"]
        report["sealed_authority"] = False
        report["worker_image"] = image
        report["sandbox"] = {"network": "none", "rootfs": "read-only", "pids_limit": 128}
        report["harbor_reward"] = 0.0
        report["full_reward"] = False
    (output_root / "result.json").write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    (output_root / "reward.txt").write_text("0.00000000\n", encoding="utf-8")
    return 0 if report.get("passed") is True else 1


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--workspace", required=True, type=Path)
    parser.add_argument("--manifest", required=True, type=Path)
    parser.add_argument("--output-root", required=True, type=Path)
    parser.add_argument("--candidate-kind", choices=("reference", "agent"), default="agent")
    parser.add_argument("--timeout", type=int, default=900)
    args = parser.parse_args()
    try:
        return score(args.workspace, args.manifest, args.output_root, args.candidate_kind, args.timeout)
    except Exception as error:
        args.output_root.mkdir(parents=True, exist_ok=True)
        failure = {"schema_version": "cardbench.magic.card-pvp.check.v1", "task_id": "cardbench/magic/card", "passed": False, "pilot_card_passed": False, "cards_passed": 0, "cards_total": 291, "printings_total": 306, "cards_credited": 0, "coverage_complete": False, "full_reward": False, "harbor_reward": 0.0, "pilot_reward": 0.0, "sealed_authority": False, "error": f"{type(error).__name__}: {error}"}
        (args.output_root / "result.json").write_text(json.dumps(failure, indent=2) + "\n", encoding="utf-8")
        (args.output_root / "reward.txt").write_text("0.00000000\n", encoding="utf-8")
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
