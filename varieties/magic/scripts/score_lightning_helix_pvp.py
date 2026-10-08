#!/usr/bin/env python3
"""Run the Lightning Helix pilot in a networkless, resource-limited worker."""

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
MATERIALIZER = Path(__file__).with_name("materialize_lightning_helix_task.py")
TARGET = Path(
    "varieties/magic/sets/ravnica_city_of_guilds/src/cards/lightning_helix.rs"
)
AUTHORITY_SOURCE = Path(
    "varieties/magic/session/src/bin/rav_lightning_helix_pvp.rs"
)
DEFAULT_WORKER_IMAGE = (
    "rust@sha256:d7482085ff5b415f84dba5647ae71606650bdef00db7aeb69f4b3d170c3e4082"
)
MAX_OUTPUT_BYTES = 2 * 1024 * 1024


class WorkerFailure(RuntimeError):
    pass


def sha256_bytes(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def verify_workspace(workspace: Path, manifest: Path) -> None:
    subprocess.run(
        [
            sys.executable,
            str(MATERIALIZER),
            "--workspace",
            str(workspace),
            "--manifest",
            str(manifest),
            "--verify",
        ],
        check=True,
        cwd=REPO,
    )


def make_scoring_snapshot(
    workspace: Path, manifest_data: dict, snapshot: Path
) -> Path:
    """Copy only manifest-listed task files into the isolated worker input."""
    if snapshot.is_symlink():
        raise WorkerFailure("candidate_snapshot_path_is_symlink")
    if snapshot.exists():
        shutil.rmtree(snapshot)
    snapshot.mkdir(parents=True)
    for relative in sorted(manifest_data["files"]):
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


def sandbox_command(
    workspace: Path,
    registry: Path,
    authority: Path,
    name: str,
    image: str,
    candidate_kind: str,
) -> list[str]:
    if authority.is_symlink() or not authority.is_file():
        raise WorkerFailure("trusted_authority_source_missing")
    candidate_flag = " --candidate" if candidate_kind == "agent" else ""
    return [
        "docker",
        "run",
        "--rm",
        "--name",
        name,
        "--network",
        "none",
        "--read-only",
        "--cap-drop",
        "ALL",
        "--security-opt",
        "no-new-privileges",
        "--pids-limit",
        "128",
        "--memory",
        "3g",
        "--cpus",
        "2",
        "--user",
        "1000:1000",
        "--tmpfs",
        "/work:rw,exec,size=2147483648,uid=1000,gid=1000",
        "--tmpfs",
        "/tmp:rw,noexec,size=67108864,uid=1000,gid=1000",
        "--mount",
        f"type=bind,src={workspace},dst=/submission,readonly",
        "--mount",
        f"type=bind,src={registry},dst=/usr/local/cargo/registry,readonly",
        "--mount",
        f"type=bind,src={authority.resolve()},dst=/trusted-authority.rs,readonly",
        "--env",
        "HOME=/tmp/home",
        "--env",
        "CARGO_HOME=/usr/local/cargo",
        "--env",
        "RUSTUP_HOME=/usr/local/rustup",
        "--env",
        "CARGO_NET_OFFLINE=true",
        "--env",
        "CARGO_BUILD_JOBS=2",
        "--env",
        "RAYON_NUM_THREADS=2",
        image,
        "sh",
        "-c",
        "cp -R /submission/. /work/ && cp /trusted-authority.rs /work/varieties/magic/session/src/bin/rav_lightning_helix_pvp.rs && cd /work && cargo run --offline --locked -j 2 --manifest-path varieties/magic/Cargo.toml -p cardbench-magic-session --bin rav-lightning-helix-pvp --"
        + candidate_flag,
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
    raise WorkerFailure("candidate_protocol_error: no valid Lightning Helix PvP result")


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
    ):
        raise WorkerFailure("candidate_protocol_error: incomplete single-card pilot report")

    candidate = report.get("candidate")
    if not isinstance(candidate, dict) or candidate.get("card_definition") != "RAV-LIGHTNING-HELIX":
        raise WorkerFailure("candidate_protocol_error: wrong card identity in report")

    traces = report.get("traces")
    if not isinstance(traces, list) or len(traces) != 2:
        raise WorkerFailure("candidate_protocol_error: both target traces are required")
    expected = {
        "rav-lightning-helix-player-target": "DamageDealtToPlayer",
        "rav-lightning-helix-creature-target": "DamageDealtToPermanent",
    }
    for trace in traces:
        if not isinstance(trace, dict) or trace.get("id") not in expected:
            raise WorkerFailure("candidate_protocol_error: unexpected target trace")
        if not trace.get("passed") or trace.get("terminal") != "Winner(PlayerId(0))":
            raise WorkerFailure("candidate_protocol_error: trace did not reach a clean terminal win")
        if trace.get("winner_seat") != 0 or trace.get("rejected_moves") != 0:
            raise WorkerFailure("candidate_protocol_error: trace contains rejected moves")
        events = trace.get("events")
        if not isinstance(events, list) or len(events) != 6:
            raise WorkerFailure("candidate_protocol_error: incomplete projected trace")
        event_kinds = [event.get("kind") for event in events if isinstance(event, dict)]
        damage_kind = expected[trace["id"]]
        if event_kinds != [
            "SpellCast",
            damage_kind,
            "LifeGained",
            "SpellResolved",
            "CardMoved",
            "GameEnded",
        ]:
            raise WorkerFailure("candidate_protocol_error: ordered event trace differs")
        sequences = [event.get("sequence") for event in events]
        if (
            any(not isinstance(sequence, int) for sequence in sequences)
            or sequences != sorted(sequences)
            or trace.get("ordered_event_sequence") != sequences[:5]
        ):
            raise WorkerFailure("candidate_protocol_error: event sequence is malformed")

        helix_object = trace.get("helix_object")
        facts = [event.get("facts", {}) for event in events]
        if (
            not isinstance(helix_object, int)
            or facts[0].get("objects") != [helix_object]
            or facts[1].get("amount") != 3
            or facts[1].get("objects", [None])[0] != helix_object
            or facts[2].get("amount") != 3
            or facts[2].get("seats") != [0]
            or facts[3].get("objects") != [helix_object]
            or facts[4].get("objects") != [helix_object]
            or facts[4].get("zone") != "Graveyard"
        ):
            raise WorkerFailure("candidate_protocol_error: damage, gain, or zone facts differ")
        if trace["id"] == "rav-lightning-helix-player-target":
            if facts[1].get("seats") != [1] or trace.get("target") != {"kind": "player", "seat": 1}:
                raise WorkerFailure("candidate_protocol_error: player target differs")
        else:
            target = trace.get("target")
            objects = facts[1].get("objects", [])
            if (
                not isinstance(target, dict)
                or target.get("kind") != "opponent_creature"
                or len(objects) != 2
                or target.get("object") != objects[1]
            ):
                raise WorkerFailure("candidate_protocol_error: creature target differs")
        life_before = trace.get("life_before_target_damage")
        resolution_life = trace.get("resolution_life")
        expected_life = (
            [life_before[0] + 3, life_before[1] - 3]
            if trace["id"] == "rav-lightning-helix-player-target"
            else [life_before[0] + 3, life_before[1]]
        ) if isinstance(life_before, list) and len(life_before) == 2 else None
        if resolution_life != expected_life:
            raise WorkerFailure("candidate_protocol_error: post-resolution life differs")

    mutation = report.get("mutation_sensitivity", {})
    required_mutations = (
        "player_damage_3_to_2_detected",
        "creature_damage_3_to_2_detected",
        "life_gain_3_to_2_detected",
        "damage_and_gain_order_swap_detected",
    )
    if not isinstance(mutation, dict) or any(mutation.get(key) is not True for key in required_mutations):
        raise WorkerFailure("candidate_protocol_error: semantic mutation controls are incomplete")


def score(workspace: Path, manifest: Path, output_root: Path, candidate_kind: str) -> int:
    output_root.mkdir(parents=True, exist_ok=True)
    manifest_data = json.loads(manifest.read_text(encoding="utf-8"))
    worker_image = os.environ.get("CARDBENCH_MAGIC_WORKER_IMAGE", DEFAULT_WORKER_IMAGE)
    if not (worker_image.startswith("rust@sha256:") or worker_image.startswith("sha256:")):
        raise ValueError("CARDBENCH_MAGIC_WORKER_IMAGE must be an immutable digest reference")
    cargo_home = Path.home() / ".cargo"
    cargo_registry = cargo_home / "registry"
    if not cargo_registry.is_dir():
        raise FileNotFoundError(f"offline Cargo registry cache is missing: {cargo_registry}")
    authority = REPO / AUTHORITY_SOURCE
    if authority.is_symlink() or not authority.is_file():
        raise FileNotFoundError(f"trusted Lightning Helix authority source is missing: {authority}")
    docker = shutil.which("docker")
    if not docker:
        raise FileNotFoundError("Docker is required for Magic candidate isolation")

    verify_workspace(workspace, manifest)
    scoring_workspace = make_scoring_snapshot(
        workspace, manifest_data, output_root / "candidate-workspace"
    )
    candidate_path = scoring_workspace / TARGET
    candidate_hash = sha256_bytes(candidate_path.read_bytes())
    name = f"cardbench-magic-helix-{uuid.uuid4().hex[:16]}"
    command = sandbox_command(
        scoring_workspace,
        cargo_registry.resolve(),
        authority,
        name,
        worker_image,
        candidate_kind,
    )
    code, stdout, stderr = run_bounded(command, timeout=900, name=name)
    (output_root / "worker.stdout").write_bytes(stdout)
    (output_root / "worker.stderr").write_bytes(stderr)
    if code != 0:
        excerpt = stderr.decode(errors="replace")[-12000:]
        result = {
            "schema_version": "cardbench.magic.card-pvp.check.v1",
            "task_id": "cardbench/magic/card",
            "candidate": {"kind": candidate_kind, "source_sha256": candidate_hash},
            "passed": False,
            "pilot_card_passed": False,
            "cards_passed": 0,
            "cards_total": 291,
            "coverage_complete": False,
            "full_reward": False,
            "harbor_reward": 0.0,
            "pilot_reward": 0.0,
            "sealed_authority": False,
            "worker_image": worker_image,
            "sandbox": {"network": "none", "rootfs": "read-only", "pids_limit": 128},
            "error": "candidate compile or Lightning Helix reference trace failed",
            "diagnostics": excerpt,
        }
    else:
        result = parse_report(stdout)
        result["candidate"] = {
            "kind": candidate_kind,
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
    (output_root / "result.json").write_text(
        json.dumps(result, indent=2) + "\n", encoding="utf-8"
    )
    (output_root / "reward.txt").write_text(f"{result['harbor_reward']:.8f}\n")
    print(
        f"candidate_passed={result.get('pilot_card_passed', False)} "
        f"cards={result.get('cards_passed', 0)}/{result.get('cards_total', 291)} "
        f"coverage_complete=false"
    )
    return 0 if result.get("passed") is True and result.get("pilot_card_passed") is True else 1


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--workspace", required=True, type=Path)
    parser.add_argument("--manifest", required=True, type=Path)
    parser.add_argument("--output-root", required=True, type=Path)
    parser.add_argument("--candidate-kind", choices=["agent", "reference"], default="agent")
    args = parser.parse_args()
    try:
        return score(
            args.workspace.resolve(),
            args.manifest.resolve(),
            args.output_root.resolve(),
            args.candidate_kind,
        )
    except Exception as error:
        args.output_root.mkdir(parents=True, exist_ok=True)
        result = {
            "schema_version": "cardbench.magic.card-pvp.check.v1",
            "task_id": "cardbench/magic/card",
            "passed": False,
            "pilot_card_passed": False,
            "cards_passed": 0,
            "cards_total": 291,
            "coverage_complete": False,
            "full_reward": False,
            "harbor_reward": 0.0,
            "sealed_authority": False,
            "error": str(error),
        }
        (args.output_root / "result.json").write_text(json.dumps(result, indent=2) + "\n")
        (args.output_root / "reward.txt").write_text("0.0\n")
        print(f"Magic candidate verifier failed: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
