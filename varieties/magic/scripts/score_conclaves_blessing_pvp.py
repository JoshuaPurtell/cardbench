#!/usr/bin/env python3
"""Run Conclave's Blessing in a bounded offline candidate worker and score its PvP trace."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import selectors
import shutil
import subprocess
import sys
import time
import uuid
from pathlib import Path

REPO = Path(__file__).resolve().parents[3]
MATERIALIZER = Path(__file__).with_name("materialize_conclaves_blessing_task.py")
TARGET = Path("varieties/magic/sets/ravnica_city_of_guilds/src/cards/conclaves_blessing.rs")
AUTHORITY_SOURCE = Path("varieties/magic/session/src/bin/rav_conclaves_blessing_pvp.rs")
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
        source = workspace / relative
        if source.is_symlink() or not source.is_file():
            raise WorkerFailure(f"candidate_snapshot_source_missing: {relative}")
        destination = snapshot / relative
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
        "cp /trusted-authority.rs /work/varieties/magic/session/src/bin/rav_conclaves_blessing_pvp.rs && "
        "cd /work && cargo run --offline --locked -j 2 --manifest-path varieties/magic/Cargo.toml "
        "-p cardbench-magic-session --bin rav-conclaves-blessing-pvp --" + candidate_flag,
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
    raise WorkerFailure("candidate_protocol_error: no valid Conclave's Blessing PvP result")


def validate_report(report: dict) -> None:
    if (
        report.get("task_id") != "cardbench/magic/card" or report.get("passed") is not True
        or report.get("pilot_card_passed") is not True or report.get("cards_passed") != 1
        or report.get("cards_credited") != 0 or report.get("cards_total") != 291
        or report.get("printings_total") != 306 or report.get("coverage_complete") is not False
        or report.get("full_reward") is not False or report.get("harbor_reward") != 0.0
        or report.get("pilot_reward") != 0.0 or report.get("viewer_scope") != "Judge"
        or report.get("card") != {"collector_number": 11, "definition_id": "RAV-CONCLAVES-BLESSING", "name": "Conclave's Blessing"}
    ):
        raise WorkerFailure("candidate_protocol_error: incomplete Conclave's Blessing diagnostic result")
    candidate = report.get("candidate")
    if not isinstance(candidate, dict) or candidate.get("card_definition") != "RAV-CONCLAVES-BLESSING":
        raise WorkerFailure("candidate_protocol_error: wrong Conclave's Blessing candidate identity")
    traces = report.get("traces")
    if not isinstance(traces, list) or len(traces) != 1:
        raise WorkerFailure("candidate_protocol_error: exactly one full Conclave's Blessing trace is required")
    validate_trace(traces[0])
    mutation = report.get("mutation_sensitivity", {})
    required = (
        "wrong_power_modifier_detected_in_game", "wrong_controller_count_detected_in_game",
        "aura_attachment_removed_detected_in_game", "wrong_mana_cost_detected_in_game",
        "convoke_removed_detected_in_game",
    )
    if not isinstance(mutation, dict) or any(mutation.get(key) is not True for key in required):
        raise WorkerFailure("candidate_protocol_error: Conclave's Blessing gameplay mutation controls are incomplete")


def validate_trace(trace: object) -> None:
    if not isinstance(trace, dict) or trace.get("trace_id") != "rav-conclaves-blessing-convoke-aura-controller-creature-count-full-game" or trace.get("terminal") != "Winner(PlayerId(0))" or trace.get("winner") != 0 or trace.get("rejected_policy_moves") != 0:
        raise WorkerFailure("candidate_protocol_error: Conclave's Blessing duel did not finish a clean P0 win")
    mechanics = trace.get("mechanics")
    if not isinstance(mechanics, dict):
        raise WorkerFailure("candidate_protocol_error: Conclave's Blessing mechanics are missing")
    convokers = mechanics.get("convoke_creature_objects")
    if (
        mechanics.get("mana_cost") != "{3}{W}" or mechanics.get("paid_mana_colors") != ["White"]
        or mechanics.get("convoke_generic_symbols_paid") != 3
        or not isinstance(convokers, list) or len(convokers) != 3
        or any(not isinstance(obj, int) for obj in convokers) or len(set(convokers)) != 3
        or mechanics.get("convoke_creatures_tapped_at_cast") != [True, True, True]
        or mechanics.get("aura_attached_to_opponent_creature") is not True
        or mechanics.get("aura_controller_other_creature_count") != 3
        or mechanics.get("target_controller_seat") != 1
        or mechanics.get("live_power_toughness") != [1, 7]
        or mechanics.get("modifier_per_other_creature") != [0, 2]
        or mechanics.get("p0_creatures_attacked") is not True
        or mechanics.get("controller_combat_damage", 0) < 20
        or mechanics.get("deterministic_replay") is not True
        or mechanics.get("terminal_combat_win") is not True
    ):
        raise WorkerFailure("candidate_protocol_error: Conclave's Blessing Convoke, controller-relative toughness, combat, replay, or terminal result differs")
    aura, target = mechanics.get("aura_object"), mechanics.get("target_creature_object")
    if not isinstance(aura, int) or not isinstance(target, int) or aura == target:
        raise WorkerFailure("candidate_protocol_error: Conclave's Blessing Aura/target objects are malformed")
    events = trace.get("events")
    if not isinstance(events, list) or not all(isinstance(event, dict) for event in events):
        raise WorkerFailure("candidate_protocol_error: Conclave's Blessing event trace is malformed")
    sequences = [event.get("sequence") for event in events]
    if any(not isinstance(sequence, int) for sequence in sequences) or sequences != sorted(sequences) or len(set(sequences)) != len(sequences):
        raise WorkerFailure("candidate_protocol_error: Conclave's Blessing event order is malformed")
    by_kind: dict[str, list[dict]] = {}
    for event in events:
        by_kind.setdefault(event.get("kind"), []).append(event)
    required = {"ConvokeUsed", "SpellManaPaid", "SpellCast", "SpellResolved", "CardMoved", "AuraAttached", "AttackersDeclared", "DamageDealtToPlayer", "GameEnded"}
    if not required.issubset(by_kind):
        raise WorkerFailure("candidate_protocol_error: required Conclave's Blessing gameplay receipts are missing")
    convoke = [e for e in by_kind["ConvokeUsed"] if e.get("facts", {}).get("seats") == [0] and e.get("facts", {}).get("convoke_contribution") == "generic"]
    if len(convoke) != 3 or {e.get("facts", {}).get("objects", [None])[0] for e in convoke} != set(convokers):
        raise WorkerFailure("candidate_protocol_error: exact generic Convoke receipts differ from tapped creature objects")
    payment = next((e for e in by_kind["SpellManaPaid"] if e.get("facts", {}).get("objects") == [aura] and e.get("facts", {}).get("seats") == [0]), None)
    cast = next((e for e in by_kind["SpellCast"] if e.get("facts", {}).get("objects") == [aura]), None)
    resolved = next((e for e in by_kind["SpellResolved"] if e.get("facts", {}).get("objects") == [aura]), None)
    enters = next((e for e in by_kind["CardMoved"] if e.get("facts", {}).get("objects") == [aura] and e.get("facts", {}).get("zone") == "Battlefield"), None)
    attached = next((e for e in by_kind["AuraAttached"] if f"aura: ObjectId({aura})" in e.get("canonical", "") and f"target: ObjectId({target})" in e.get("canonical", "")), None)
    ended = next((e for e in by_kind["GameEnded"] if e.get("facts", {}).get("seats") == [0]), None)
    attacks = [e for e in by_kind["AttackersDeclared"] if e.get("facts", {}).get("seats") == [0]]
    damage = [e for e in by_kind["DamageDealtToPlayer"] if e.get("facts", {}).get("seats") == [1] and e.get("facts", {}).get("amount", 0) > 0]
    if any(e is None for e in (payment, cast, resolved, enters, attached, ended)) or not attacks or not damage:
        raise WorkerFailure("candidate_protocol_error: exact Convoke, payment, attachment, combat, and game-end receipts are missing")
    if payment.get("facts", {}).get("mana_colors") != ["White"]:
        raise WorkerFailure("candidate_protocol_error: reported white payment differs from the typed payment receipt")
    aura_match = re.search(r"card: ObjectId\((\d+)\)", enters.get("canonical", ""))
    if aura_match is None or int(aura_match.group(1)) != aura:
        raise WorkerFailure("candidate_protocol_error: Conclave's Blessing battlefield entry object differs")
    if not (max(e["sequence"] for e in convoke) < payment["sequence"] < cast["sequence"] < resolved["sequence"] < enters["sequence"] < attached["sequence"] < min(e["sequence"] for e in attacks) < min(e["sequence"] for e in damage) < ended["sequence"]):
        raise WorkerFailure("candidate_protocol_error: Convoke/payment/attachment/combat/game-end event order differs")

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
    if authority.is_symlink() or not authority.is_file():
        raise FileNotFoundError(f"trusted Conclave's Blessing authority is missing: {authority}")
    if not shutil.which("docker"):
        raise FileNotFoundError("Docker is required for Magic candidate isolation")
    verify_workspace(workspace, manifest)
    snapshot = make_scoring_snapshot(workspace, manifest_data["files"], output_root / "candidate-workspace")
    digest = sha256((snapshot / TARGET).read_bytes())
    name = f"cardbench-magic-conclaves-blessing-{uuid.uuid4().hex[:16]}"
    code, stdout, stderr = run_bounded(sandbox_command(snapshot, registry.resolve(), authority, name, image, candidate_kind), timeout=900, name=name)
    (output_root / "worker.stdout").write_bytes(stdout)
    (output_root / "worker.stderr").write_bytes(stderr)
    if code != 0:
        result = {"schema_version":"cardbench.magic.card-pvp.check.v1","task_id":"cardbench/magic/card","candidate":{"kind":candidate_kind,"source_sha256":digest},"passed":False,"pilot_card_passed":False,"cards_passed":0,"cards_total":291,"printings_total":306,"cards_credited":0,"coverage_complete":False,"full_reward":False,"harbor_reward":0.0,"pilot_reward":0.0,"sealed_authority":False,"worker_image":image,"sandbox":{"network":"none","rootfs":"read-only","pids_limit":128},"error":"candidate compile or Conclave's Blessing reference trace failed","diagnostics":stderr.decode(errors="replace")[-12000:]}
        (output_root/"result.json").write_text(json.dumps(result,indent=2)+"\n",encoding="utf-8"); print(json.dumps(result)); return 1
    report=parse_report(stdout);report["candidate"].update({"kind":candidate_kind,"source_sha256":digest});report.update({"sealed_authority":False,"worker_image":image,"sandbox":{"network":"none","rootfs":"read-only","pids_limit":128}})
    (output_root/"result.json").write_text(json.dumps(report,indent=2)+"\n",encoding="utf-8");print(json.dumps(report));return 0


def main() -> int:
    parser=argparse.ArgumentParser();parser.add_argument("--workspace",required=True,type=Path);parser.add_argument("--manifest",required=True,type=Path);parser.add_argument("--output-root",required=True,type=Path);parser.add_argument("--candidate-kind",choices=("agent","reference"),default="agent");args=parser.parse_args()
    return score(args.workspace.resolve(),args.manifest.resolve(),args.output_root.resolve(),args.candidate_kind)


if __name__ == "__main__":
    raise SystemExit(main())
