#!/usr/bin/env python3
"""Run the isolated Dream Leash reference or candidate PvP authority."""

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
MATERIALIZER = Path(__file__).with_name("materialize_dream_leash_task.py")
TARGET = Path("varieties/magic/sets/ravnica_city_of_guilds/src/cards/dream_leash.rs")
AUTHORITY_SOURCE = Path("varieties/magic/session/src/bin/rav_dream_leash_pvp.rs")
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
    output = bytearray()
    diagnostics = bytearray()
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
                destination = output if key.data == "stdout" else diagnostics
                destination.extend(chunk)
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
        "cp /trusted-authority.rs /work/varieties/magic/session/src/bin/rav_dream_leash_pvp.rs && "
        "cd /work && cargo run --offline --locked -j 2 --manifest-path varieties/magic/Cargo.toml "
        "-p cardbench-magic-session --bin rav-dream-leash-pvp --" + candidate_flag,
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
    raise WorkerFailure("candidate_protocol_error: no valid Dream Leash PvP result")


def validate_report(report: dict) -> None:
    expected_card = {"collector_number": 45, "printings": 1, "definition_id": "RAV-DREAM-LEASH", "name": "Dream Leash"}
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
        or report.get("replay_deterministic") is not True
        or report.get("viewer_scope") != "Judge"
        or report.get("card") != expected_card
    ):
        raise WorkerFailure("candidate_protocol_error: incomplete Dream Leash single-card report")
    candidate = report.get("candidate")
    if not isinstance(candidate, dict) or candidate.get("card_definition") != "RAV-DREAM-LEASH":
        raise WorkerFailure("candidate_protocol_error: wrong Dream Leash identity")
    traces = report.get("traces")
    if not isinstance(traces, list) or len(traces) != 1:
        raise WorkerFailure("candidate_protocol_error: one Dream Leash trace is required")
    trace = traces[0]
    mechanics = trace.get("mechanics", {}) if isinstance(trace, dict) else {}
    if (
        not isinstance(trace, dict)
        or trace.get("trace_id") != "rav-dream-leash-tapped-permanent-cast-only-pvp"
        or trace.get("passed") is not True
        or trace.get("terminal") != "Winner(PlayerId(0))"
        or trace.get("winner_seat") != 0
        or trace.get("rejected_moves") != 0
        or mechanics.get("mana_cost") != "{3}{U}{U}"
        or mechanics.get("target_definition") != "RAV-WATCHWOLF"
        or mechanics.get("target_was_tapped_at_cast") is not True
        or mechanics.get("tidewater_targeted_exact_aura_target") is not True
        or mechanics.get("target_untapped_while_aura_spell_on_stack") is not True
        or mechanics.get("aura_resolved_after_target_untapped") is not True
        or mechanics.get("aura_attached") is not True
        or mechanics.get("controller_changed_from_opponent_to_pilot") is not True
        or mechanics.get("target_controller_at_terminal") != 0
        or mechanics.get("target_is_untapped_at_terminal") is not True
        or mechanics.get("aura_final_zone") != "Battlefield"
        or mechanics.get("controller_won") is not True
    ):
        raise WorkerFailure("candidate_protocol_error: Dream Leash did not satisfy its cast-time tapped Aura contract")
    aura, target, tidewater = (mechanics.get(key) for key in ("aura_object", "target_object", "tidewater_source"))
    if not all(isinstance(value, int) for value in (aura, target, tidewater)) or len({aura, target, tidewater}) != 3:
        raise WorkerFailure("candidate_protocol_error: Aura, target, and response source identities are not distinct")
    payment = mechanics.get("payment_colors")
    if not isinstance(payment, list) or len(payment) != 5 or payment.count("Blue") < 2:
        raise WorkerFailure("candidate_protocol_error: Dream Leash exact colored/generic payment evidence is incomplete")
    event_order = trace.get("ordered_event_sequence")
    events = trace.get("events")
    expected_kinds = ["AttackersDeclared", "SpellManaPaid", "SpellCast", "AbilityActivated", "PermanentUntapped", "AbilityResolved", "SpellResolved", None, "ControllerChanged", "GameEnded"]
    if (
        not isinstance(event_order, list)
        or len(event_order) != 10
        or any(not isinstance(value, int) for value in event_order)
        or event_order != sorted(event_order)
        or not isinstance(events, list)
        or len(events) != 10
    ):
        raise WorkerFailure("candidate_protocol_error: Dream Leash event trace is incomplete")
    sequences = [event.get("sequence") for event in events if isinstance(event, dict)]
    kinds = [event.get("kind") for event in events if isinstance(event, dict)]
    if (
        len(sequences) != len(events)
        or any(not isinstance(value, int) for value in sequences)
        or sequences != event_order
        or sequences != sorted(sequences)
        or kinds[:7] != expected_kinds[:7]
        or kinds[7] not in {"AuraAttached", "AttachmentEstablishedWithoutContinuousEffect"}
        or kinds[8:] != expected_kinds[8:]
    ):
        raise WorkerFailure("candidate_protocol_error: cast, untap response, Aura attachment, control, and terminal order differs")
    paid = events[1].get("facts", {})
    if (
        paid.get("objects") != [aura]
        or paid.get("seats") != [0]
        or paid.get("amount") != 5
        or paid.get("mana_colors") != payment
        or payment.count("Blue") < 2
        or events[0].get("facts", {}).get("seats") != [1]
        or target not in events[0].get("facts", {}).get("objects", [])
        or events[2].get("facts", {}).get("objects") != [aura]
        or events[2].get("facts", {}).get("seats") != [0]
        or events[3].get("facts", {}).get("objects") != [tidewater]
        or events[3].get("facts", {}).get("seats") != [1]
        or events[3].get("facts", {}).get("ability") != "tap-untap-target-permanent"
        # PermanentUntapped is not projected into typed facts; its canonical
        # event binds the exact source and target below.
        or events[4].get("facts", {}) != {}
        or f"source: ObjectId({tidewater})" not in events[4].get("canonical", "")
        or f"card: ObjectId({target})" not in events[4].get("canonical", "")
        or events[5].get("facts", {}).get("objects") != [tidewater]
        or events[5].get("facts", {}).get("ability") != "tap-untap-target-permanent"
        or events[6].get("facts", {}).get("objects") != [aura]
        or events[9].get("facts", {}).get("seats") != [0]
    ):
        raise WorkerFailure("candidate_protocol_error: Dream Leash exact object, actor, payment, and response evidence differs")
    mutations = report.get("mutation_sensitivity")
    required = (
        "untapped_target_rejected_before_payment_or_stack_mutation",
        "missing_tapped_target_restriction_detected_by_untapped_control",
        "source_relative_control_effect_removal_detected_in_game",
        "incorrect_mana_cost_detected_in_game",
        "compile_valid_side_effect_candidate_rejected_before_candidate_calls",
    )
    if not isinstance(mutations, dict) or any(mutations.get(key) is not True for key in required) or mutations.get("sealed_mutation_authority") is not False:
        raise WorkerFailure("candidate_protocol_error: Dream Leash semantic mutation and source gates are incomplete")
    guard = report.get("source_guard")
    if not isinstance(guard, dict) or guard.get("compile_valid_side_effect_candidate_rejected_before_candidate_calls") is not True or guard.get("candidate_factories_called_for_injected_source") is not False:
        raise WorkerFailure("candidate_protocol_error: Dream Leash malicious-candidate rejection attestation is missing")


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
        raise FileNotFoundError(f"trusted Dream Leash authority is missing: {authority}")
    if not shutil.which("docker"):
        raise FileNotFoundError("Docker is required for Magic candidate isolation")
    verify_workspace(workspace, manifest)
    snapshot = make_scoring_snapshot(workspace, manifest_data["files"], output_root / "candidate-workspace")
    candidate_hash = sha256((snapshot / TARGET).read_bytes())
    name = f"cardbench-magic-dream-leash-{uuid.uuid4().hex[:16]}"
    command = sandbox_command(snapshot, cargo_registry.resolve(), authority, name, worker_image, candidate_kind)
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
            "error": "candidate compile or Dream Leash reference trace failed",
            "diagnostics": stderr.decode(errors="replace")[-12000:],
        }
    else:
        result = parse_report(stdout)
        result["candidate"] = {
            "kind": candidate_kind,
            "card_definition": "RAV-DREAM-LEASH",
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
        (args.output_root / "reward.txt").write_text("0.00000000\n", encoding="utf-8")
        print(f"Magic candidate verifier failed: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
