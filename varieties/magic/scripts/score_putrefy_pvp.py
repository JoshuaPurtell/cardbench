#!/usr/bin/env python3
"""Run the Putrefy pilot in an isolated, networkless Rust worker."""
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
MAGIC = REPO / "varieties" / "magic"
MATERIALIZER = Path(__file__).with_name("materialize_putrefy_task.py")
TARGET = Path("varieties/magic/sets/ravnica_city_of_guilds/src/cards/putrefy.rs")
AUTHORITY = Path("varieties/magic/session/src/bin/rav_putrefy_pvp.rs")
DEFAULT_WORKER_IMAGE = "rust@sha256:d7482085ff5b415f84dba5647ae71606650bdef00db7aeb69f4b3d170c3e4082"


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
        or report.get("cards_total") != 291
        or report.get("printings_total") != 306
        or report.get("cards_credited") != 0
        or report.get("coverage_complete") is not False
        or report.get("full_reward") is not False
        or report.get("harbor_reward") != 0.0
        or report.get("pilot_reward") != 0.0
        or report.get("viewer_scope") != "Judge"
        or report.get("card") != {"collector_number": 221, "definition_id": "RAV-PUTREFY", "name": "Putrefy"}
        or report.get("rules_authority", {}).get("comprehensive_rules") != "2026-08-07"
    ):
        raise WorkerFailure("candidate_protocol_error: incomplete Putrefy diagnostic report")
    traces = report.get("traces")
    if not isinstance(traces, list) or len(traces) != 1:
        raise WorkerFailure("candidate_protocol_error: one Putrefy PvP trace is required")
    trace = traces[0]
    if (
        not isinstance(trace, dict)
        or trace.get("id") != "rav-putrefy-artifact-creature-no-regeneration"
        or trace.get("passed") is not True
        or trace.get("terminal") != "Winner(PlayerId(0))"
        or trace.get("winner_seat") != 0
        or trace.get("decks") != ["rav_putrefy_golgari_midrange", "rav_glass_golem_colorless"]
        or trace.get("rejected_moves") != 0
        or trace.get("target_zone") != "Some(Graveyard)"
        or trace.get("putrefy_zone") != "Some(Graveyard)"
        or trace.get("regeneration_shield_created") is not True
        or trace.get("regeneration_shield_used") is not False
    ):
        raise WorkerFailure("candidate_protocol_error: Putrefy must destroy the shielded artifact creature and reach terminal PvP")
    payment = trace.get("putrefy_payment", {})
    colors = payment.get("colors")
    if not isinstance(colors, list) or len(colors) != 3 or payment.get("black_paid") is not True or payment.get("green_paid") is not True or payment.get("total_symbols_paid") != 3 or not {"Black", "Green"}.issubset(set(colors)):
        raise WorkerFailure("candidate_protocol_error: actual {1}{B}{G} Putrefy payment is missing")
    events = trace.get("events")
    if not isinstance(events, list) or not events or any(not isinstance(item, dict) or not isinstance(item.get("sequence"), int) or not isinstance(item.get("kind"), str) or not isinstance(item.get("canonical"), str) for item in events):
        raise WorkerFailure("candidate_protocol_error: malformed projected event list")
    sequences = [item["sequence"] for item in events]
    if sequences != sorted(sequences):
        raise WorkerFailure("candidate_protocol_error: projected event order is unstable")
    positions = trace.get("ordered_event_sequence")
    if not isinstance(positions, list) or len(positions) != 9 or any(not isinstance(index, int) or index < 0 or index >= len(events) for index in positions):
        raise WorkerFailure("candidate_protocol_error: expected Putrefy, destruction, and terminal event citations are missing")
    by_kind = {index: events[index]["kind"] for index in positions}
    required = {"RegenerationShieldCreated", "SpellManaPaid", "SpellCast", "SpellResolved", "CardDestroyed", "CardMoved", "AttackersDeclared", "DamageDealtToPlayer", "GameEnded"}
    if set(by_kind.values()) != required:
        raise WorkerFailure("candidate_protocol_error: cited events do not cover shield, spell, destruction, combat, and game-end receipts")
    canonical = [item["canonical"] for item in events]
    spell = f"ObjectId({trace.get('putrefy_object')})"
    target = f"ObjectId({trace.get('target_glass_golem_object')})"
    attacker = f"ObjectId({trace.get('terminal_attacker_object')})"
    if not any("SpellManaPaid" in event and spell in event and "Black" in event and "Green" in event for event in canonical):
        raise WorkerFailure("candidate_protocol_error: Putrefy colored-payment event is missing")
    if not any("SpellCast" in event and spell in event and "PlayerId(0)" in event for event in canonical):
        raise WorkerFailure("candidate_protocol_error: Putrefy cast event is missing")
    if not any("CardDestroyed" in event and f"source: {spell}" in event and f"card: {target}" in event for event in canonical):
        raise WorkerFailure("candidate_protocol_error: the targeted Glass Golem was not destroyed by Putrefy")
    if not any("RegenerationShieldCreated" in event and f"target: {target}" in event for event in canonical):
        raise WorkerFailure("candidate_protocol_error: target regeneration-shield receipt is missing")
    if any("RegenerationShieldUsed" in event and f"target: {target}" in event for event in canonical):
        raise WorkerFailure("candidate_protocol_error: Putrefy consumed the target's regeneration shield")
    if not any("CardMoved" in event and f"card: {target}" in event and "to: Graveyard" in event for event in canonical):
        raise WorkerFailure("candidate_protocol_error: destroyed target graveyard transition is missing")
    if not any("DamageDealtToPlayer" in event and f"source: {attacker}" in event and "player: PlayerId(1)" in event for event in canonical):
        raise WorkerFailure("candidate_protocol_error: terminal combat damage receipt is missing")
    mutation = report.get("mutation_sensitivity", {})
    if any(mutation.get(key) is not True for key in ("destruction_instruction_removed_detected", "regeneration_bypass_removed_detected", "printed_cost_changed_detected")) or mutation.get("sealed_mutation_authority") is not False:
        raise WorkerFailure("candidate_protocol_error: Putrefy gameplay mutations must be detected")


def parse_report(stdout: bytes) -> dict:
    for line in reversed(stdout.decode(errors="replace").splitlines()):
        try:
            value = json.loads(line)
        except json.JSONDecodeError:
            continue
        if value.get("schema_version") == "cardbench.magic.card-pvp.check.v1":
            validate_report(value)
            return value
    raise WorkerFailure("candidate_protocol_error: no Putrefy PvP report")


def verify_workspace(workspace: Path, manifest: Path) -> dict:
    subprocess.run([sys.executable, str(MATERIALIZER), "--workspace", str(workspace), "--manifest", str(manifest), "--verify"], check=True, cwd=REPO)
    return json.loads(manifest.read_text(encoding="utf-8"))


def snapshot(workspace: Path, manifest: dict, destination: Path) -> Path:
    destination.mkdir(parents=True, exist_ok=True)
    for relative in sorted(manifest["files"]):
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
    name = f"cardbench-magic-putrefy-{uuid.uuid4().hex[:12]}"
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
        "cp -R /submission/. /work/ && cp /trusted-authority.rs /work/varieties/magic/session/src/bin/rav_putrefy_pvp.rs && cd /work && cargo run --offline --locked -j 2 --manifest-path varieties/magic/Cargo.toml -p cardbench-magic-session --bin rav-putrefy-pvp --" + candidate_flag,
    ]
    completed = subprocess.run(command, stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=timeout, check=False)
    return completed.returncode, completed.stdout, completed.stderr


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--workspace", required=True, type=Path)
    parser.add_argument("--manifest", required=True, type=Path)
    parser.add_argument("--output-root", required=True, type=Path)
    parser.add_argument("--candidate-kind", choices=("reference", "agent"), default="agent")
    parser.add_argument("--timeout", type=int, default=900)
    args = parser.parse_args()
    args.output_root.mkdir(parents=True, exist_ok=True)
    try:
        manifest = verify_workspace(args.workspace.resolve(), args.manifest.resolve())
        image = os.environ.get("CARDBENCH_MAGIC_WORKER_IMAGE", DEFAULT_WORKER_IMAGE)
        if not (image.startswith("rust@sha256:") or image.startswith("sha256:")):
            raise ValueError("CARDBENCH_MAGIC_WORKER_IMAGE must be an immutable digest reference")
        with tempfile.TemporaryDirectory(prefix="putrefy-candidate-") as temporary:
            candidate = snapshot(args.workspace.resolve(), manifest, Path(temporary) / "workspace")
            authority = (REPO / AUTHORITY).resolve()
            code, stdout, stderr = run_worker(candidate, authority, image, args.candidate_kind, args.timeout)
        (args.output_root / "worker.stdout").write_bytes(stdout)
        (args.output_root / "worker.stderr").write_bytes(stderr)
        if code == 0:
            result = parse_report(stdout)
            result["candidate"] = {"kind": args.candidate_kind, "card_definition": "RAV-PUTREFY", "source_sha256": sha256((args.workspace.resolve() / TARGET).read_bytes()), "materialized_workspace_sha256": manifest["workspace_sha256"]}
            result["candidate_kind"] = args.candidate_kind
            result["starting_workspace_sha256"] = manifest["workspace_sha256"]
            result["sealed_authority"] = False
            result["worker_image"] = image
            result["sandbox"] = {"network": "none", "rootfs": "read-only", "pids_limit": 128}
            result["harbor_reward"] = 0.0
            result["pilot_reward"] = 0.0
            result["full_reward"] = False
        else:
            result = {"schema_version":"cardbench.magic.card-pvp.check.v1", "task_id":"cardbench/magic/card", "candidate":{"kind":args.candidate_kind,"source_sha256":sha256((args.workspace.resolve()/TARGET).read_bytes())}, "passed":False, "pilot_card_passed":False, "cards_passed":0, "cards_total":291, "printings_total":306, "cards_credited":0, "coverage_complete":False, "full_reward":False, "harbor_reward":0.0, "pilot_reward":0.0, "sealed_authority":False, "error":"candidate compile or Putrefy trace gate failed", "diagnostics":stderr.decode(errors="replace")[-12000:]}
        (args.output_root / "result.json").write_text(json.dumps(result, indent=2)+"\n", encoding="utf-8")
        (args.output_root / "reward.txt").write_text("0.00000000\n", encoding="utf-8")
        return 0 if result.get("passed") else 1
    except Exception as error:
        result = {"schema_version":"cardbench.magic.card-pvp.check.v1", "task_id":"cardbench/magic/card", "passed":False, "pilot_card_passed":False, "cards_passed":0, "cards_total":291, "printings_total":306, "cards_credited":0, "coverage_complete":False, "full_reward":False, "harbor_reward":0.0, "pilot_reward":0.0, "sealed_authority":False, "error":f"{type(error).__name__}: {error}"}
        (args.output_root / "result.json").write_text(json.dumps(result, indent=2)+"\n", encoding="utf-8")
        (args.output_root / "reward.txt").write_text("0.00000000\n", encoding="utf-8")
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
