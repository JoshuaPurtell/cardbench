#!/usr/bin/env python3
"""Run the Seismic Spike pilot in a networkless, resource-limited worker."""

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
MATERIALIZER = Path(__file__).with_name("materialize_seismic_spike_task.py")
TARGET = Path("varieties/magic/sets/ravnica_city_of_guilds/src/cards/seismic_spike.rs")
AUTHORITY_SOURCE = Path("varieties/magic/session/src/bin/rav_seismic_spike_pvp.rs")
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
        "cp /trusted-authority.rs /work/varieties/magic/session/src/bin/rav_seismic_spike_pvp.rs && "
        "cd /work && cargo run --offline --locked -j 2 --manifest-path varieties/magic/Cargo.toml "
        "-p cardbench-magic-session --bin rav-seismic_spike-pvp --" + candidate_flag,
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
    raise WorkerFailure("candidate_protocol_error: no valid Seismic Spike PvP result")


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
        or report.get("viewer_scope") != "Judge"
        or report.get("card") != {
            "collector_number": 141,
            "printings": 1,
            "definition_id": "RAV-SEISMIC-SPIKE",
            "name": "Seismic Spike",
        }
    ):
        raise WorkerFailure("candidate_protocol_error: incomplete Seismic Spike single-card report")
    candidate = report.get("candidate")
    if not isinstance(candidate, dict) or candidate.get("card_definition") != "RAV-SEISMIC-SPIKE":
        raise WorkerFailure("candidate_protocol_error: wrong Seismic Spike identity")
    traces = report.get("traces")
    if not isinstance(traces, list) or len(traces) != 1:
        raise WorkerFailure("candidate_protocol_error: one Seismic Spike trace is required")
    trace = traces[0]
    if (
        not isinstance(trace, dict)
        or trace.get("id") != "rav-seismic-spike-destroys-land-adds-two-red"
        or trace.get("passed") is not True
        or trace.get("terminal") != "Winner(PlayerId(0))"
        or trace.get("winner_seat") != 0
        or trace.get("rejected_moves") != 0
        or trace.get("land_destroyed_by_spike") is not True
        or trace.get("land_definition") != "RAV-MOUNTAIN"
        or trace.get("mana_added_by_spike") != {"color": "red", "amount": 2}
        or trace.get("followup_spell_paid_with_added_mana") is not True
        or trace.get("combat_source_is_followup_slasher") is not True
    ):
        raise WorkerFailure("candidate_protocol_error: Spike did not destroy, add two red, and continue through a clean combat win")
    spike = trace.get("spike_object")
    land = trace.get("land_object")
    slasher = trace.get("followup_slasher_object")
    if not all(isinstance(value, int) for value in (spike, land, slasher)) or len({spike, land, slasher}) != 3:
        raise WorkerFailure("candidate_protocol_error: Spike, target, and follow-up Slasher identities are not distinct")
    events = trace.get("events")
    expected_kinds = [
        "SpellManaPaid", "SpellCast", "CardDestroyed", "CardMoved", "ManaAdded",
        "SpellResolved", "CardMoved", "SpellManaPaid", "SpellCast", "SpellResolved",
        "CardMoved", "DamageDealtToPlayer", "GameEnded",
    ]
    if not isinstance(events, list) or [event.get("kind") for event in events if isinstance(event, dict)] != expected_kinds:
        raise WorkerFailure("candidate_protocol_error: incomplete Seismic Spike resolution and follow-up trace")
    sequences = [event.get("sequence") for event in events]
    if any(not isinstance(sequence, int) for sequence in sequences) or sequences != sorted(sequences):
        raise WorkerFailure("candidate_protocol_error: Seismic Spike event sequence is malformed")
    facts = [event.get("facts", {}) for event in events]
    canonical = [event.get("canonical", "") for event in events]
    if (
        facts[0].get("objects") != [spike] or facts[0].get("seats") != [0]
        or facts[0].get("amount") != 4 or facts[0].get("mana_colors") != ["Red"] * 4
        or facts[1].get("objects") != [spike] or facts[1].get("seats") != [0]
        or facts[2].get("objects") != [spike, land]
        or f"source: ObjectId({spike})" not in canonical[2]
        or f"card: ObjectId({land})" not in canonical[2]
        or facts[3].get("objects") != [land] or facts[3].get("zone") != "Graveyard"
        or facts[4].get("seats") != [0] or facts[4].get("zone") != "Red" or facts[4].get("amount") != 2
        or facts[5].get("objects") != [spike]
        or facts[6].get("objects") != [spike] or facts[6].get("zone") != "Graveyard"
        or facts[7].get("objects") != [slasher] or facts[7].get("seats") != [0]
        or facts[7].get("amount") != 2 or facts[7].get("mana_colors") != ["Red", "Red"]
        or facts[8].get("objects") != [slasher] or facts[8].get("seats") != [0]
        or facts[9].get("objects") != [slasher]
        or facts[10].get("objects") != [slasher] or facts[10].get("zone") != "Battlefield"
        or facts[11].get("objects") != [slasher]
        or facts[11].get("seats") != [1] or facts[11].get("amount", 0) <= 0
        or facts[12].get("seats") != [0]
        or trace.get("combat_damage_source") != slasher
        or sequences[0] >= sequences[1] or sequences[1] >= sequences[2]
        or sequences[2] >= sequences[3] or sequences[3] >= sequences[4]
        or sequences[4] >= sequences[5] or sequences[5] >= sequences[6]
        or sequences[6] >= sequences[7] or sequences[7] >= sequences[8]
        or sequences[8] >= sequences[9] or sequences[9] >= sequences[10]
        or sequences[10] >= sequences[11] or sequences[11] >= sequences[12]
        or trace.get("ordered_event_sequence") != sequences
        or trace.get("accepted_moves", 0) <= 0 or trace.get("turns", 0) <= 0
    ):
        raise WorkerFailure("candidate_protocol_error: Spike payment, ordered destruction/mana, follow-up cast, or combat facts differ")
    mutation = report.get("mutation_sensitivity", {})
    required = (
        "effect_order_mutation_detected",
        "land_destroyed_by_live_spike",
        "two_red_mana_added_by_live_spike",
        "followup_spell_paid_with_spike_mana",
    )
    if not isinstance(mutation, dict) or any(mutation.get(key) is not True for key in required):
        raise WorkerFailure("candidate_protocol_error: Seismic Spike semantic mutation controls are incomplete")


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
        raise FileNotFoundError(f"trusted Seismic Spike authority source is missing: {authority}")
    if not shutil.which("docker"):
        raise FileNotFoundError("Docker is required for Magic candidate isolation")
    verify_workspace(workspace, manifest)
    scoring_workspace = make_scoring_snapshot(workspace, manifest_data["files"], output_root / "candidate-workspace")
    candidate_hash = sha256((scoring_workspace / TARGET).read_bytes())
    name = f"cardbench-magic-seismic_spike-{uuid.uuid4().hex[:16]}"
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
            "error": "candidate compile or Seismic Spike reference trace failed",
            "diagnostics": stderr.decode(errors="replace")[-12000:],
        }
    else:
        result = parse_report(stdout)
        result["candidate"] = {
            "kind": candidate_kind,
            "card_definition": "RAV-SEISMIC-SPIKE",
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
