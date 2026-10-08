#!/usr/bin/env python3
"""Run the bounded offline Agrus Kos candidate authority."""

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
MATERIALIZER = Path(__file__).with_name("materialize_agrus_kos_wojek_veteran_task.py")
TARGET = Path("varieties/magic/sets/ravnica_city_of_guilds/src/cards/agrus_kos_wojek_veteran.rs")
AUTHORITY = Path("varieties/magic/session/src/bin/rav_agrus_kos_wojek_veteran_pvp.rs")
BIN = "rav_agrus_kos_wojek_veteran_pvp"
IMAGE = "rust@sha256:d7482085ff5b415f84dba5647ae71606650bdef00db7aeb69f4b3d170c3e4082"
MAX_OUTPUT = 2 * 1024 * 1024


class WorkerFailure(RuntimeError):
    pass


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def verify_workspace(workspace: Path, manifest: Path) -> dict:
    subprocess.run(
        [sys.executable, str(MATERIALIZER), "--workspace", str(workspace), "--manifest", str(manifest), "--verify"],
        check=True,
        cwd=REPO,
    )
    return json.loads(manifest.read_text(encoding="utf-8"))


def snapshot_workspace(workspace: Path, files: dict[str, str], destination: Path) -> Path:
    if destination.is_symlink():
        raise WorkerFailure("candidate_snapshot_path_is_symlink")
    if destination.exists():
        shutil.rmtree(destination)
    destination.mkdir(parents=True)
    for relative in sorted(files):
        source = workspace / Path(relative)
        if source.is_symlink() or not source.is_file():
            raise WorkerFailure(f"candidate_snapshot_source_missing: {relative}")
        target = destination / Path(relative)
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
                raise WorkerFailure("candidate_timeout: offline authority exceeded its deadline")
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
        raise WorkerFailure("trusted_agrus_kos_authority_missing")
    candidate_flag = "candidate" if candidate_kind == "agent" else ""
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
        f"cp /trusted-authority.rs /work/varieties/magic/session/src/bin/{BIN}.rs && "
        "cd /work && cargo run --offline --locked -j 2 --manifest-path varieties/magic/Cargo.toml "
        f"-p cardbench-magic-session --bin {BIN} --{candidate_flag}",
    ]


def validate_trace(trace: object) -> None:
    if not isinstance(trace, dict) or trace.get("trace_id") != "rav-agrus-kos-color-specific-attacker-modifiers-full-game" or trace.get("passed") is not True:
        raise WorkerFailure("candidate_protocol_error: missing Agrus Kos trace")
    if trace.get("terminal") != "Winner(PlayerId(0))" or trace.get("winner_seat") != 0 or trace.get("rejected_moves") != 0:
        raise WorkerFailure("candidate_protocol_error: duel did not end in a clean pilot win")
    if trace.get("deck_p0") != "4 Agrus Kos + 4 Boros Recruit + 4 Goblin Spelunkers + 4 Watchwolf + 44 RAV basic lands (60 shuffled cards)" or trace.get("deck_p1") != "60 Plains (60 shuffled cards)":
        raise WorkerFailure("candidate_protocol_error: shuffled deck contract changed")
    expected_after = {"agrus": [5, 5], "red_white": [3, 3], "red": [4, 2], "white": [3, 5], "nonattacking_red": [2, 2]}
    expected_before = {"agrus": [3, 3], "red_white": [1, 1], "red": [2, 2], "white": [3, 3], "nonattacking_red": [2, 2]}
    if trace.get("cohort_base_power_toughness") != expected_before or trace.get("cohort_after_trigger_power_toughness") != expected_after or trace.get("cleanup_power_toughness") != expected_before:
        raise WorkerFailure("candidate_protocol_error: color-specific attacker or Cleanup characteristics differ")
    if trace.get("cleanup_modifier_expirations") != 6 or trace.get("mechanics", {}).get("red_white_attacker_gains_both") is not True:
        raise WorkerFailure("candidate_protocol_error: double-color ruling or Cleanup receipts are missing")
    events = trace.get("events")
    if not isinstance(events, list) or len(events) < 20 or trace.get("event_count", 0) < len(events):
        raise WorkerFailure("candidate_protocol_error: ordered full-game event trace is incomplete")


def validate_report(report: dict) -> None:
    expected = {"collector_number": 190, "printings": 1, "definition_id": "RAV-AGRUS-KOS-WOJEK-VETERAN", "name": "Agrus Kos, Wojek Veteran"}
    if report.get("schema_version") != "cardbench.magic.card-pvp.check.v1" or report.get("task_id") != "cardbench/magic/card" or report.get("expansion") != "RAV" or report.get("passed") is not True or report.get("pilot_card_passed") is not True or report.get("replay_deterministic") is not True or report.get("cards_passed") != 1 or report.get("cards_credited") != 0 or report.get("cards_total") != 291 or report.get("printings_total") != 306 or report.get("coverage_complete") is not False or report.get("full_reward") is not False or report.get("harbor_reward") != 0.0 or report.get("pilot_reward") != 0.0 or report.get("viewer_scope") != "Judge" or report.get("card") != expected:
        raise WorkerFailure("candidate_protocol_error: incomplete Agrus Kos zero-credit diagnostic")
    if report.get("candidate_source_validation", {}).get("normalized_literal_source_checked_before_candidate_calls") is not True:
        raise WorkerFailure("candidate_protocol_error: pinned source guard is missing")
    traces = report.get("traces")
    if not isinstance(traces, list) or len(traces) != 1:
        raise WorkerFailure("candidate_protocol_error: exactly one full-game trace is required")
    validate_trace(traces[0])
    mutations = report.get("mutation_sensitivity", {})
    keys = ("wrong_mana_cost_detected", "wrong_power_detected", "wrong_toughness_detected", "missing_trigger_detected", "wrong_trigger_condition_detected", "red_modifier_mutation_detected", "white_modifier_mutation_detected")
    if any(mutations.get(key) is not True for key in keys) or mutations.get("sealed_mutation_authority") is not False:
        raise WorkerFailure("candidate_protocol_error: semantic mutation controls are incomplete")


def parse_report(stdout: bytes) -> dict:
    for line in reversed(stdout.decode(errors="replace").splitlines()):
        try:
            report = json.loads(line)
        except json.JSONDecodeError:
            continue
        if report.get("schema_version") == "cardbench.magic.card-pvp.check.v1":
            validate_report(report)
            return report
    raise WorkerFailure("candidate_protocol_error: no valid Agrus Kos report")


def score(workspace: Path, manifest: Path, output_root: Path, candidate_kind: str) -> int:
    output_root.mkdir(parents=True, exist_ok=True)
    data = verify_workspace(workspace, manifest)
    image = os.environ.get("CARDBENCH_MAGIC_WORKER_IMAGE", IMAGE)
    if not (image.startswith("rust@sha256:") or image.startswith("sha256:")):
        raise ValueError("CARDBENCH_MAGIC_WORKER_IMAGE must be an immutable digest reference")
    registry = Path.home() / ".cargo" / "registry"
    if not registry.is_dir() or not shutil.which("docker"):
        raise FileNotFoundError("offline Rust registry cache and Docker are required for candidate isolation")
    authority = REPO / AUTHORITY
    snapshot = snapshot_workspace(workspace, data["files"], output_root / "candidate-workspace")
    digest = sha256((snapshot / TARGET).read_bytes())
    container = f"cardbench-magic-agrus-kos-{uuid.uuid4().hex[:16]}"
    command = sandbox_command(snapshot, registry.resolve(), authority, container, image, candidate_kind)
    code, stdout, stderr = run_bounded(command, 900, container)
    (output_root / "worker.stdout").write_bytes(stdout)
    (output_root / "worker.stderr").write_bytes(stderr)
    if code:
        result = {"schema_version": "cardbench.magic.card-pvp.check.v1", "task_id": "cardbench/magic/card", "candidate": {"kind": candidate_kind, "source_sha256": digest}, "passed": False, "cards_total": 291, "printings_total": 306, "cards_credited": 0, "coverage_complete": False, "full_reward": False, "harbor_reward": 0.0, "pilot_reward": 0.0, "sealed_authority": False, "worker_image": image, "error": "candidate compile or Agrus Kos trace failed", "diagnostics": stderr.decode(errors="replace")[-12000:]}
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
