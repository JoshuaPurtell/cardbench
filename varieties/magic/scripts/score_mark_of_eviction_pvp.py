#!/usr/bin/env python3
"""Validate and score the isolated Mark of Eviction PvP diagnostic."""
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
MATERIALIZER = Path(__file__).with_name("materialize_mark_of_eviction_task.py")
TARGET = Path("varieties/magic/sets/ravnica_city_of_guilds/src/cards/mark_of_eviction.rs")
AUTHORITY = Path("varieties/magic/session/src/bin/rav_mark_of_eviction_pvp.rs")
DEFAULT_WORKER_IMAGE = "rust@sha256:d7482085ff5b415f84dba5647ae71606650bdef00db7aeb69f4b3d170c3e4082"


class WorkerFailure(RuntimeError):
    pass


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def validate_report(report: dict) -> None:
    expected_card = {"collector_number": 58, "definition_id": "RAV-MARK-OF-EVICTION", "name": "Mark of Eviction"}
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
        raise WorkerFailure("candidate_protocol_error: incomplete Mark of Eviction diagnostic report")
    candidate = report.get("candidate")
    if not isinstance(candidate, dict) or candidate.get("card_definition") != expected_card["definition_id"]:
        raise WorkerFailure("candidate_protocol_error: wrong Mark of Eviction identity")
    traces = report.get("traces")
    if not isinstance(traces, list) or len(traces) != 1:
        raise WorkerFailure("candidate_protocol_error: one Mark of Eviction full-game trace is required")
    trace = traces[0]
    mechanics = trace.get("mechanics", {}) if isinstance(trace, dict) else {}
    if (
        trace.get("trace_id") != "rav-mark-of-eviction-attached-creature-upkeep-return-pvp"
        or trace.get("terminal") != "Winner(PlayerId(0))"
        or trace.get("rejected_moves") != 0
        or mechanics.get("mana_cost") != "{U}"
        or mechanics.get("paid_colors") != ["Blue"]
        or mechanics.get("attachment_observed") is not True
        or mechanics.get("attached_creature_definition") != "RAV-WAR-TORCH-GOBLIN"
        or mechanics.get("trigger_controller") != 0
        or mechanics.get("opponent_upkeep_passed_before_trigger") is not True
        or mechanics.get("creature_returned_to_owner_hand") is not True
        or mechanics.get("mark_returned_to_its_owner_hand") is not True
        or mechanics.get("mark_owner_hand_receipt") is not True
        or mechanics.get("creature_owner_hand_receipt") is not True
        or mechanics.get("attached_aura_final_zone") != "Hand"
        or mechanics.get("controller_won") is not True
    ):
        raise WorkerFailure("candidate_protocol_error: Aura attachment, creature-and-Aura owner returns, or terminal trace is incomplete")
    events = trace.get("events")
    if not isinstance(events, list) or len(events) < 10 or any(
        not isinstance(item, dict)
        or not isinstance(item.get("sequence"), int)
        or not isinstance(item.get("kind"), str)
        or not isinstance(item.get("canonical"), str)
        for item in events
    ):
        raise WorkerFailure("candidate_protocol_error: malformed Mark of Eviction event projection")
    sequences = [item["sequence"] for item in events]
    if sequences != sorted(sequences):
        raise WorkerFailure("candidate_protocol_error: projected event order is unstable")
    canonical = [item["canonical"] for item in events]
    aura = f"ObjectId({mechanics.get('aura')})"
    target = f"ObjectId({mechanics.get('attached_creature')})"
    checks = [
        ("SpellManaPaid", aura),
        ("SpellCast", aura),
        ("AttachmentEstablishedWithoutContinuousEffect", aura),
        ("StepBegan", "PlayerId(1), step: Upkeep"),
        ("TriggeredAbilityStacked", aura),
        ("CardMoved", f"{aura}, to: Hand"),
        ("CardMoved", f"{target}, to: Hand"),
        ("AbilityResolved", aura),
        ("GameEnded", "winner: Some(PlayerId(0))"),
    ]
    for first, second in checks:
        if not any(first in item and second in item for item in canonical):
            raise WorkerFailure(f"candidate_protocol_error: missing ordered {first} receipt for Mark or its creature")
    indices = [next(i for i, item in enumerate(canonical) if first in item and second in item) for first, second in checks]
    if indices != sorted(indices):
        raise WorkerFailure("candidate_protocol_error: Mark attachment/upkeep/return lifecycle order differs")
    mutation = report.get("mutation_sensitivity", {})
    required = (
        "aura_attachment_removed_detected",
        "upkeep_trigger_removed_detected",
        "wrong_upkeep_controller_detected",
        "return_instruction_removed_detected",
        "aura_group_return_removed_detected",
        "wrong_exact_cost_detected",
    )
    if any(mutation.get(key) is not True for key in required) or mutation.get("sealed_mutation_authority") is not False:
        raise WorkerFailure("candidate_protocol_error: Mark of Eviction gameplay mutations must be detected")


def parse_report(stdout: bytes) -> dict:
    for line in reversed(stdout.decode(errors="replace").splitlines()):
        try:
            value = json.loads(line)
        except json.JSONDecodeError:
            continue
        if value.get("schema_version") == "cardbench.magic.card-pvp.check.v1":
            validate_report(value)
            return value
    raise WorkerFailure("candidate_protocol_error: no Mark of Eviction PvP report")


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
    name = f"cardbench-magic-mark-of-eviction-{uuid.uuid4().hex[:12]}"
    flag = " --candidate" if candidate_kind == "agent" else ""
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
        "cp -R /submission/. /work/ && cp /trusted-authority.rs /work/varieties/magic/session/src/bin/rav_mark_of_eviction_pvp.rs && "
        "cd /work && cargo run --offline --locked -j 2 --manifest-path varieties/magic/Cargo.toml "
        "-p cardbench-magic-session --bin rav-mark-of-eviction-pvp --" + flag,
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
        with tempfile.TemporaryDirectory(prefix="mark-of-eviction-candidate-") as temporary:
            candidate = snapshot(workspace, manifest["files"], Path(temporary) / "workspace")
            code, stdout, stderr = run_worker(candidate, (REPO / AUTHORITY).resolve(), image, args.candidate_kind, args.timeout)
        (args.output_root / "worker.stdout").write_bytes(stdout)
        (args.output_root / "worker.stderr").write_bytes(stderr)
        if code == 0:
            result = parse_report(stdout)
            result["candidate"] = {"kind": args.candidate_kind, "card_definition": "RAV-MARK-OF-EVICTION", "source_sha256": sha256((workspace / TARGET).read_bytes()), "materialized_workspace_sha256": manifest["workspace_sha256"]}
            result["candidate_kind"] = args.candidate_kind
            result["starting_workspace_sha256"] = manifest["workspace_sha256"]
            result["sealed_authority"] = False
            result["worker_image"] = image
            result["sandbox"] = {"network": "none", "rootfs": "read-only", "pids_limit": 128}
            result["harbor_reward"] = 0.0
            result["pilot_reward"] = 0.0
            write_result(args.output_root, result)
            return 0
        write_result(args.output_root, {"schema_version":"cardbench.magic.card-pvp.check.v1", "task_id":"cardbench/magic/card", "candidate":{"kind":args.candidate_kind, "source_sha256":sha256((workspace / TARGET).read_bytes())}, "passed":False, "pilot_card_passed":False, "cards_passed":0, "cards_total":291, "printings_total":306, "cards_credited":0, "coverage_complete":False, "full_reward":False, "harbor_reward":0.0, "pilot_reward":0.0, "sealed_authority":False, "error":"candidate compile or Mark of Eviction trace gate failed", "diagnostics":stderr.decode(errors="replace")[-12000:]})
        return 1
    except Exception as error:
        write_result(args.output_root, {"schema_version":"cardbench.magic.card-pvp.check.v1", "task_id":"cardbench/magic/card", "passed":False, "pilot_card_passed":False, "cards_passed":0, "cards_total":291, "printings_total":306, "cards_credited":0, "coverage_complete":False, "full_reward":False, "harbor_reward":0.0, "pilot_reward":0.0, "sealed_authority":False, "error":f"{type(error).__name__}: {error}"})
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
