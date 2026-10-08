#!/usr/bin/env python3
"""Run the Carrion Howler pilot in a bounded, networkless Rust worker."""

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
MATERIALIZER = Path(__file__).with_name("materialize_carrion_howler_task.py")
TARGET = Path("varieties/magic/sets/ravnica_city_of_guilds/src/cards/carrion_howler.rs")
AUTHORITY = Path("varieties/magic/session/src/bin/rav_carrion_howler_pvp.rs")
DEFAULT_WORKER_IMAGE = "rust@sha256:d7482085ff5b415f84dba5647ae71606650bdef00db7aeb69f4b3d170c3e4082"


class WorkerFailure(RuntimeError):
    pass


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def verify_workspace(workspace: Path, manifest: Path) -> dict:
    subprocess.run([sys.executable, str(MATERIALIZER), "--workspace", str(workspace), "--manifest", str(manifest), "--verify"], check=True, cwd=REPO)
    value = json.loads(manifest.read_text(encoding="utf-8"))
    return value


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


def validate_report(report: dict) -> None:
    if (report.get("schema_version") != "cardbench.magic.card-pvp.check.v1" or report.get("task_id") != "cardbench/magic/card" or report.get("passed") is not True or report.get("pilot_card_passed") is not True or report.get("cards_passed") != 1 or report.get("cards_total") != 291 or report.get("printings_total") != 306 or report.get("cards_credited") != 0 or report.get("coverage_complete") is not False or report.get("full_reward") is not False or report.get("harbor_reward") != 0.0 or report.get("viewer_scope") != "Judge" or report.get("card") != {"collector_number": 79, "definition_id": "RAV-CARRION-HOWLER", "name": "Carrion Howler"} or report.get("rules_authority", {}).get("comprehensive_rules") != "2026-08-07"):
        raise WorkerFailure("candidate_protocol_error: incomplete Carrion Howler result")
    traces = report.get("traces")
    if not isinstance(traces, list) or len(traces) != 1:
        raise WorkerFailure("candidate_protocol_error: one Carrion Howler PvP trace is required")
    trace = traces[0]
    if not isinstance(trace, dict) or trace.get("id") != "rav-carrion-howler-life-paid-pump" or trace.get("passed") is not True or trace.get("terminal") != "Winner(PlayerId(0))" or trace.get("winner_seat") != 0 or trace.get("decks") != ["rav_carrion_howler_life_pump", "rav_basic_land_opponent"] or trace.get("life_at_match_end") != [15, 0] or trace.get("life_at_activation") != 19 or trace.get("live_pump_power_toughness") != {"power": 4, "toughness": 1} or trace.get("post_cleanup_power_toughness") != {"power": 2, "toughness": 2} or trace.get("rejected_moves") != 0:
        raise WorkerFailure("candidate_protocol_error: paid cast, 4/1 pump, cleanup reset, and terminal game are required")
    payment = trace.get("cast_payment", {})
    if payment.get("total") != 4 or payment.get("black_symbol_paid") is not True or "Black" not in payment.get("colors", []):
        raise WorkerFailure("candidate_protocol_error: actual {3}{B} cast payment is missing")
    attack_turns = trace.get("attack_turns")
    if not isinstance(attack_turns, list) or len(attack_turns) != 5 or trace.get("first_attack_turn") not in attack_turns:
        raise WorkerFailure("candidate_protocol_error: repeated Carrion Howler combat is required")
    events = trace.get("events")
    if not isinstance(events, list) or not events or any(not isinstance(event.get("sequence"), int) or not isinstance(event.get("kind"), str) or not isinstance(event.get("canonical"), str) for event in events):
        raise WorkerFailure("candidate_protocol_error: projected event list is malformed")
    sequences = [event["sequence"] for event in events]
    if sequences != sorted(sequences):
        raise WorkerFailure("candidate_protocol_error: event sequence is not ordered")
    positions = trace.get("ordered_event_sequence")
    if not isinstance(positions, list) or len(positions) != 10 or any(not isinstance(item, int) for item in positions) or positions != sorted(positions):
        raise WorkerFailure("candidate_protocol_error: cast/activation/resolution/combat event order is missing")
    required_kinds = ["SpellManaPaid", "SpellCast", "SpellResolved", "CardMoved", "AbilityLifePaid", "AbilityActivated", "AbilityResolved", "AttackersDeclared", "DamageDealtToPlayer", "GameEnded"]
    if any(position >= len(events) or events[position]["sequence"] != position or events[position]["kind"] != kind for position, kind in zip(positions, required_kinds, strict=True)):
        raise WorkerFailure("candidate_protocol_error: cited event positions do not identify the card's ordered cast, ability, combat, and terminal receipts")
    kinds = [event["kind"] for event in events]
    expected = ["SpellManaPaid", "SpellCast", "SpellResolved", "CardMoved", "AbilityLifePaid", "AbilityActivated", "AbilityResolved", "AttackersDeclared", "DamageDealtToPlayer", "GameEnded"]
    cursor = iter(kinds)
    if not all(any(actual == required for actual in cursor) for required in expected):
        raise WorkerFailure("candidate_protocol_error: ordered life cost, stack resolution, attack damage, and game end receipts are required")
    canonicals = [event["canonical"] for event in events]
    howler = f"ObjectId({trace.get('howler_object')})"
    if not any("AbilityLifePaid" in event and howler in event and "amount: 1" in event for event in canonicals):
        raise WorkerFailure("candidate_protocol_error: one-life payment receipt for Carrion Howler is missing")
    if not any("AbilityActivated" in event and howler in event and "pay-life-pump-plus-two-minus-one" in event for event in canonicals):
        raise WorkerFailure("candidate_protocol_error: stack activation receipt is missing")
    if not any("DamageDealtToPlayer" in event and howler in event and "player: PlayerId(1)" in event and "amount: 4" in event for event in canonicals):
        raise WorkerFailure("candidate_protocol_error: live 4/1 Carrion Howler combat damage is missing")
    if sum("AbilityLifePaid" in event and howler in event and "amount: 1" in event for event in canonicals) != 5 or sum("AbilityActivated" in event and howler in event and "pay-life-pump-plus-two-minus-one" in event for event in canonicals) != 5 or sum("AbilityResolved" in event and howler in event and "pay-life-pump-plus-two-minus-one" in event for event in canonicals) != 5 or sum("DamageDealtToPlayer" in event and howler in event and "player: PlayerId(1)" in event and "amount: 4" in event for event in canonicals) != 5:
        raise WorkerFailure("candidate_protocol_error: all five life-paid pumps and 4-damage attacks are required")
    mutation = report.get("mutation_sensitivity", {})
    if mutation.get("wrong_temporary_pump_effect_mutation_detected") is not True or mutation.get("wrong_life_payment_mutation_detected") is not True or mutation.get("sealed_mutation_authority") is not False:
        raise WorkerFailure("candidate_protocol_error: targeted effect and payment mutants must fail")


def parse_report(stdout: bytes) -> dict:
    for line in reversed(stdout.decode(errors="replace").splitlines()):
        try:
            value = json.loads(line)
        except json.JSONDecodeError:
            continue
        if value.get("schema_version") == "cardbench.magic.card-pvp.check.v1":
            validate_report(value)
            return value
    raise WorkerFailure("candidate_protocol_error: no Carrion Howler PvP report")


def run_worker(workspace: Path, authority: Path, image: str, candidate_kind: str, timeout: int) -> tuple[int, bytes, bytes]:
    registry = Path.home() / ".cargo" / "registry"
    if not registry.is_dir():
        raise FileNotFoundError(f"offline Cargo registry cache is missing: {registry}")
    if not shutil.which("docker"):
        raise FileNotFoundError("Docker is required for isolated Magic candidate scoring")
    name = f"cardbench-magic-carrion-howler-{uuid.uuid4().hex[:12]}"
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
        "cp -R /submission/. /work/ && cp /trusted-authority.rs /work/varieties/magic/session/src/bin/rav_carrion_howler_pvp.rs && cd /work && cargo run --offline --locked -j 2 --manifest-path varieties/magic/Cargo.toml -p cardbench-magic-session --bin rav-carrion-howler-pvp --" + candidate_flag,
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
        with tempfile.TemporaryDirectory(prefix="carrion-howler-candidate-") as temporary:
            candidate = snapshot(args.workspace.resolve(), manifest, Path(temporary) / "workspace")
            authority = (REPO / AUTHORITY).resolve()
            code, stdout, stderr = run_worker(candidate, authority, image, args.candidate_kind, args.timeout)
        (args.output_root / "worker.stdout").write_bytes(stdout)
        (args.output_root / "worker.stderr").write_bytes(stderr)
        if code != 0:
            result = {"schema_version":"cardbench.magic.card-pvp.check.v1","task_id":"cardbench/magic/card","candidate":{"kind":args.candidate_kind,"source_sha256":sha256((args.workspace.resolve()/TARGET).read_bytes())},"passed":False,"pilot_card_passed":False,"cards_passed":0,"cards_total":291,"printings_total":306,"cards_credited":0,"coverage_complete":False,"full_reward":False,"harbor_reward":0.0,"pilot_reward":0.0,"sealed_authority":False,"error":"candidate compile or Carrion Howler trace gate failed","diagnostics":stderr.decode(errors="replace")[-12000:]}
        else:
            result = parse_report(stdout)
            result["candidate"] = {"kind":args.candidate_kind,"card_definition":"RAV-CARRION-HOWLER","source_sha256":sha256((args.workspace.resolve()/TARGET).read_bytes()),"materialized_workspace_sha256":manifest["workspace_sha256"]}
            result["candidate_kind"] = args.candidate_kind
            result["starting_workspace_sha256"] = manifest["workspace_sha256"]
            result["sealed_authority"] = False
            result["worker_image"] = image
            result["sandbox"] = {"network":"none","rootfs":"read-only","pids_limit":128}
            result["harbor_reward"] = 0.0
            result["full_reward"] = False
        (args.output_root / "result.json").write_text(json.dumps(result, indent=2)+"\n", encoding="utf-8")
        (args.output_root / "reward.txt").write_text(("1.00000000\n" if result.get("passed") else "0.00000000\n"), encoding="utf-8")
        return 0 if result.get("passed") else 1
    except Exception as error:  # the worker report is the bounded failure protocol
        failure = {"schema_version":"cardbench.magic.card-pvp.check.v1","task_id":"cardbench/magic/card","passed":False,"pilot_card_passed":False,"cards_passed":0,"cards_total":291,"printings_total":306,"cards_credited":0,"coverage_complete":False,"full_reward":False,"harbor_reward":0.0,"pilot_reward":0.0,"sealed_authority":False,"error":f"{type(error).__name__}: {error}"}
        (args.output_root / "result.json").write_text(json.dumps(failure, indent=2)+"\n", encoding="utf-8")
        (args.output_root / "reward.txt").write_text("0.00000000\n", encoding="utf-8")
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
