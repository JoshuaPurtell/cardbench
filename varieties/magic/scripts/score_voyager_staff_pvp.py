#!/usr/bin/env python3
"""Run Voyager Staff in a bounded offline candidate worker."""

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
MATERIALIZER = Path(__file__).with_name("materialize_voyager_staff_task.py")
TARGET = Path("varieties/magic/sets/ravnica_city_of_guilds/src/cards/voyager_staff.rs")
AUTHORITY = Path("varieties/magic/session/src/bin/rav_voyager_staff_pvp.rs")
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
        "cp /trusted-authority.rs /work/varieties/magic/session/src/bin/rav_voyager_staff_pvp.rs && "
        "cd /work && cargo run --offline --locked -j 2 --manifest-path varieties/magic/Cargo.toml "
        "-p cardbench-magic-session --bin rav-voyager-staff-pvp --" + candidate_flag,
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
    raise WorkerFailure("candidate_protocol_error: no valid Voyager Staff PvP result")


def validate_report(report: dict) -> None:
    expected_card = {"collector_number": 274, "definition_id": "RAV-VOYAGER-STAFF", "name": "Voyager Staff"}
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
        raise WorkerFailure("candidate_protocol_error: incomplete Voyager Staff zero-credit report")
    candidate = report.get("candidate")
    if not isinstance(candidate, dict) or candidate.get("card_definition") != "RAV-VOYAGER-STAFF":
        raise WorkerFailure("candidate_protocol_error: wrong Voyager Staff candidate identity")
    traces = report.get("traces")
    if not isinstance(traces, list) or len(traces) != 1 or traces[0].get("trace_id") != "rav-voyager-staff-linked-exile-next-end-step-full-game":
        raise WorkerFailure("candidate_protocol_error: exactly one Voyager Staff full-game trace is required")
    trace = traces[0]
    mechanics = trace.get("mechanics")
    if (
        trace.get("terminal") != "Winner(PlayerId(0))"
        or trace.get("winner_seat") != 0
        or trace.get("rejected_moves") != 0
        or not isinstance(mechanics, dict)
        or mechanics.get("staff_definition_id") != "RAV-VOYAGER-STAFF"
        or mechanics.get("staff_cast_cost") != "{1}"
        or not isinstance(mechanics.get("staff_cast_payment"), list)
        or len(mechanics["staff_cast_payment"]) != 1
        or mechanics.get("staff_zone_after_activation") != "Graveyard"
        or mechanics.get("activation_ability") != "sacrifice-linked-exile-target-creature-until-end-step"
        or mechanics.get("activation_cost") != "{2}"
        or mechanics.get("activation_mana_paid") != 2
        or mechanics.get("activation_source_sacrificed") is not True
        or mechanics.get("target_definition_id") != "RAV-WATCHWOLF"
        or mechanics.get("target_owner") != 1
        or mechanics.get("target_controller_after_return") != 1
        or mechanics.get("target_zone_after_return") != "Battlefield"
        or mechanics.get("returned_to_owner") is not True
        or mechanics.get("new_incarnation_on_return") is not True
        or mechanics.get("linked_group_source") != mechanics.get("staff_object")
        or mechanics.get("linked_group_controller") != 0
    ):
        raise WorkerFailure("candidate_protocol_error: Voyager Staff cost, sacrifice, target, or owner return differs")
    ids = [mechanics.get("target_original_incarnation"), mechanics.get("target_exile_incarnation"), mechanics.get("target_return_incarnation")]
    if not all(isinstance(value, int) for value in ids) or not ids[0] < ids[1] < ids[2]:
        raise WorkerFailure("candidate_protocol_error: exact target incarnations did not advance through exile and return")
    action = mechanics.get("delayed_action")
    if not isinstance(action, dict) or action.get("timing") != "EndStep" or action.get("returned_objects") != [mechanics.get("target_object")]:
        raise WorkerFailure("candidate_protocol_error: exact target was not returned by the next end-step action")
    members = mechanics.get("linked_group_members")
    if (
        not isinstance(members, list)
        or len(members) != 1
        or members[0] != {
            "object": mechanics.get("target_object"),
            "exile_incarnation": mechanics.get("target_exile_incarnation"),
            "primary_creature": True,
        }
    ):
        raise WorkerFailure("candidate_protocol_error: linked group does not retain exactly the target exile incarnation")
    events = trace.get("events")
    if not isinstance(events, list) or len(events) < 20:
        raise WorkerFailure("candidate_protocol_error: Voyager Staff causal event trace is missing")
    required = [
        "AbilityManaPaid", "SacrificedAsAbilityCost", "AbilityActivated", "CardMoved",
        "ObjectIncarnationAdvanced", "DelayedActionScheduled", "AbilityResolved", "StepBegan",
        "DelayedActionConsumed", "CardMoved", "ObjectIncarnationAdvanced",
        "AttackersDeclared", "DamageDealtToPlayer", "GameEnded",
    ]
    kinds = [event.get("kind") for event in events if isinstance(event, dict)]
    cursor = 0
    for kind in kinds:
        if cursor < len(required) and kind == required[cursor]:
            cursor += 1
    if cursor != len(required):
        raise WorkerFailure("candidate_protocol_error: linked exile and return event order is incomplete")
    activation = next((event for event in events if event.get("kind") == "AbilityActivated" and event.get("facts", {}).get("ability") == "sacrifice-linked-exile-target-creature-until-end-step"), None)
    if (
        activation is None
        or activation.get("facts", {}).get("incarnation") != mechanics.get("linked_group_source_incarnation")
        or activation.get("facts", {}).get("objects") != [mechanics.get("staff_object")]
        or activation.get("turn") != action.get("due_turn")
    ):
        raise WorkerFailure("candidate_protocol_error: linked group source incarnation or due turn differs from activation")
    scheduled = next((event for event in events if event.get("kind") == "DelayedActionScheduled"), None)
    consumed = next((event for event in events if event.get("kind") == "DelayedActionConsumed"), None)
    due_end = next((
        event for event in events
        if event.get("kind") == "StepBegan"
        and event.get("facts", {}).get("seats") == [0]
        and event.get("facts", {}).get("amount") == action.get("due_turn")
        and event.get("facts", {}).get("step") == "End"
    ), None)
    activation_window = next((
        event for event in events
        if event.get("sequence") == trace.get("activation_window_sequence")
        and event.get("kind") == "StepBegan"
        and event.get("facts", {}).get("seats") == [0]
        and event.get("facts", {}).get("amount") == action.get("due_turn")
        and event.get("facts", {}).get("step") in {"PrecombatMain", "PostcombatMain"}
    ), None)
    if (
        activation is None
        or scheduled is None
        or due_end is None
        or consumed is None
        or activation_window is None
        or activation.get("sequence") != trace.get("activation_sequence")
        or scheduled.get("sequence") != trace.get("schedule_sequence")
        or due_end.get("sequence") != trace.get("next_end_step_sequence")
        or consumed.get("sequence") != trace.get("return_sequence")
        or not activation_window.get("sequence") < scheduled.get("sequence") < due_end.get("sequence") < consumed.get("sequence")
    ):
        raise WorkerFailure("candidate_protocol_error: target return did not occur at the controller's next end step")
    if not any(
        event.get("kind") == "DelayedActionScheduled"
        and "EndStep" in event.get("canonical", "")
        and f"ObjectId({mechanics['target_object']})" in event.get("canonical", "")
        and str(mechanics["target_exile_incarnation"]) in event.get("canonical", "")
        for event in events
        if isinstance(event, dict)
    ):
        raise WorkerFailure("candidate_protocol_error: delayed action lost exact target exile provenance")
    mutations = report.get("mutation_sensitivity")
    mutation_keys = (
        "wrong_staff_cast_cost_detected_in_game",
        "wrong_activation_cost_detected_in_game",
        "missing_source_sacrifice_detected_in_game",
        "missing_delayed_return_detected_in_game",
        "target_requirement_mutation_detected",
    )
    if not isinstance(mutations, dict) or any(mutations.get(key) is not True for key in mutation_keys):
        raise WorkerFailure("candidate_protocol_error: all Voyager Staff mutations must be detected")
    guard = report.get("candidate_source_validation")
    guard_keys = (
        "normalized_literal_source_and_ast_checked_before_candidate_calls",
        "compile_valid_injected_side_effect_fixture_compiled",
        "compile_valid_injected_side_effect_candidate_rejected",
        "guard_ran_before_candidate_function_calls",
        "side_effect_sentinel_absent_after_rejection",
    )
    if not isinstance(guard, dict) or any(guard.get(key) is not True for key in guard_keys) or guard.get("injected_candidate_calls_before_rejection") != 0:
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
        raise FileNotFoundError(f"trusted Voyager Staff authority is missing: {authority}")
    if not shutil.which("docker"):
        raise FileNotFoundError("Docker is required for Magic candidate isolation")
    verify_workspace(workspace, manifest)
    candidate_hash = sha256((workspace / TARGET).read_bytes())
    name = f"cardbench-magic-voyager-staff-{uuid.uuid4().hex[:16]}"
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
            "error": "candidate compile or Voyager Staff reference trace failed",
            "diagnostics": stderr.decode(errors="replace")[-12000:],
        }
    else:
        result = parse_report(stdout)
        result["candidate"] = {
            "kind": candidate_kind,
            "card_definition": "RAV-VOYAGER-STAFF",
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
            "pilot_reward": 0.0,
            "sealed_authority": False,
            "error": str(error),
        }
        (args.output_root / "result.json").write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
        (args.output_root / "reward.txt").write_text("0.0\n", encoding="utf-8")
        print(f"Magic candidate verifier failed: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
