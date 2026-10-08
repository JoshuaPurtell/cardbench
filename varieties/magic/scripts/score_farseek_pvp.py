#!/usr/bin/env python3
"""Run Farseek in a bounded offline candidate worker and score its trace."""

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
MATERIALIZER = Path(__file__).with_name("materialize_farseek_task.py")
TARGET = Path("varieties/magic/sets/ravnica_city_of_guilds/src/cards/farseek.rs")
AUTHORITY_SOURCE = Path("varieties/magic/session/src/bin/rav_farseek_pvp.rs")
DEFAULT_WORKER_IMAGE = "rust@sha256:d7482085ff5b415f84dba5647ae71606650bdef00db7aeb69f4b3d170c3e4082"
MAX_OUTPUT_BYTES = 2 * 1024 * 1024


class WorkerFailure(RuntimeError):
    pass


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def verify_workspace(workspace: Path, manifest: Path) -> None:
    subprocess.run([sys.executable, str(MATERIALIZER), "--workspace", str(workspace), "--manifest", str(manifest), "--verify"], check=True, cwd=REPO)


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
        "cp /trusted-authority.rs /work/varieties/magic/session/src/bin/rav_farseek_pvp.rs && "
        "cd /work && cargo run --offline --locked -j 2 --manifest-path varieties/magic/Cargo.toml "
        "-p cardbench-magic-session --bin rav-farseek-pvp --" + candidate_flag,
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
    raise WorkerFailure("candidate_protocol_error: no valid Farseek PvP result")


def validate_report(report: dict) -> None:
    if (
        report.get("task_id") != "cardbench/magic/card"
        or report.get("passed") is not True
        or report.get("pilot_card_passed") is not True
        or report.get("deterministic_replay") is not True
        or report.get("cards_passed") != 1
        or report.get("cards_credited") != 0
        or report.get("cards_total") != 291
        or report.get("printings_total") != 306
        or report.get("coverage_complete") is not False
        or report.get("full_reward") is not False
        or report.get("harbor_reward") != 0.0
        or report.get("pilot_reward") != 0.0
        or report.get("viewer_scope") != "Judge"
        or report.get("card") != {"collector_number": 163, "definition_id": "RAV-FARSEEK", "name": "Farseek"}
    ):
        raise WorkerFailure("candidate_protocol_error: incomplete Farseek 1/291 diagnostic result")
    candidate = report.get("candidate")
    if not isinstance(candidate, dict) or candidate.get("card_definition") != "RAV-FARSEEK":
        raise WorkerFailure("candidate_protocol_error: wrong Farseek candidate identity")
    traces = report.get("traces")
    if not isinstance(traces, list) or len(traces) != 1:
        raise WorkerFailure("candidate_protocol_error: one complete Farseek search trace is required")
    validate_trace(traces[0])
    mutation = report.get("mutation_sensitivity", {})
    required = (
        "forest_allowed_by_search_mutation_detected_in_game",
        "tapped_destination_removed_mutation_detected_in_game",
        "private_selection_reveal_mutation_detected_in_game",
    )
    if not isinstance(mutation, dict) or any(mutation.get(key) is not True for key in required):
        raise WorkerFailure("candidate_protocol_error: Farseek semantic controls are incomplete")


def validate_trace(trace: object) -> None:
    if not isinstance(trace, dict) or trace.get("trace_id") != "rav-farseek-search-nonforest-basic-land-tapped" or trace.get("terminal") != "Winner(PlayerId(0))" or trace.get("winner") != 0 or trace.get("rejected_policy_moves") != 0:
        raise WorkerFailure("candidate_protocol_error: Farseek did not finish a clean full game")
    mechanics = trace.get("mechanics")
    if not isinstance(mechanics, dict):
        raise WorkerFailure("candidate_protocol_error: Farseek mechanics are missing")
    options = mechanics.get("candidate_definitions")
    if (
        mechanics.get("mana_cost") != "{1}{G}"
        or not isinstance(mechanics.get("paid_mana_colors"), list)
        or len(mechanics["paid_mana_colors"]) != 2
        or "Green" not in mechanics["paid_mana_colors"]
        or mechanics.get("private_to_controller") is not True
        or mechanics.get("hidden_from_opponent") is not True
        or not isinstance(options, list)
        or set(options) != {"RAV-PLAINS", "RAV-ISLAND", "RAV-SWAMP", "RAV-MOUNTAIN"}
        or mechanics.get("forest_excluded") is not True
        or mechanics.get("selected_definition") != "RAV-PLAINS"
        or mechanics.get("selected_is_plains") is not True
        or mechanics.get("destination") != "BattlefieldTapped"
        or mechanics.get("selected_tapped_at_resolution") is not True
        or mechanics.get("selected_land_battlefield") is not True
        or mechanics.get("selected_land_revealed") is not False
        or mechanics.get("library_shuffled_after_search") is not True
        or mechanics.get("combat_to_terminal_win") is not True
    ):
        raise WorkerFailure("candidate_protocol_error: Farseek search choice, privacy, destination, or PvP result differs")
    events = trace.get("events")
    required_kinds = {"SpellManaPaid", "SpellCast", "SpellResolved", "DecisionOpened", "DecisionCompleted", "LibrarySearchResolved", "CardMoved", "LibraryShuffled", "AttackersDeclared", "DamageDealtToPlayer", "GameEnded"}
    if not isinstance(events, list) or not required_kinds.issubset({event.get("kind") for event in events if isinstance(event, dict)}):
        raise WorkerFailure("candidate_protocol_error: Farseek payment, search, tapped entry, shuffle, or combat receipts are missing")
    sequences = [event.get("sequence") for event in events if isinstance(event, dict)]
    if any(not isinstance(sequence, int) for sequence in sequences) or sequences != sorted(sequences) or len(set(sequences)) != len(sequences):
        raise WorkerFailure("candidate_protocol_error: Farseek event order is malformed")
    opened = next(event for event in events if event.get("kind") == "DecisionOpened")
    resolved = next(event for event in events if event.get("kind") == "LibrarySearchResolved")
    if "visibility: Private" not in opened.get("canonical", "") or "BattlefieldTapped" not in resolved.get("canonical", ""):
        raise WorkerFailure("candidate_protocol_error: Farseek decision was not private or destination was not tapped")

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
        raise FileNotFoundError(f"trusted Farseek authority source is missing: {authority}")
    if not shutil.which("docker"):
        raise FileNotFoundError("Docker is required for Magic candidate isolation")
    verify_workspace(workspace, manifest)
    scoring_workspace = make_scoring_snapshot(workspace, manifest_data["files"], output_root / "candidate-workspace")
    candidate_hash = sha256((scoring_workspace / TARGET).read_bytes())
    name = f"cardbench-magic-farseek-{uuid.uuid4().hex[:16]}"
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
            "error": "candidate compile or Farseek reference trace failed",
            "diagnostics": stderr.decode(errors="replace")[-12000:],
        }
        (output_root / "result.json").write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
        print(json.dumps(result))
        return 1
    report = parse_report(stdout)
    report["candidate"].update({"kind": candidate_kind, "source_sha256": candidate_hash})
    report.update({"sealed_authority": False, "worker_image": worker_image, "sandbox": {"network": "none", "rootfs": "read-only", "pids_limit": 128}})
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
