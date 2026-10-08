#!/usr/bin/env python3
"""Run the Viashino Slasher pilot in a bounded, networkless Rust worker."""

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
MATERIALIZER = Path(__file__).with_name("materialize_viashino_slasher_task.py")
TARGET = Path("varieties/magic/sets/ravnica_city_of_guilds/src/cards/viashino_slasher.rs")
AUTHORITY = Path("varieties/magic/session/src/bin/rav_viashino_slasher_pvp.rs")
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
        or report.get("viewer_scope") != "Judge"
        or report.get("card") != {"collector_number": 149, "definition_id": "RAV-VIASHINO-SLASHER", "name": "Viashino Slasher"}
        or report.get("rules_authority", {}).get("comprehensive_rules") != "2026-08-07"
    ):
        raise WorkerFailure("candidate_protocol_error: incomplete Viashino Slasher result")
    traces = report.get("traces")
    if not isinstance(traces, list) or len(traces) != 1:
        raise WorkerFailure("candidate_protocol_error: one Viashino Slasher PvP trace is required")
    trace = traces[0]
    if (
        not isinstance(trace, dict)
        or trace.get("id") != "rav-viashino-slasher-red-discard-pump"
        or trace.get("passed") is not True
        or trace.get("terminal") != "Winner(PlayerId(0))"
        or trace.get("winner_seat") != 0
        or trace.get("decks") != ["rav_viashino_slasher_discard_pump", "rav_basic_plains_opponent"]
        or trace.get("life_at_match_end") != [20, 0]
        or trace.get("rejected_moves") != 0
        or trace.get("live_power_toughness") != {"power": 2, "toughness": 1}
        or trace.get("post_cleanup_power_toughness") != {"power": 1, "toughness": 2}
        or trace.get("same_turn_pump_attack") is not True
    ):
        raise WorkerFailure("candidate_protocol_error: red discard pump, cleanup reset, and terminal PvP are required")
    payment = trace.get("cast_payment", {})
    if not isinstance(payment.get("colors"), list) or len(payment["colors"]) != 2 or payment.get("red_paid") is not True or "Red" not in payment["colors"]:
        raise WorkerFailure("candidate_protocol_error: actual {1}{R} creature payment is missing")
    attack_turns = trace.get("attack_turns")
    powered_attacks = trace.get("powered_attack_count")
    if not isinstance(attack_turns, list) or len(attack_turns) < 5 or powered_attacks != len(attack_turns):
        raise WorkerFailure("candidate_protocol_error: every repeated Slasher attack must deal the pumped 2 damage")
    if trace.get("ability_red_payments") != trace.get("discard_cost_receipts") or trace.get("ability_red_payments") != trace.get("ability_resolutions") or trace.get("ability_red_payments", 0) < 5:
        raise WorkerFailure("candidate_protocol_error: each required pump needs a red payment, discard, and resolution receipt")
    events = trace.get("events")
    if not isinstance(events, list) or not events or any(not isinstance(event.get("sequence"), int) or not isinstance(event.get("kind"), str) or not isinstance(event.get("canonical"), str) for event in events):
        raise WorkerFailure("candidate_protocol_error: projected event list is malformed")
    sequences = [event["sequence"] for event in events]
    if sequences != list(range(len(events))):
        raise WorkerFailure("candidate_protocol_error: projected event sequence is not contiguous and ordered")
    positions = trace.get("ordered_event_sequence")
    if not isinstance(positions, list) or len(positions) != 12 or any(not isinstance(item, int) for item in positions) or positions != sorted(positions):
        raise WorkerFailure("candidate_protocol_error: cast/cost/stack/combat event order is missing")
    required_kinds = ["SpellManaPaid", "SpellCast", "SpellResolved", "CardMoved", "AbilityManaPaid", "DiscardedAsAbilityCost", "CardMoved", "AbilityActivated", "AbilityResolved", "AttackersDeclared", "DamageDealtToPlayer", "GameEnded"]
    if any(position >= len(events) or events[position]["sequence"] != position or events[position]["kind"] != kind for position, kind in zip(positions, required_kinds, strict=True)):
        raise WorkerFailure("candidate_protocol_error: cited positions do not identify cast, red/discard costs, stack resolution, combat, and terminal receipts")
    kinds = [event["kind"] for event in events]
    cursor = iter(kinds)
    if not all(any(actual == required for actual in cursor) for required in required_kinds):
        raise WorkerFailure("candidate_protocol_error: ordered Slasher gameplay receipts are incomplete")
    canonicals = [event["canonical"] for event in events]
    source = f"ObjectId({trace.get('slasher_object')})"
    discard = f"ObjectId({trace.get('first_discard_object')})"
    if not any("SpellManaPaid" in event and source in event and "Red" in event for event in canonicals):
        raise WorkerFailure("candidate_protocol_error: red spell-cast payment is missing")
    if not any("AbilityManaPaid" in event and source in event and "pump-plus-one-minus-one" in event and "colored: [Red]" in event for event in canonicals):
        raise WorkerFailure("candidate_protocol_error: actual red activated-ability payment is missing")
    if not any("DiscardedAsAbilityCost" in event and source in event and discard in event for event in canonicals):
        raise WorkerFailure("candidate_protocol_error: actual hand discard cost receipt is missing")
    if not any("CardMoved" in event and discard in event and "to: Graveyard" in event for event in canonicals):
        raise WorkerFailure("candidate_protocol_error: discarded cost card did not move to its owner's graveyard")
    if not any("AbilityActivated" in event and source in event and "pump-plus-one-minus-one" in event for event in canonicals):
        raise WorkerFailure("candidate_protocol_error: activated ability stack receipt is missing")
    if not any("AbilityResolved" in event and source in event and "pump-plus-one-minus-one" in event for event in canonicals):
        raise WorkerFailure("candidate_protocol_error: ability resolution receipt is missing")
    if not any("ContinuousEffectCreated" in event and source in event and "target: ObjectId" in event and "PowerToughness" in event for event in canonicals):
        raise WorkerFailure("candidate_protocol_error: source +1/-1 continuous effect was not created")
    if not any("ContinuousEffectExpired" in event and source in event and "PowerToughness" in event for event in canonicals):
        raise WorkerFailure("candidate_protocol_error: until-end-of-turn effect expiration receipt is missing")
    if sum("DamageDealtToPlayer" in event and source in event and "player: PlayerId(1)" in event and "amount: 2" in event for event in canonicals) != powered_attacks:
        raise WorkerFailure("candidate_protocol_error: repeated 2-damage Slasher attacks are missing")
    mutation = report.get("mutation_sensitivity", {})
    if mutation.get("wrong_pump_power_mutation_detected") is not True or mutation.get("sealed_mutation_authority") is not False:
        raise WorkerFailure("candidate_protocol_error: targeted +0/-1 gameplay mutant must fail")

def parse_report(stdout: bytes) -> dict:
    for line in reversed(stdout.decode(errors="replace").splitlines()):
        try:
            value = json.loads(line)
        except json.JSONDecodeError:
            continue
        if value.get("schema_version") == "cardbench.magic.card-pvp.check.v1":
            validate_report(value)
            return value
    raise WorkerFailure("candidate_protocol_error: no Viashino Slasher PvP report")


def run_worker(workspace: Path, authority: Path, image: str, candidate_kind: str, timeout: int) -> tuple[int, bytes, bytes]:
    registry = Path.home() / ".cargo" / "registry"
    if not registry.is_dir():
        raise FileNotFoundError(f"offline Cargo registry cache is missing: {registry}")
    if not shutil.which("docker"):
        raise FileNotFoundError("Docker is required for isolated Magic candidate scoring")
    name = f"cardbench-magic-viashino-slasher-{uuid.uuid4().hex[:12]}"
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
        "cp -R /submission/. /work/ && cp /trusted-authority.rs /work/varieties/magic/session/src/bin/rav_viashino_slasher_pvp.rs && cd /work && cargo run --offline --locked -j 2 --manifest-path varieties/magic/Cargo.toml -p cardbench-magic-session --bin rav-viashino-slasher-pvp --" + candidate_flag,
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
        with tempfile.TemporaryDirectory(prefix="viashino-slasher-candidate-") as temporary:
            candidate = snapshot(args.workspace.resolve(), manifest, Path(temporary) / "workspace")
            authority = (REPO / AUTHORITY).resolve()
            code, stdout, stderr = run_worker(candidate, authority, image, args.candidate_kind, args.timeout)
        (args.output_root / "worker.stdout").write_bytes(stdout)
        (args.output_root / "worker.stderr").write_bytes(stderr)
        if code != 0:
            result = {"schema_version":"cardbench.magic.card-pvp.check.v1","task_id":"cardbench/magic/card","candidate":{"kind":args.candidate_kind,"source_sha256":sha256((args.workspace.resolve()/TARGET).read_bytes())},"passed":False,"pilot_card_passed":False,"cards_passed":0,"cards_total":291,"printings_total":306,"cards_credited":0,"coverage_complete":False,"full_reward":False,"harbor_reward":0.0,"pilot_reward":0.0,"sealed_authority":False,"error":"candidate compile or Viashino Slasher trace gate failed","diagnostics":stderr.decode(errors="replace")[-12000:]}
        else:
            result = parse_report(stdout)
            result["candidate"] = {"kind":args.candidate_kind,"card_definition":"RAV-VIASHINO-SLASHER","source_sha256":sha256((args.workspace.resolve()/TARGET).read_bytes()),"materialized_workspace_sha256":manifest["workspace_sha256"]}
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
