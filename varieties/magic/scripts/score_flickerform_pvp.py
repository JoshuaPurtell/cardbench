#!/usr/bin/env python3
"""Run the Flickerform reference authority in a bounded offline worker."""

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
MATERIALIZER = Path(__file__).with_name("materialize_flickerform_task.py")
TARGET = Path("varieties/magic/sets/ravnica_city_of_guilds/src/cards/flickerform.rs")
AUTHORITY_SOURCE = Path("varieties/magic/session/src/bin/rav_flickerform_pvp.rs")
DEFAULT_WORKER_IMAGE = "rust@sha256:d7482085ff5b415f84dba5647ae71606650bdef00db7aeb69f4b3d170c3e4082"
MAX_OUTPUT_BYTES = 2 * 1024 * 1024


class WorkerFailure(RuntimeError):
    pass


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def verify_workspace(workspace: Path, manifest: Path) -> None:
    subprocess.run(
        [sys.executable, str(MATERIALIZER), "--workspace", str(workspace), "--manifest", str(manifest), "--verify"],
        check=True,
        cwd=REPO,
    )


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


def sandbox_command(
    workspace: Path, registry: Path, authority: Path, name: str, image: str, candidate_kind: str
) -> list[str]:
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
        "cp /trusted-authority.rs /work/varieties/magic/session/src/bin/rav_flickerform_pvp.rs && "
        "cd /work && cargo run --offline --locked -j 2 --manifest-path varieties/magic/Cargo.toml "
        "-p cardbench-magic-session --bin rav-flickerform-pvp --" + candidate_flag,
    ]


def validate_report(report: dict) -> None:
    if (
        report.get("schema_version") != "cardbench.magic.card-pvp.check.v1"
        or report.get("task_id") != "cardbench/magic/card"
        or report.get("expansion") != "RAV"
        or report.get("passed") is not True
        or report.get("pilot_card_passed") is not True
        or report.get("cards_passed") != 1
        or report.get("cards_credited") != 0
        or report.get("cards_total") != 291
        or report.get("printings_total") != 306
        or report.get("coverage_complete") is not False
        or report.get("full_reward") is not False
        or report.get("harbor_reward") != 0.0
        or report.get("pilot_reward") != 0.0
        or report.get("viewer_scope") != "Judge"
        or report.get("card") != {"collector_number": 18, "definition_id": "RAV-FLICKERFORM", "name": "Flickerform"}
        or report.get("deterministic_replay") is not True
    ):
        raise WorkerFailure("candidate_protocol_error: incomplete Flickerform single-card report")
    traces = report.get("traces")
    if not isinstance(traces, list) or len(traces) != 1:
        raise WorkerFailure("candidate_protocol_error: exactly one full-game Flickerform trace is required")
    trace = traces[0]
    source = trace.get("flickerform_object") if isinstance(trace, dict) else None
    creature = trace.get("creature_object") if isinstance(trace, dict) else None
    aura = trace.get("attached_aura_object") if isinstance(trace, dict) else None
    linked = trace.get("linked_exile_objects") if isinstance(trace, dict) else None
    returned = trace.get("returned_object_incarnations") if isinstance(trace, dict) else None
    returned_ids = trace.get("returned_objects") if isinstance(trace, dict) else None
    combat = trace.get("combat") if isinstance(trace, dict) else None
    linked_turns = trace.get("linked_return_turns") if isinstance(trace, dict) else None
    if (
        not isinstance(trace, dict)
        or trace.get("id") != "rav-flickerform-linked-exile-return-and-combat-full-game"
        or trace.get("passed") is not True
        or trace.get("terminal") != "Winner(PlayerId(0))"
        or trace.get("winner_seat") != 0
        or not isinstance(trace.get("turns"), int) or trace["turns"] <= 0
        or trace.get("rejected_moves") != 0
        or not all(isinstance(value, int) for value in (source, creature, aura))
        or len({source, creature, aura}) != 3
        or not isinstance(linked, list) or not isinstance(returned, list) or not isinstance(returned_ids, list)
        or {item.get("object") for item in linked if isinstance(item, dict)} != {source, creature, aura}
        or {item.get("object") for item in returned if isinstance(item, dict)} != {source, creature, aura}
        or set(returned_ids) != {source, creature, aura}
        or any(not isinstance(item, dict) or not isinstance(item.get("exile_incarnation"), int) for item in linked)
        or any(not isinstance(item, dict) or not isinstance(item.get("incarnation"), int) for item in returned)
        or any(next(item["incarnation"] for item in returned if item["object"] == exile["object"]) <= exile["exile_incarnation"] for exile in linked)
        or trace.get("creature_power_after_return") != 6
        or not isinstance(linked_turns, dict)
        or not isinstance(linked_turns.get("activated"), int)
        or linked_turns.get("due") != linked_turns.get("activated")
        or linked_turns.get("returned") != linked_turns.get("activated")
        or not isinstance(combat, dict)
        or not isinstance(combat.get("attack_turns"), list) or not combat["attack_turns"]
        or combat.get("damage_to_opponent", 0) < 20
        or not isinstance(trace.get("events_digest"), str)
    ):
        raise WorkerFailure("candidate_protocol_error: exact linked group, new incarnations, reattachment, combat, or terminal state is invalid")
    events = trace.get("events")
    if not isinstance(events, list):
        raise WorkerFailure("candidate_protocol_error: Flickerform event receipts are missing")
    sequences = [event.get("sequence") for event in events]
    if any(not isinstance(value, int) for value in sequences) or sequences != sorted(sequences):
        raise WorkerFailure("candidate_protocol_error: Flickerform event sequence is malformed")
    kinds = [event.get("kind") for event in events]
    required_counts = {
        "SpellManaPaid": 1, "SpellCast": 1, "SpellResolved": 1,
        "AttachmentEstablishedWithoutContinuousEffect": 2, "AuraAttached": 2,
        "ContinuousEffectCreated": 1,
        "AbilityManaPaid": 1, "AbilityActivated": 1,
        "CardMoved": 6, "DelayedActionScheduled": 1, "AbilityResolved": 1,
        "DelayedActionConsumed": 1, "AttackersDeclared": 1,
        "DamageDealtToPlayer": 1, "GameEnded": 1,
    }
    if any(kinds.count(kind) != count for kind, count in required_counts.items()):
        raise WorkerFailure("candidate_protocol_error: cast, Aura, linked exile/return, combat, or terminal receipt count differs")
    canon = [event.get("canonical", "") for event in events]
    def at(kind: str, predicate) -> int:
        for index, event in enumerate(events):
            if event.get("kind") == kind and predicate(event.get("canonical", "")):
                return index
        raise WorkerFailure(f"candidate_protocol_error: {kind} exact identity receipt missing")
    cast = at("SpellCast", lambda text: f"card: ObjectId({source})" in text and "player: PlayerId(0)" in text)
    resolved = at("SpellResolved", lambda text: f"card: ObjectId({source})" in text)
    flicker_attach = at("AttachmentEstablishedWithoutContinuousEffect", lambda text: f"attachment: ObjectId({source})" in text and f"target: ObjectId({creature})" in text)
    cloak_attach = at("AuraAttached", lambda text: f"aura: ObjectId({aura})" in text and f"target: ObjectId({creature})" in text)
    paid = at("AbilityManaPaid", lambda text: f"source: ObjectId({source})" in text and "linked-exile-attached-creature-and-auras" in text and "generic: 2" in text and text.count("White") == 2)
    activated = at("AbilityActivated", lambda text: f"source: ObjectId({source})" in text and "linked-exile-attached-creature-and-auras" in text)
    scheduled = at("DelayedActionScheduled", lambda text: all(f"ObjectId({object_id})" in text for object_id in (source, creature, aura)) and "EndStep" in text and f"due_turn: {linked_turns['due']}" in text and "PrimaryCreature" in text and text.count("AttachedAura") == 2)
    ability_resolved = at("AbilityResolved", lambda text: f"source: ObjectId({source})" in text and "linked-exile-attached-creature-and-auras" in text)
    consumed = at("DelayedActionConsumed", lambda text: all(f"ObjectId({object_id})" in text for object_id in (source, creature, aura)))
    attack = at("AttackersDeclared", lambda text: f"ObjectId({creature})" in text and "PlayerId(0)" in text)
    damage = at("DamageDealtToPlayer", lambda text: f"ObjectId({creature})" in text and "PlayerId(1)" in text and "amount: 6" in text)
    ended = at("GameEnded", lambda text: "Some(PlayerId(0))" in text)
    returned_flicker_attach = next((index for index, event in enumerate(events) if index > flicker_attach and event.get("kind") == "AttachmentEstablishedWithoutContinuousEffect" and f"attachment: ObjectId({source})" in event.get("canonical", "") and f"target: ObjectId({creature})" in event.get("canonical", "")), None)
    restored_modifier = at("ContinuousEffectCreated", lambda text: f"source: ObjectId({aura})" in text and f"target: ObjectId({creature})" in text and "PowerToughness" in text)
    returned_cloak_attach = next((index for index, event in enumerate(events) if index > cloak_attach and event.get("kind") == "AuraAttached" and f"aura: ObjectId({aura})" in event.get("canonical", "") and f"target: ObjectId({creature})" in event.get("canonical", "")), None)
    if returned_flicker_attach is None or returned_cloak_attach is None or not (cast < resolved < flicker_attach < cloak_attach < paid < activated < scheduled < ability_resolved < returned_flicker_attach < restored_modifier < returned_cloak_attach < consumed < attack < damage < ended):
        raise WorkerFailure("candidate_protocol_error: Flickerform cast, attachment, activation, delay, return, combat, and terminal order differs")
    if events[scheduled].get("turn") != linked_turns["due"] or events[consumed].get("turn") != linked_turns["returned"]:
        raise WorkerFailure("candidate_protocol_error: linked group did not return at its scheduled next end step")
    exile_receipts = [event for event in events if event.get("kind") == "CardMoved" and "to: Exile" in event.get("canonical", "")]
    return_receipts = [event for event in events if event.get("kind") == "CardMoved" and "to: Battlefield" in event.get("canonical", "") and any(f"card: ObjectId({object_id})" in event.get("canonical", "") for object_id in (source, creature, aura))]
    if len(exile_receipts) != 3 or len(return_receipts) != 3:
        raise WorkerFailure("candidate_protocol_error: all three exact linked cards must move to Exile and back")
    mutation = report.get("mutation_sensitivity", {})
    if not isinstance(mutation, dict) or any(mutation.get(key) is not True for key in (
        "linked_blink_effect_removed_mutation_detected_in_game",
        "activation_cost_changed_mutation_detected_in_game",
        "creature_aura_attachment_removed_mutation_detected_in_game",
    )):
        raise WorkerFailure("candidate_protocol_error: all Flickerform semantic mutations must be detected in game")


def parse_report(stdout: bytes) -> dict:
    for line in reversed(stdout.decode(errors="replace").splitlines()):
        try:
            value = json.loads(line)
        except json.JSONDecodeError:
            continue
        if value.get("schema_version") == "cardbench.magic.card-pvp.check.v1":
            validate_report(value)
            return value
    raise WorkerFailure("candidate_protocol_error: no valid Flickerform PvP result")


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
        raise FileNotFoundError(f"trusted Flickerform authority source is missing: {authority}")
    if not shutil.which("docker"):
        raise FileNotFoundError("Docker is required for Magic candidate isolation")
    verify_workspace(workspace, manifest)
    scoring_workspace = make_scoring_snapshot(workspace, manifest_data["files"], output_root / "candidate-workspace")
    candidate_hash = sha256((scoring_workspace / TARGET).read_bytes())
    name = f"cardbench-magic-flickerform-{uuid.uuid4().hex[:16]}"
    command = sandbox_command(scoring_workspace, cargo_registry.resolve(), authority, name, worker_image, candidate_kind)
    code, stdout, stderr = run_bounded(command, timeout=900, name=name)
    (output_root / "worker.stdout").write_bytes(stdout)
    (output_root / "worker.stderr").write_bytes(stderr)
    if code != 0:
        result = {
            "schema_version": "cardbench.magic.card-pvp.check.v1", "task_id": "cardbench/magic/card",
            "candidate": {"kind": candidate_kind, "source_sha256": candidate_hash}, "passed": False,
            "pilot_card_passed": False, "cards_passed": 0, "cards_total": 291, "printings_total": 306,
            "cards_credited": 0, "coverage_complete": False, "full_reward": False,
            "harbor_reward": 0.0, "pilot_reward": 0.0, "sealed_authority": False,
            "worker_image": worker_image, "sandbox": {"network": "none", "rootfs": "read-only", "pids_limit": 128},
            "error": "candidate compile or Flickerform reference trace failed",
            "diagnostics": stderr.decode(errors="replace")[-12000:],
        }
    else:
        result = parse_report(stdout)
        result["candidate"] = {
            "kind": candidate_kind, "card_definition": "RAV-FLICKERFORM", "source_sha256": candidate_hash,
            "materialized_workspace_sha256": manifest_data["workspace_sha256"],
        }
        result["candidate_kind"] = candidate_kind
        result["starting_workspace_sha256"] = manifest_data["workspace_sha256"]
        result["sealed_authority"] = False
        result["worker_image"] = worker_image
        result["sandbox"] = {"network": "none", "rootfs": "read-only", "pids_limit": 128}
        result["harbor_reward"] = 0.0
        result["full_reward"] = False
    (output_root / "result.json").write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
    (output_root / "reward.txt").write_text(f"{result['harbor_reward']:.8f}\n", encoding="utf-8")
    print(f"candidate_passed={result.get('pilot_card_passed', False)} cards={result.get('cards_passed', 0)}/291 coverage_complete=false")
    return 0 if result.get("passed") is True and result.get("pilot_card_passed") is True else 1


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--workspace", required=True, type=Path)
    parser.add_argument("--manifest", required=True, type=Path)
    parser.add_argument("--output-root", required=True, type=Path)
    parser.add_argument("--candidate-kind", choices=["agent", "reference"], default="agent")
    args = parser.parse_args()
    try:
        return score(args.workspace.resolve(), args.manifest.resolve(), args.output_root.resolve(), args.candidate_kind)
    except Exception as error:
        args.output_root.mkdir(parents=True, exist_ok=True)
        result = {
            "schema_version": "cardbench.magic.card-pvp.check.v1", "task_id": "cardbench/magic/card",
            "passed": False, "pilot_card_passed": False, "cards_passed": 0, "cards_total": 291,
            "printings_total": 306, "cards_credited": 0, "coverage_complete": False,
            "full_reward": False, "harbor_reward": 0.0, "sealed_authority": False, "error": str(error),
        }
        (args.output_root / "result.json").write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
        (args.output_root / "reward.txt").write_text("0.0\n", encoding="utf-8")
        print(f"Magic candidate verifier failed: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
