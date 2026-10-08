#!/usr/bin/env python3
"""Validate and score the isolated Peel from Reality PvP diagnostic."""
from __future__ import annotations

import argparse
import hashlib
import json
import os
import shutil
import subprocess
import sys
import tempfile
import uuid
from pathlib import Path

REPO = Path(__file__).resolve().parents[3]
MATERIALIZER = Path(__file__).with_name("materialize_peel_from_reality_task.py")
TARGET = Path("varieties/magic/sets/ravnica_city_of_guilds/src/cards/peel_from_reality.rs")
AUTHORITY = Path("varieties/magic/session/src/bin/rav_peel_from_reality_pvp.rs")
DEFAULT_WORKER_IMAGE = "rust@sha256:d7482085ff5b415f84dba5647ae71606650bdef00db7aeb69f4b3d170c3e4082"


class WorkerFailure(RuntimeError):
    pass


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def validate_report(report: dict) -> None:
    expected_card = {"collector_number": 61, "definition_id": "RAV-PEEL-FROM-REALITY", "name": "Peel from Reality"}
    if (
        report.get("schema_version") != "cardbench.magic.card-pvp.check.v1"
        or report.get("task_id") != "cardbench/magic/card"
        or report.get("passed") is not True
        or report.get("pilot_card_passed") is not True
        or report.get("replay_deterministic") is not True
        or report.get("cards_passed") != 1
        or report.get("cards_total") != 291
        or report.get("printings_total") != 306
        or report.get("cards_credited") != 0
        or report.get("coverage_complete") is not False
        or report.get("full_reward") is not False
        or report.get("harbor_reward") != 0.0
        or report.get("pilot_reward") != 0.0
        or report.get("viewer_scope") != "Judge"
        or report.get("card") != expected_card
        or report.get("rules_authority", {}).get("comprehensive_rules") != "2026-08-07"
    ):
        raise WorkerFailure("candidate_protocol_error: incomplete Peel from Reality diagnostic report")
    candidate = report.get("candidate")
    if not isinstance(candidate, dict) or candidate.get("card_definition") != expected_card["definition_id"]:
        raise WorkerFailure("candidate_protocol_error: wrong Peel from Reality identity")
    traces = report.get("traces")
    if not isinstance(traces, list) or len(traces) != 1:
        raise WorkerFailure("candidate_protocol_error: one Peel from Reality full-game trace is required")
    trace = traces[0]
    mechanics = trace.get("mechanics", {}) if isinstance(trace, dict) else {}
    paid = mechanics.get("paid_colors", [])
    if (
        trace.get("trace_id") != "rav-peel-from-reality-two-controller-bounce-pvp"
        or trace.get("terminal") != "Winner(PlayerId(0))"
        or trace.get("rejected_moves") != 0
        or mechanics.get("mana_cost") != "{1}{U}"
        or not isinstance(paid, list)
        or len(paid) != 2
        or paid.count("Blue") < 1
        or mechanics.get("target_order") != "controlled_then_opponent"
        or mechanics.get("both_targets_returned_in_order") is not True
        or mechanics.get("controlled_target_owner_hand_seen") is not True
        or mechanics.get("opponent_target_owner_hand_seen") is not True
        or mechanics.get("both_targets_final_zone") != "Hand"
        or mechanics.get("controller_won") is not True
    ):
        raise WorkerFailure("candidate_protocol_error: target order, payment, owner zones, or terminal trace is incomplete")
    events = trace.get("events")
    if not isinstance(events, list) or len(events) < 7 or any(
        not isinstance(item, dict)
        or not isinstance(item.get("sequence"), int)
        or not isinstance(item.get("kind"), str)
        or not isinstance(item.get("canonical"), str)
        for item in events
    ):
        raise WorkerFailure("candidate_protocol_error: malformed Peel from Reality event projection")
    sequences = [item["sequence"] for item in events]
    if sequences != sorted(sequences):
        raise WorkerFailure("candidate_protocol_error: projected event order is unstable")
    canonical = [item["canonical"] for item in events]
    peel = f"ObjectId({mechanics.get('spell')})"
    own = f"ObjectId({mechanics.get('controlled_target')})"
    opponent = f"ObjectId({mechanics.get('opponent_target')})"
    if not any("SpellManaPaid" in item and peel in item and all(color in item for color in paid) for item in canonical):
        raise WorkerFailure("candidate_protocol_error: exact Peel mana payment is missing")
    if not any("SpellCast" in item and peel in item and "PlayerId(0)" in item for item in canonical):
        raise WorkerFailure("candidate_protocol_error: Peel cast receipt is missing")
    if not any("CardMoved" in item and own in item and "to: Hand" in item for item in canonical):
        raise WorkerFailure("candidate_protocol_error: controlled creature hand move is missing")
    if not any("CardMoved" in item and opponent in item and "to: Hand" in item for item in canonical):
        raise WorkerFailure("candidate_protocol_error: opponent creature hand move is missing")
    if not any("GameEnded" in item and "winner: Some(PlayerId(0))" in item for item in canonical):
        raise WorkerFailure("candidate_protocol_error: terminal seat-zero win receipt is missing")
    mutation = report.get("mutation_sensitivity", {})
    required_mutations = (
        "controlled_target_scope_mutation_detected",
        "opponent_target_scope_mutation_detected",
        "second_return_instruction_mutation_detected",
        "wrong_exact_cost_mutation_detected",
    )
    if any(mutation.get(key) is not True for key in required_mutations) or mutation.get("sealed_mutation_authority") is not False:
        raise WorkerFailure("candidate_protocol_error: Peel from Reality gameplay mutations must be detected")


def parse_report(stdout: bytes) -> dict:
    for line in reversed(stdout.decode(errors="replace").splitlines()):
        try:
            value = json.loads(line)
        except json.JSONDecodeError:
            continue
        if value.get("schema_version") == "cardbench.magic.card-pvp.check.v1":
            validate_report(value)
            return value
    raise WorkerFailure("candidate_protocol_error: no Peel from Reality PvP report")


def verify_workspace(workspace: Path, manifest: Path) -> dict:
    subprocess.run([sys.executable, str(MATERIALIZER), "--workspace", str(workspace), "--manifest", str(manifest), "--verify"], check=True, cwd=REPO)
    return json.loads(manifest.read_text(encoding="utf-8"))


def snapshot(workspace: Path, files: dict[str, str], destination: Path) -> Path:
    destination.mkdir(parents=True, exist_ok=True)
    for relative in sorted(files):
        source = workspace / Path(relative)
        if source.is_symlink() or not source.is_file():
            raise WorkerFailure(f"candidate_snapshot_source_missing: {relative}")
        target = destination / Path(relative)
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(source, target)
    return destination


def run_worker(workspace: Path, authority: Path, image: str, candidate_kind: str, timeout: int) -> tuple[int, bytes, bytes]:
    registry = Path.home() / ".cargo" / "registry"
    if not registry.is_dir():
        raise FileNotFoundError(f"offline Cargo registry cache is missing: {registry}")
    if not shutil.which("docker"):
        raise FileNotFoundError("Docker is required for isolated Magic candidate scoring")
    name = f"cardbench-magic-peel-{uuid.uuid4().hex[:12]}"
    candidate_flag = " --candidate" if candidate_kind == "agent" else ""
    command = [
        "docker", "run", "--rm", "--name", name, "--network", "none", "--read-only",
        "--cap-drop", "ALL", "--security-opt", "no-new-privileges", "--pids-limit", "128",
        "--memory", "3g", "--cpus", "2", "--user", "1000:1000",
        "--tmpfs", "/work:rw,exec,size=2147483648,uid=1000,gid=1000",
        "--tmpfs", "/tmp:rw,noexec,size=67108864,uid=1000,gid=1000",
        "--mount", f"type=bind,src={workspace},dst=/submission,readonly",
        "--mount", f"type=bind,src={registry.resolve()},dst=/usr/local/cargo/registry,readonly",
        "--mount", f"type=bind,src={authority.resolve()},dst=/trusted-authority.rs,readonly",
        "--env", "HOME=/tmp/home", "--env", "CARGO_HOME=/usr/local/cargo", "--env", "RUSTUP_HOME=/usr/local/rustup",
        "--env", "CARGO_NET_OFFLINE=true", "--env", "CARGO_BUILD_JOBS=2", "--env", "RAYON_NUM_THREADS=2",
        image, "sh", "-c",
        "cp -R /submission/. /work/ && cp /trusted-authority.rs /work/varieties/magic/session/src/bin/rav_peel_from_reality_pvp.rs && "
        "cd /work && cargo run --offline --locked -j 2 --manifest-path varieties/magic/Cargo.toml "
        "-p cardbench-magic-session --bin rav-peel-from-reality-pvp --" + candidate_flag,
    ]
    completed = subprocess.run(command, stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=timeout, check=False)
    return completed.returncode, completed.stdout, completed.stderr


def write_result(output_root: Path, result: dict) -> None:
    output_root.mkdir(parents=True, exist_ok=True)
    (output_root / "result.json").write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
    (output_root / "reward.txt").write_text("0.00000000\n", encoding="utf-8")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--workspace", type=Path)
    parser.add_argument("--manifest", type=Path)
    parser.add_argument("--output-root", required=True, type=Path)
    parser.add_argument("--candidate-kind", choices=("reference", "agent"), default="agent")
    parser.add_argument("--timeout", type=int, default=900)
    parser.add_argument("--report-file", type=Path, help="validate a local authority JSON-lines capture without running a worker")
    args = parser.parse_args()
    try:
        if args.report_file:
            report = parse_report(args.report_file.read_bytes())
            report["candidate_kind"] = report.get("candidate", {}).get("kind", args.candidate_kind)
            report["sealed_authority"] = False
            report["harbor_reward"] = 0.0
            report["pilot_reward"] = 0.0
            write_result(args.output_root, report)
            return 0
        if args.workspace is None or args.manifest is None:
            raise ValueError("--workspace and --manifest are required when --report-file is not provided")
        workspace = args.workspace.resolve()
        manifest_path = args.manifest.resolve()
        manifest = verify_workspace(workspace, manifest_path)
        image = os.environ.get("CARDBENCH_MAGIC_WORKER_IMAGE", DEFAULT_WORKER_IMAGE)
        if not (image.startswith("rust@sha256:") or image.startswith("sha256:")):
            raise ValueError("CARDBENCH_MAGIC_WORKER_IMAGE must be an immutable digest reference")
        with tempfile.TemporaryDirectory(prefix="peel-from-reality-candidate-") as temporary:
            candidate = snapshot(workspace, manifest["files"], Path(temporary) / "workspace")
            code, stdout, stderr = run_worker(candidate, (REPO / AUTHORITY).resolve(), image, args.candidate_kind, args.timeout)
        (args.output_root / "worker.stdout").write_bytes(stdout)
        (args.output_root / "worker.stderr").write_bytes(stderr)
        if code == 0:
            result = parse_report(stdout)
            result["candidate"] = {"kind": args.candidate_kind, "card_definition": "RAV-PEEL-FROM-REALITY", "source_sha256": sha256((workspace / TARGET).read_bytes()), "materialized_workspace_sha256": manifest["workspace_sha256"]}
            result["candidate_kind"] = args.candidate_kind
            result["starting_workspace_sha256"] = manifest["workspace_sha256"]
            result["sealed_authority"] = False
            result["worker_image"] = image
            result["sandbox"] = {"network": "none", "rootfs": "read-only", "pids_limit": 128}
            result["harbor_reward"] = 0.0
            result["pilot_reward"] = 0.0
            write_result(args.output_root, result)
            return 0
        write_result(args.output_root, {"schema_version":"cardbench.magic.card-pvp.check.v1", "task_id":"cardbench/magic/card", "candidate":{"kind":args.candidate_kind, "source_sha256":sha256((workspace / TARGET).read_bytes())}, "passed":False, "pilot_card_passed":False, "cards_passed":0, "cards_total":291, "printings_total":306, "cards_credited":0, "coverage_complete":False, "full_reward":False, "harbor_reward":0.0, "pilot_reward":0.0, "sealed_authority":False, "error":"candidate compile or Peel from Reality trace gate failed", "diagnostics":stderr.decode(errors="replace")[-12000:]})
        return 1
    except Exception as error:
        write_result(args.output_root, {"schema_version":"cardbench.magic.card-pvp.check.v1", "task_id":"cardbench/magic/card", "passed":False, "pilot_card_passed":False, "cards_passed":0, "cards_total":291, "printings_total":306, "cards_credited":0, "coverage_complete":False, "full_reward":False, "harbor_reward":0.0, "pilot_reward":0.0, "sealed_authority":False, "error":f"{type(error).__name__}: {error}"})
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
