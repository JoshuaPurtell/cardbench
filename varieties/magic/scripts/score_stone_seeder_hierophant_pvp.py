#!/usr/bin/env python3
"""Fail-closed scorer and isolated runner for the Stone-Seeder task."""

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
MATERIALIZER = Path(__file__).with_name("materialize_stone_seeder_hierophant_task.py")
TARGET = Path("varieties/magic/sets/ravnica_city_of_guilds/src/cards/stone_seeder_hierophant.rs")
AUTHORITY_SOURCE = Path("varieties/magic/session/src/bin/rav_stone_seeder_hierophant_pvp.rs")
DEFAULT_WORKER_IMAGE = "rust@sha256:d7482085ff5b415f84dba5647ae71606650bdef00db7aeb69f4b3d170c3e4082"
MAX_OUTPUT_BYTES = 2 * 1024 * 1024


class WorkerFailure(RuntimeError):
    pass


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def validate_report(report: dict) -> None:
    if (
        report.get("schema_version") != "cardbench.magic.card-pvp.check.v1"
        or report.get("task_id") != "cardbench/magic/card"
        or report.get("passed") is not True
        or report.get("pilot_card_passed") is not True
        or report.get("replay_deterministic") is not True
        or report.get("cards_passed") != 1
        or report.get("cards_credited") != 0
        or report.get("cards_total") != 291
        or report.get("printings_total") != 306
        or report.get("coverage_complete") is not False
        or report.get("full_reward") is not False
        or report.get("harbor_reward") != 0.0
        or report.get("pilot_reward") != 0.0
        or report.get("viewer_scope") != "Judge"
        or report.get("card") != {"collector_number": 184, "definition_id": "RAV-STONE-SEEDER-HIEROPHANT", "name": "Stone-Seeder Hierophant"}
        or report.get("rules_authority", {}).get("comprehensive_rules") != "2026-08-07"
    ):
        raise WorkerFailure("candidate_protocol_error: incomplete Stone-Seeder report")
    guard = report.get("source_guard", {})
    if guard.get("side_effect_mutation_parse_valid") is not True or guard.get("side_effect_mutation_rejected") is not True or guard.get("candidate_calls_before_guard") != 0:
        raise WorkerFailure("candidate_protocol_error: side-effect source was not rejected before candidate calls")
    traces = report.get("traces")
    if not isinstance(traces, list) or len(traces) != 1 or not isinstance(traces[0], dict):
        raise WorkerFailure("candidate_protocol_error: one full-game Stone-Seeder trace is required")
    trace = traces[0]
    if trace.get("id") != "rav-stone-seeder-hierophant-landfall-target-untap-full-game" or trace.get("passed") is not True or trace.get("terminal") != "Winner(PlayerId(0))" or trace.get("winner_seat") != 0 or trace.get("rejected_moves") != 0:
        raise WorkerFailure("candidate_protocol_error: Stone-Seeder did not complete its full-game trace")
    mechanics = trace.get("mechanics", {})
    own_lands = mechanics.get("controller_land_entries_after_entry", [])
    opponent_lands = mechanics.get("opponent_land_entries_after_entry", [])
    paid = mechanics.get("paid_mana_colors", [])
    activation_count = mechanics.get("activation_count")
    if (
        mechanics.get("definition_id") != "RAV-STONE-SEEDER-HIEROPHANT"
        or mechanics.get("mana_cost") != "{2}{G}{G}"
        or len(paid) != 4
        or paid.count("Green") != 4
        or mechanics.get("base_power") != 1
        or mechanics.get("base_toughness") != 1
        or mechanics.get("has_mana_ability") is not False
        or not own_lands
        or not opponent_lands
        or mechanics.get("trigger_count") != len(own_lands)
        or mechanics.get("source_untap_receipt_count") != len(own_lands)
        or mechanics.get("opponent_land_triggered_source") is not False
        or not isinstance(activation_count, int)
        or activation_count < 1
        or mechanics.get("activation_source_tapped_for_each_cost") is not True
        or mechanics.get("target_land_untap_receipt_count") != activation_count
        or mechanics.get("terminal_pilot_win") is not True
    ):
        raise WorkerFailure("candidate_protocol_error: Stone-Seeder cast, controller landfall, or target-land activation differs")
    sequence = trace.get("ordered_event_sequence")
    events = trace.get("events")
    if not isinstance(sequence, list) or any(not isinstance(item, int) for item in sequence) or sequence != sorted(sequence):
        raise WorkerFailure("candidate_protocol_error: ordered event sequence is malformed")
    if not isinstance(events, list) or not all(isinstance(event, dict) for event in events):
        raise WorkerFailure("candidate_protocol_error: projected event list is malformed")
    kinds = [event.get("kind") for event in events]
    if kinds.count("TriggeredAbilityStacked") != mechanics["trigger_count"] or kinds.count("AbilityActivated") < activation_count:
        raise WorkerFailure("candidate_protocol_error: trigger or activated-ability receipts are missing")
    mutations = report.get("mutation_sensitivity", {})
    for key in (
        "printed_cost_mutation_detected", "base_power_mutation_detected", "wrong_land_scope_mutation_detected",
        "wrong_trigger_effect_mutation_detected", "trigger_binding_removal_detected", "activation_tap_cost_mutation_detected",
        "activation_target_mutation_detected", "activation_effect_mutation_detected", "activation_binding_removal_detected",
        "parse_valid_side_effect_source_rejected_before_candidate_calls",
    ):
        if mutations.get(key) is not True:
            raise WorkerFailure(f"candidate_protocol_error: mutation control missing: {key}")


def verify_workspace(workspace: Path, manifest: Path) -> None:
    subprocess.run([sys.executable, str(MATERIALIZER), "--workspace", str(workspace), "--manifest", str(manifest), "--verify"], check=True, cwd=REPO)


def make_snapshot(workspace: Path, files: dict[str, str], destination: Path) -> Path:
    if destination.is_symlink():
        raise WorkerFailure("candidate_snapshot_path_is_symlink")
    if destination.exists():
        shutil.rmtree(destination)
    destination.mkdir(parents=True)
    for relative in sorted(files):
        source = workspace / Path(relative)
        if source.is_symlink() or not source.is_file():
            raise WorkerFailure(f"candidate_snapshot_source_missing: {relative}")
        output = destination / Path(relative)
        output.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(source, output)
    return destination


def run_bounded(command: list[str], timeout: int, name: str) -> tuple[int, bytes, bytes]:
    process = subprocess.Popen(command, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    assert process.stdout is not None and process.stderr is not None
    selector = selectors.DefaultSelector()
    selector.register(process.stdout, selectors.EVENT_READ, "stdout")
    selector.register(process.stderr, selectors.EVENT_READ, "stderr")
    output, diagnostics = bytearray(), bytearray()
    started = time.monotonic()
    try:
        while selector.get_map() or process.poll() is None:
            if time.monotonic() - started > timeout:
                raise WorkerFailure("candidate_timeout: bounded Stone-Seeder worker timed out")
            for key, _ in selector.select(0.2):
                chunk = os.read(key.fileobj.fileno(), 65536)
                if not chunk:
                    selector.unregister(key.fileobj)
                    continue
                (output if key.data == "stdout" else diagnostics).extend(chunk)
                if len(output) + len(diagnostics) > MAX_OUTPUT_BYTES:
                    raise WorkerFailure("candidate_output_limit: worker output exceeded 2 MiB")
        return process.wait(), bytes(output), bytes(diagnostics)
    except BaseException:
        process.kill()
        process.wait()
        subprocess.run(["docker", "rm", "-f", name], capture_output=True, check=False)
        raise


def parse_report(stdout: bytes) -> dict:
    for line in reversed(stdout.decode(errors="replace").splitlines()):
        try:
            value = json.loads(line)
        except json.JSONDecodeError:
            continue
        if value.get("schema_version") == "cardbench.magic.card-pvp.check.v1":
            validate_report(value)
            return value
    raise WorkerFailure("candidate_protocol_error: no valid Stone-Seeder report")


def sandbox_command(workspace: Path, registry: Path, authority: Path, name: str, image: str, candidate_kind: str) -> list[str]:
    if authority.is_symlink() or not authority.is_file():
        raise WorkerFailure("trusted_authority_source_missing")
    candidate_flag = " --candidate" if candidate_kind == "agent" else ""
    return [
        "docker", "run", "--rm", "--name", name, "--network", "none", "--read-only", "--cap-drop", "ALL",
        "--security-opt", "no-new-privileges", "--pids-limit", "128", "--memory", "3g", "--cpus", "2",
        "--user", "1000:1000", "--tmpfs", "/work:rw,exec,size=2147483648,uid=1000,gid=1000",
        "--tmpfs", "/tmp:rw,noexec,size=67108864,uid=1000,gid=1000", "--mount", f"type=bind,src={workspace},dst=/submission,readonly",
        "--mount", f"type=bind,src={registry},dst=/usr/local/cargo/registry,readonly", "--mount", f"type=bind,src={authority.resolve()},dst=/trusted-authority.rs,readonly",
        "--env", "HOME=/tmp/home", "--env", "CARGO_HOME=/usr/local/cargo", "--env", "RUSTUP_HOME=/usr/local/rustup",
        "--env", "CARGO_NET_OFFLINE=true", "--env", "CARGO_BUILD_JOBS=2", "--env", "RAYON_NUM_THREADS=2", image, "sh", "-c",
        "cp -R /submission/. /work/ && cp /trusted-authority.rs /work/varieties/magic/session/src/bin/rav_stone_seeder_hierophant_pvp.rs && cd /work && cargo run --offline --locked -j 2 --manifest-path varieties/magic/Cargo.toml -p cardbench-magic-session --bin rav-stone-seeder-hierophant-pvp --" + candidate_flag,
    ]


def score(workspace: Path, manifest: Path, output_root: Path, candidate_kind: str) -> int:
    output_root.mkdir(parents=True, exist_ok=True)
    manifest_data = json.loads(manifest.read_text(encoding="utf-8"))
    image = os.environ.get("CARDBENCH_MAGIC_WORKER_IMAGE", DEFAULT_WORKER_IMAGE)
    if not (image.startswith("rust@sha256:") or image.startswith("sha256:")):
        raise ValueError("CARDBENCH_MAGIC_WORKER_IMAGE must be an immutable digest reference")
    registry = Path.home() / ".cargo" / "registry"
    if not registry.is_dir():
        raise FileNotFoundError(f"offline Cargo registry cache is missing: {registry}")
    authority = REPO / AUTHORITY_SOURCE
    if not shutil.which("docker"):
        raise FileNotFoundError("Docker is required for Magic candidate isolation")
    verify_workspace(workspace, manifest)
    snapshot = make_snapshot(workspace, manifest_data["files"], output_root / "candidate-workspace")
    candidate_hash = sha256((snapshot / TARGET).read_bytes())
    name = f"cardbench-magic-stone-seeder-{uuid.uuid4().hex[:16]}"
    code, stdout, stderr = run_bounded(sandbox_command(snapshot, registry.resolve(), authority, name, image, candidate_kind), 900, name)
    (output_root / "worker.stdout").write_bytes(stdout)
    (output_root / "worker.stderr").write_bytes(stderr)
    if code != 0:
        result = {"schema_version":"cardbench.magic.card-pvp.check.v1","task_id":"cardbench/magic/card","candidate":{"kind":candidate_kind,"source_sha256":candidate_hash},"passed":False,"pilot_card_passed":False,"cards_passed":0,"cards_total":291,"printings_total":306,"cards_credited":0,"coverage_complete":False,"full_reward":False,"harbor_reward":0.0,"pilot_reward":0.0,"sealed_authority":False,"worker_image":image,"sandbox":{"network":"none","rootfs":"read-only","pids_limit":128},"error":"candidate compile or Stone-Seeder reference trace failed","diagnostics":stderr.decode(errors="replace")[-12000:]}
    else:
        result = parse_report(stdout)
        result["candidate"] = {"kind":candidate_kind,"card_definition":"RAV-STONE-SEEDER-HIEROPHANT","source_sha256":candidate_hash,"materialized_workspace_sha256":manifest_data["workspace_sha256"]}
        result["candidate_kind"] = candidate_kind
        result["starting_workspace_sha256"] = manifest_data["workspace_sha256"]
        result["sealed_authority"] = False
        result["worker_image"] = image
        result["sandbox"] = {"network":"none","rootfs":"read-only","pids_limit":128}
        result["harbor_reward"] = 0.0
        result["pilot_reward"] = 0.0
    (output_root / "result.json").write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
    (output_root / "reward.txt").write_text(f"{result['harbor_reward']:.8f}\n", encoding="utf-8")
    return 0 if result.get("passed") is True and result.get("pilot_card_passed") is True else 1


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--workspace", type=Path)
    parser.add_argument("--manifest", type=Path)
    parser.add_argument("--output-root", type=Path)
    parser.add_argument("--candidate-kind", choices=["agent", "reference"], default="agent")
    parser.add_argument("--validate-report", type=Path)
    args = parser.parse_args()
    if args.validate_report:
        validate_report(json.loads(args.validate_report.read_text(encoding="utf-8")))
        print("Stone-Seeder Hierophant report validates; rewards and credit remain zero")
        return 0
    if not args.workspace or not args.manifest or not args.output_root:
        parser.error("--workspace, --manifest, and --output-root are required unless --validate-report is used")
    try:
        return score(args.workspace.resolve(), args.manifest.resolve(), args.output_root.resolve(), args.candidate_kind)
    except Exception as error:
        args.output_root.mkdir(parents=True, exist_ok=True)
        result = {"schema_version":"cardbench.magic.card-pvp.check.v1","task_id":"cardbench/magic/card","passed":False,"pilot_card_passed":False,"cards_passed":0,"cards_total":291,"printings_total":306,"cards_credited":0,"coverage_complete":False,"full_reward":False,"harbor_reward":0.0,"sealed_authority":False,"error":str(error)}
        (args.output_root / "result.json").write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
        (args.output_root / "reward.txt").write_text("0.0\n", encoding="utf-8")
        print(f"Magic candidate verifier failed: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
