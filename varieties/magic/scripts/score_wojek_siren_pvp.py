#!/usr/bin/env python3
"""Run the Wojek Siren pilot in a networkless, resource-limited worker."""

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
MATERIALIZER = Path(__file__).with_name("materialize_wojek_siren_task.py")
TARGET = Path("varieties/magic/sets/ravnica_city_of_guilds/src/cards/wojek_siren.rs")
AUTHORITY_SOURCE = Path("varieties/magic/session/src/bin/rav_wojek_siren_pvp.rs")
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
        "cp /trusted-authority.rs /work/varieties/magic/session/src/bin/rav_wojek_siren_pvp.rs && "
        "cd /work && cargo run --offline --locked -j 2 --manifest-path varieties/magic/Cargo.toml "
        "-p cardbench-magic-session --bin rav-wojek-siren-pvp --" + candidate_flag,
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
    raise WorkerFailure("candidate_protocol_error: no valid Wojek Siren PvP result")


def validate_report(report: dict) -> None:
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
        or report.get("card") != {
            "collector_number": 37,
            "printings": 1,
            "definition_id": "RAV-WOJEK-SIREN",
            "name": "Wojek Siren",
        }
    ):
        raise WorkerFailure("candidate_protocol_error: incomplete Wojek Siren single-card report")
    candidate = report.get("candidate")
    if not isinstance(candidate, dict) or candidate.get("card_definition") != "RAV-WOJEK-SIREN":
        raise WorkerFailure("candidate_protocol_error: wrong Wojek Siren identity")
    traces = report.get("traces")
    if not isinstance(traces, list) or len(traces) != 1:
        raise WorkerFailure("candidate_protocol_error: one Wojek Siren trace is required")
    trace = traces[0]
    if (
        not isinstance(trace, dict)
        or trace.get("id") != "rav-wojek-siren-radiance-shared-color-pump"
        or trace.get("passed") is not True
        or trace.get("terminal") != "Winner(PlayerId(0))"
        or trace.get("winner_seat") != 0
        or trace.get("rejected_moves") != 0
        or trace.get("target_definition") != "RAV-WATCHWOLF"
        or trace.get("target_bound_stack_object_verified") is not True
        or trace.get("combat_damage_total", 0) < 20
    ):
        raise WorkerFailure("candidate_protocol_error: Wojek Siren did not resolve Radiance in a clean full-game combat win")
    siren = trace.get("siren_object")
    target = trace.get("target_object")
    ally = trace.get("shared_color_ally_object")
    unshared = trace.get("nonsharing_control_object")
    if not all(isinstance(value, int) for value in (siren, target, ally, unshared)) or len({siren, target, ally, unshared}) != 4:
        raise WorkerFailure("candidate_protocol_error: Siren, target, Radiance peer, and nonsharing control identities are not distinct")
    watchwolves = trace.get("shared_watchwolf_objects")
    recruits = trace.get("shared_recruit_objects")
    recipients = trace.get("radiance_recipients")
    if (
        not isinstance(watchwolves, list) or not watchwolves
        or not isinstance(recruits, list) or not recruits
        or target not in watchwolves or ally not in recruits
        or set(recipients or []) != set(watchwolves) | set(recruits)
        or unshared in (set(watchwolves) | set(recruits))
    ):
        raise WorkerFailure("candidate_protocol_error: Radiance recipient identities are incomplete or include a nonsharing creature")
    snapshots = trace.get("live_power_toughness")
    if not isinstance(snapshots, dict):
        raise WorkerFailure("candidate_protocol_error: live power/toughness snapshots are missing")
    if (
        any(snapshots.get(str(object_id)) != [4, 4] for object_id in watchwolves)
        or any(snapshots.get(str(object_id)) != [2, 2] for object_id in recruits)
        or snapshots.get(str(unshared)) != [2, 2]
    ):
        raise WorkerFailure("candidate_protocol_error: Wojek Siren applied the wrong Radiance P/T set")
    events = trace.get("events")
    if not isinstance(events, list) or not events:
        raise WorkerFailure("candidate_protocol_error: event trace is missing")
    sequences = [event.get("sequence") for event in events if isinstance(event, dict)]
    if len(sequences) != len(events) or any(not isinstance(sequence, int) for sequence in sequences):
        raise WorkerFailure("candidate_protocol_error: event sequence is malformed")
    if sequences != sorted(sequences) or sequences != trace.get("ordered_event_sequence"):
        raise WorkerFailure("candidate_protocol_error: selected event sequence is not canonical")
    kinds = [event.get("kind") for event in events]
    if kinds.count("SpellManaPaid") != 1 or kinds.count("SpellCast") != 1 or kinds.count("SpellResolved") != 1 or kinds.count("GameEnded") != 1:
        raise WorkerFailure("candidate_protocol_error: cast and terminal event receipts are incomplete")
    recipient_events = [event for event in events if event.get("kind") == "ContinuousEffectCreated"]
    if len(recipient_events) != len(set(watchwolves) | set(recruits)):
        raise WorkerFailure("candidate_protocol_error: not every shared-color creature received one effect event")
    expected_event_order = ["SpellManaPaid", "SpellCast", "ContinuousEffectCreated", "SpellResolved", "CardMoved", "AttackersDeclared", "DamageDealtToPlayer", "GameEnded"]
    positions = {kind: kinds.index(kind) for kind in expected_event_order if kind in kinds}
    if not all(kind in positions for kind in expected_event_order):
        raise WorkerFailure("candidate_protocol_error: required resolution, move, attack, damage, and terminal event kinds are absent")
    if not (positions["SpellManaPaid"] < positions["SpellCast"] < positions["ContinuousEffectCreated"] < positions["SpellResolved"] < positions["CardMoved"] < positions["AttackersDeclared"] < positions["DamageDealtToPlayer"] < positions["GameEnded"]):
        raise WorkerFailure("candidate_protocol_error: Wojek Siren cast, resolution, and combat event ordering differs")
    payment = events[positions["SpellManaPaid"]]
    if payment.get("facts", {}).get("mana_colors") != ["White"] or payment.get("facts", {}).get("objects") != [siren]:
        raise WorkerFailure("candidate_protocol_error: Wojek Siren did not pay exactly one white mana")
    effect_targets = set()
    for event in recipient_events:
        canonical = event.get("canonical", "")
        if f"source: ObjectId({siren})" not in canonical:
            raise WorkerFailure("candidate_protocol_error: Radiance event provenance is missing")
        matched = [object_id for object_id in set(watchwolves) | set(recruits) if f"target: ObjectId({object_id})" in canonical]
        if len(matched) != 1:
            raise WorkerFailure("candidate_protocol_error: Radiance event target is missing or ambiguous")
        effect_targets.add(matched[0])
    if effect_targets != set(watchwolves) | set(recruits):
        raise WorkerFailure("candidate_protocol_error: Radiance effects do not match the complete shared-color set")
    attack_event = events[positions["AttackersDeclared"]]
    attack_ids = set(attack_event.get("facts", {}).get("objects", []))
    if target not in attack_ids or ally not in attack_ids:
        raise WorkerFailure("candidate_protocol_error: modified target and ally did not attack")
    damage_events = [event for event in events if event.get("kind") == "DamageDealtToPlayer"]
    damage_by_source = {event.get("facts", {}).get("objects", [None])[0]: event.get("facts", {}).get("amount") for event in damage_events}
    if damage_by_source.get(target) != 4 or damage_by_source.get(ally) != 2:
        raise WorkerFailure("candidate_protocol_error: pumped Watchwolf and Boros Recruit damage differs")
    terminal = events[positions["GameEnded"]]
    if terminal.get("facts", {}).get("seats") != [0]:
        raise WorkerFailure("candidate_protocol_error: match did not end in the Wojek Siren pilot win")
    mutation = report.get("mutation_sensitivity", {})
    required = (
        "wrong_radiance_scope_detected_in_game",
        "wrong_pump_detected_in_game",
        "wrong_cost_detected_in_game",
        "wrong_color_detected_in_game",
    )
    if not isinstance(mutation, dict) or any(mutation.get(key) is not True for key in required):
        raise WorkerFailure("candidate_protocol_error: Wojek Siren semantic mutation controls are incomplete")


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
        raise FileNotFoundError(f"trusted Wojek Siren authority source is missing: {authority}")
    if not shutil.which("docker"):
        raise FileNotFoundError("Docker is required for Magic candidate isolation")
    verify_workspace(workspace, manifest)
    scoring_workspace = make_scoring_snapshot(workspace, manifest_data["files"], output_root / "candidate-workspace")
    candidate_hash = sha256((scoring_workspace / TARGET).read_bytes())
    name = f"cardbench-magic-wojek-siren-{uuid.uuid4().hex[:16]}"
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
            "pilot_reward": 0.0,
            "sealed_authority": False,
            "worker_image": worker_image,
            "sandbox": {"network": "none", "rootfs": "read-only", "pids_limit": 128},
            "error": "candidate compile or Wojek Siren reference trace failed",
            "diagnostics": stderr.decode(errors="replace")[-12000:],
        }
    else:
        result = parse_report(stdout)
        result["candidate"] = {
            "kind": candidate_kind,
            "card_definition": "RAV-WOJEK-SIREN",
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
