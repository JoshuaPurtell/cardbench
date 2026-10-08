#!/usr/bin/env python3
"""Run the Autochthon Wurm reference authority in a bounded offline worker."""

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
MATERIALIZER = Path(__file__).with_name("materialize_autochthon_wurm_task.py")
TARGET = Path("varieties/magic/sets/ravnica_city_of_guilds/src/cards/autochthon_wurm.rs")
AUTHORITY_SOURCE = Path("varieties/magic/session/src/bin/rav_autochthon_wurm_pvp.rs")
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
        "cp /trusted-authority.rs /work/varieties/magic/session/src/bin/rav_autochthon_wurm_pvp.rs && "
        "cd /work && cargo run --offline --locked -j 2 --manifest-path varieties/magic/Cargo.toml "
        "-p cardbench-magic-session --bin rav-autochthon-wurm-pvp --" + candidate_flag,
    ]


def validate_report(report: dict) -> None:
    expected_card = {
        "collector_number": 191,
        "definition_id": "RAV-AUTOCHTHON-WURM",
        "name": "Autochthon Wurm",
        "printings": 1,
    }
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
        or report.get("card") != expected_card
        or report.get("deterministic_replay") is not True
    ):
        raise WorkerFailure("candidate_protocol_error: incomplete Autochthon Wurm single-card report")

    traces = report.get("traces")
    if not isinstance(traces, list) or len(traces) != 1:
        raise WorkerFailure("candidate_protocol_error: exactly one full-game Autochthon Wurm trace is required")
    trace = traces[0]
    blockers = trace.get("blockers") if isinstance(trace, dict) else None
    blocker_ids = [entry.get("object") for entry in blockers if isinstance(entry, dict)] if isinstance(blockers, list) else []
    if (
        not isinstance(trace, dict)
        or trace.get("id") != "rav-autochthon-wurm-nine-convoke-multi-block-trample-terminal"
        or trace.get("passed") is not True
        or trace.get("terminal") != "Winner(PlayerId(0))"
        or trace.get("winner_seat") != 0
        or not isinstance(trace.get("turns"), int)
        or not 0 < trace["turns"] <= 60
        or trace.get("rejected_moves") != 0
        or trace.get("spell_zone") != "Some(Battlefield)"
        or trace.get("live_power_toughness") != [9, 14]
        or trace.get("live_convoke") is not True
        or trace.get("live_trample") is not True
        or not isinstance(trace.get("spell_object"), int)
        or len(blocker_ids) != 2
        or len(set(blocker_ids)) != 2
        or any(entry.get("definition") not in {"RAV-BIRDS-OF-PARADISE", "RAV-CAREGIVER"} for entry in blockers if isinstance(entry, dict))
        or trace.get("ordered_blockers") != blocker_ids
        or trace.get("blocker_zones_after_combat") != ["Graveyard", "Graveyard"]
        or not isinstance(trace.get("opponent_life_after_match"), int)
        or trace["opponent_life_after_match"] > 0
    ):
        raise WorkerFailure("candidate_protocol_error: Autochthon Wurm identity, mechanics, blocker order, or terminal game is invalid")

    payments = trace.get("convoke_payments")
    if (
        not isinstance(payments, list)
        or len(payments) != 9
        or len({item.get("creature") for item in payments if isinstance(item, dict)}) != 9
        or any(not isinstance(item, dict) or item.get("tapped") is not True for item in payments)
        or sum(item.get("contribution") == "Green" for item in payments if isinstance(item, dict)) != 2
        or sum(item.get("contribution") == "White" for item in payments if isinstance(item, dict)) != 2
        or sum(item.get("contribution") == "Generic" for item in payments if isinstance(item, dict)) != 5
        or any(item.get("contribution") not in {"Green", "White", "Generic"} for item in payments if isinstance(item, dict))
        or any(item.get("definition") not in {"RAV-BIRDS-OF-PARADISE", "RAV-SELESNYA-EVANGEL", "RAV-TRANSLUMINANT", "Saproling"} for item in payments if isinstance(item, dict))
        or trace.get("mana_paid") != ["Green"] * 6
    ):
        raise WorkerFailure("candidate_protocol_error: nine distinct Convoke payers must provide two green, two white, five generic, plus six pool mana")

    damage = trace.get("damage_to_blockers")
    trample = trace.get("trample_damage_to_opponent")
    if (
        not isinstance(damage, list)
        or sorted((entry.get("object"), entry.get("amount")) for entry in damage if isinstance(entry, dict))
            != sorted((blocker, 1) for blocker in blocker_ids)
        or not isinstance(trample, list)
        or len(trample) < 2
        or trample[0] != 7
        or sum(trample) < 20
    ):
        raise WorkerFailure("candidate_protocol_error: both blockers must receive lethal damage and Autochthon Wurm must deal seven excess Trample damage")

    events = trace.get("events")
    order = trace.get("ordered_event_sequence")
    if not isinstance(events, list) or not isinstance(order, list) or len(order) < 25:
        raise WorkerFailure("candidate_protocol_error: ordered Autochthon Wurm event receipts are missing")
    sequences = [event.get("sequence") for event in events if isinstance(event, dict)]
    if len(sequences) != len(events) or any(not isinstance(value, int) for value in sequences) or sequences != sorted(sequences):
        raise WorkerFailure("candidate_protocol_error: Autochthon Wurm event sequence is malformed")
    if any(not isinstance(value, int) for value in order) or order != sorted(order):
        raise WorkerFailure("candidate_protocol_error: Autochthon Wurm semantic event order is malformed")

    def first(kind: str, predicate) -> int:
        for index, event in enumerate(events):
            if event.get("kind") == kind and predicate(event.get("canonical", "")):
                return index
        raise WorkerFailure(f"candidate_protocol_error: {kind} receipt is missing")

    spell = trace["spell_object"]
    convoke_indices = [
        index for index, event in enumerate(events)
        if event.get("kind") == "ConvokeUsed" and "player: PlayerId(0)" in event.get("canonical", "")
    ]
    if len(convoke_indices) != 9:
        raise WorkerFailure("candidate_protocol_error: exactly nine ConvokeUsed events must be serialized")
    for payment, index in zip(payments, convoke_indices, strict=True):
        canonical = events[index]["canonical"]
        contribution = {"Green": "Some(Green)", "White": "Some(White)", "Generic": "None"}[payment["contribution"]]
        if f"creature: ObjectId({payment['creature']})" not in canonical or f"contribution: {contribution}" not in canonical:
            raise WorkerFailure("candidate_protocol_error: Convoke payer report does not match its engine receipt")

    cast = first("SpellCast", lambda value: f"card: ObjectId({spell})" in value and "PlayerId(0)" in value)
    mana_paid = first("SpellManaPaid", lambda value: f"card: ObjectId({spell})" in value and "PlayerId(0)" in value and "colors: [Green, Green, Green, Green, Green, Green]" in value)
    resolved = first("SpellResolved", lambda value: f"card: ObjectId({spell})" in value)
    entered = first("CardMoved", lambda value: f"card: ObjectId({spell})" in value and "to: Battlefield" in value)
    attack = first("AttackersDeclared", lambda value: "PlayerId(0)" in value and f"ObjectId({spell})" in value)
    blockers_at = first("BlockersDeclared", lambda value: f"(ObjectId({spell}), ObjectId({blocker_ids[0]}))" in value and f"(ObjectId({spell}), ObjectId({blocker_ids[1]}))" in value)
    damage_indices = [first("DamageDealtToPermanent", lambda value, blocker=blocker: f"source: ObjectId({spell})" in value and f"permanent: ObjectId({blocker})" in value and "amount: 1" in value) for blocker in blocker_ids]
    trample_at = first("DamageDealtToPlayer", lambda value: f"source: ObjectId({spell})" in value and "player: PlayerId(1)" in value and "amount: 7" in value)
    order_at = first("CombatDamageOrderChosen", lambda value: f"attacker: ObjectId({spell})" in value and all(f"ObjectId({blocker})" in value for blocker in blocker_ids))
    sba_indices = [first("StateBasedAction", lambda value, blocker=blocker: f"card: ObjectId({blocker})" in value and "creature has lethal damage" in value) for blocker in blocker_ids]
    grave_indices = [first("CardMoved", lambda value, blocker=blocker: f"card: ObjectId({blocker})" in value and "to: Graveyard" in value) for blocker in blocker_ids]
    terminal = first("GameEnded", lambda value: "Some(PlayerId(0))" in value)
    development = trace.get("development_spells")
    if not isinstance(development, list) or not any(isinstance(item, dict) and item.get("definition") == "RAV-SCATTER-THE-SEEDS" for item in development):
        raise WorkerFailure("candidate_protocol_error: the shuffled pilot deck must cast Scatter the Seeds before Autochthon Wurm")
    scatter = next(item["object"] for item in development if item.get("definition") == "RAV-SCATTER-THE-SEEDS")
    scatter_cast = first("SpellCast", lambda value: f"card: ObjectId({scatter})" in value and "PlayerId(0)" in value)
    scatter_resolved = first("SpellResolved", lambda value: f"card: ObjectId({scatter})" in value)
    if not scatter_cast < scatter_resolved < cast:
        raise WorkerFailure("candidate_protocol_error: Scatter the Seeds must resolve before Autochthon Wurm")
    for payment in payments:
        if payment["definition"] == "Saproling":
            first("TokenCreated", lambda value, creature=payment["creature"]: "player: PlayerId(0)" in value and f"token: ObjectId({creature})" in value)
    if not (
        max(convoke_indices) < mana_paid < cast < resolved < entered < attack < blockers_at < order_at
        and max(damage_indices) < min(sba_indices) < min(grave_indices) < terminal
        and trample_at < min(sba_indices)
    ):
        raise WorkerFailure("candidate_protocol_error: Autochthon Wurm Convoke, cast, multi-block order, damage, SBA, graveyard, and terminal event order differs")

    for blocker in blocker_ids:
        blocker_cast = first("SpellCast", lambda value, blocker=blocker: f"card: ObjectId({blocker})" in value and "PlayerId(1)" in value)
        blocker_resolved = first("SpellResolved", lambda value, blocker=blocker: f"card: ObjectId({blocker})" in value)
        blocker_entered = first("CardMoved", lambda value, blocker=blocker: f"card: ObjectId({blocker})" in value and "to: Battlefield" in value)
        if not blocker_cast < blocker_resolved < blocker_entered < blockers_at:
            raise WorkerFailure("candidate_protocol_error: both exact blockers must be cast and resolve before blocking")

    mutations = report.get("mutation_sensitivity")
    required_mutations = ("convoke_removed_mutation_detected_in_game", "trample_removed_mutation_detected_in_combat", "printed_cost_changed_mutation_detected")
    if not isinstance(mutations, dict) or any(mutations.get(key) is not True for key in required_mutations):
        raise WorkerFailure("candidate_protocol_error: all Autochthon Wurm gameplay mutants must be detected")


def parse_report(stdout: bytes) -> dict:
    for line in reversed(stdout.decode(errors="replace").splitlines()):
        try:
            value = json.loads(line)
        except json.JSONDecodeError:
            continue
        if value.get("schema_version") == "cardbench.magic.card-pvp.check.v1":
            validate_report(value)
            return value
    raise WorkerFailure("candidate_protocol_error: no valid Autochthon Wurm PvP result")


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
        raise FileNotFoundError(f"trusted Autochthon Wurm authority source is missing: {authority}")
    if not shutil.which("docker"):
        raise FileNotFoundError("Docker is required for Magic candidate isolation")
    verify_workspace(workspace, manifest)
    scoring_workspace = make_scoring_snapshot(workspace, manifest_data["files"], output_root / "candidate-workspace")
    candidate_hash = sha256((scoring_workspace / TARGET).read_bytes())
    name = f"cardbench-magic-autochthon-wurm-{uuid.uuid4().hex[:16]}"
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
            "error": "candidate compile or Autochthon Wurm reference trace failed",
            "diagnostics": stderr.decode(errors="replace")[-12000:],
        }
    else:
        result = parse_report(stdout)
        result["candidate"] = {
            "kind": candidate_kind, "card_definition": "RAV-AUTOCHTHON-WURM", "source_sha256": candidate_hash,
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
            "full_reward": False, "harbor_reward": 0.0, "pilot_reward": 0.0,
            "sealed_authority": False, "error": str(error),
        }
        (args.output_root / "result.json").write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
        (args.output_root / "reward.txt").write_text("0.0\n", encoding="utf-8")
        print(f"Magic candidate verifier failed: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
