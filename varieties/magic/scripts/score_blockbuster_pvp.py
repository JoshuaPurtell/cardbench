#!/usr/bin/env python3
"""Run the Blockbuster pilot in a networkless bounded worker."""

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
MATERIALIZER = Path(__file__).with_name("materialize_blockbuster_task.py")
TARGET = Path("varieties/magic/sets/ravnica_city_of_guilds/src/cards/blockbuster.rs")
AUTHORITY_SOURCE = Path("varieties/magic/session/src/bin/rav_blockbuster_pvp.rs")
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
        "cp /trusted-authority.rs /work/varieties/magic/session/src/bin/rav_blockbuster_pvp.rs && "
        "cd /work && cargo run --offline --locked -j 2 --manifest-path varieties/magic/Cargo.toml "
        "-p cardbench-magic-session --bin rav-blockbuster-pvp --" + candidate_flag,
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
    raise WorkerFailure("candidate_protocol_error: no valid Blockbuster PvP result")


def validate_report(report: dict) -> None:
    expected_card = {"collector_number": 115, "definition_id": "RAV-BLOCKBUSTER", "name": "Blockbuster"}
    if (
        report.get("task_id") != "cardbench/magic/card"
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
        or report.get("source_guard") != {
            "runs_before_candidate_functions": True,
            "syntactic_side_effect_fixture_rejected": True,
            "fixture_compiled": False,
        }
    ):
        raise WorkerFailure("candidate_protocol_error: incomplete Blockbuster 1/291 zero-credit diagnostic report")
    candidate = report.get("candidate")
    if not isinstance(candidate, dict) or candidate.get("card_definition") != "RAV-BLOCKBUSTER":
        raise WorkerFailure("candidate_protocol_error: wrong Blockbuster candidate identity")
    traces = report.get("traces")
    if not isinstance(traces, list) or len(traces) != 1:
        raise WorkerFailure("candidate_protocol_error: one Blockbuster full-game trace is required")
    trace = traces[0]
    if (
        not isinstance(trace, dict)
        or trace.get("trace_id") != "rav-blockbuster-source-sacrifice-tapped-damage-pvp"
        or trace.get("passed") is not True
        or trace.get("terminal") != "Winner(PlayerId(0))"
        or trace.get("winner_seat") != 0
        or trace.get("rejected_moves") != 0
    ):
        raise WorkerFailure("candidate_protocol_error: Blockbuster did not reach a clean seat-zero terminal game")
    card = trace.get("blockbuster")
    activation = trace.get("activation")
    effect = trace.get("effect")
    if not isinstance(card, dict) or card.get("collector_number") != 115 or card.get("mana_cost") != "{3}{R}{R}" or card.get("mana_paid") != ["Red"] * 5 or card.get("zone_after_cost") != "Graveyard":
        raise WorkerFailure("candidate_protocol_error: Blockbuster casting payment or sacrifice destination differs")
    if not isinstance(activation, dict) or activation.get("ability") != "sacrifice-source-deal-three-to-tapped-creatures-and-players" or activation.get("mana_cost") != "{1}{R}" or activation.get("source_sacrificed_as_cost") is not True or activation.get("source_moved_before_stack") is not True or activation.get("target_count") != 0:
        raise WorkerFailure("candidate_protocol_error: targetless activation must pay {1}{R} and sacrifice Blockbuster before stacking")
    if not isinstance(effect, dict) or effect.get("damage") != 3:
        raise WorkerFailure("candidate_protocol_error: global damage amount differs")
    tapped = effect.get("tapped_creatures_before_resolution")
    untapped = effect.get("untapped_creatures_before_resolution")
    creature_damage = effect.get("damaged_creatures")
    player_damage = effect.get("player_damage")
    if not isinstance(tapped, list) or not tapped or not isinstance(untapped, list) or set(tapped) & set(untapped):
        raise WorkerFailure("candidate_protocol_error: tap-state snapshot is incomplete or inconsistent")
    if not isinstance(creature_damage, list) or {item.get("object") for item in creature_damage if isinstance(item, dict)} != set(tapped) or any(item.get("amount") != 3 for item in creature_damage if isinstance(item, dict)):
        raise WorkerFailure("candidate_protocol_error: damage must hit exactly each tapped creature for 3")
    if not isinstance(player_damage, list) or [(item.get("seat"), item.get("amount")) for item in player_damage if isinstance(item, dict)] != [(0, 3), (1, 3)]:
        raise WorkerFailure("candidate_protocol_error: both players must receive three damage")
    if any(item.get("object") in set(untapped) for item in creature_damage if isinstance(item, dict)):
        raise WorkerFailure("candidate_protocol_error: untapped creatures were damaged")
    life_before, life_after = effect.get("life_before"), effect.get("life_after")
    if not isinstance(life_before, list) or not isinstance(life_after, list) or len(life_before) != 2 or len(life_after) != 2 or life_after != [life_before[0] - 3, life_before[1] - 3]:
        raise WorkerFailure("candidate_protocol_error: immediate life changes differ from the two player damage packets")
    zones = effect.get("zones_after_resolution")
    if not isinstance(zones, list):
        raise WorkerFailure("candidate_protocol_error: post-resolution creature zones missing")
    zone_by_object = {item.get("object"): item.get("zone") for item in zones if isinstance(item, dict)}
    if any(zone_by_object.get(object_id) != "Some(Graveyard)" for object_id in tapped) or any(zone_by_object.get(object_id) != "Some(Battlefield)" for object_id in untapped):
        raise WorkerFailure("candidate_protocol_error: tapped lethal creatures or untapped survivors are in the wrong zones")
    events = trace.get("events")
    if not isinstance(events, list) or not all(isinstance(event, dict) for event in events):
        raise WorkerFailure("candidate_protocol_error: projected typed events missing")
    kinds = [event.get("kind") for event in events]
    expected = ["SpellManaPaid", "SpellCast", "SpellResolved", "CardMoved", "AbilityManaPaid", "SacrificedAsAbilityCost", "CardMoved", "AbilityActivated"]
    if kinds[: len(expected)] != expected or kinds[-1] != "GameEnded" or kinds.count("AbilityActivated") != 1 or kinds.count("AbilityResolved") != 1 or kinds.count("DamageDealtToPlayer") != 2 or kinds.count("DamageDealtToPermanent") != len(tapped):
        raise WorkerFailure("candidate_protocol_error: ordered cast, source-cost, damage, and terminal event packets differ")
    if kinds.index("AbilityActivated") > min(index for index, kind in enumerate(kinds) if kind in {"DamageDealtToPermanent", "DamageDealtToPlayer"}):
        raise WorkerFailure("candidate_protocol_error: damage preceded the activated ability")
    if kinds.index("AbilityResolved") < max(index for index, kind in enumerate(kinds) if kind in {"DamageDealtToPermanent", "DamageDealtToPlayer"}):
        raise WorkerFailure("candidate_protocol_error: ability resolved before its complete damage batch")
    facts = [event.get("facts", {}) for event in events]
    source_id = card.get("object")
    if (
        facts[0].get("seats") != [0]
        or facts[0].get("objects") != [source_id]
        or facts[0].get("amount") != 5
        or facts[0].get("mana_colors") != ["Red"] * 5
        or facts[1].get("objects") != [source_id]
        or facts[2].get("objects") != [source_id]
        or facts[3].get("objects") != [source_id]
        or facts[3].get("zone") != "Battlefield"
    ):
        raise WorkerFailure("candidate_protocol_error: exact Blockbuster payment, cast, resolution, or entry facts differ")
    if (
        facts[4].get("seats") != [0]
        or facts[4].get("objects") != [source_id]
        or facts[4].get("ability") != activation.get("ability")
        or facts[4].get("mana_cost_generic") != 1
        or facts[4].get("mana_cost_colors") != ["Red"]
        or facts[5] != {}
        or facts[6].get("objects") != [source_id]
        or facts[6].get("zone") != "Graveyard"
        or facts[7].get("seats") != [0]
        or facts[7].get("objects") != [source_id]
        or facts[7].get("ability") != activation.get("ability")
    ):
        raise WorkerFailure("candidate_protocol_error: typed activation cost, source sacrifice, graveyard move, or stack facts differ")
    player_fact_seats = []
    damaged_objects = []
    for event in events:
        event_facts = event.get("facts", {})
        if event.get("kind") == "DamageDealtToPermanent":
            objects = event_facts.get("objects", [])
            if len(objects) != 2 or objects[0] != source_id or objects[1] not in set(tapped) or event_facts.get("amount") != 3:
                raise WorkerFailure("candidate_protocol_error: typed creature damage packet differs from the tapped snapshot")
            damaged_objects.append(objects[1])
        elif event.get("kind") == "DamageDealtToPlayer":
            if event_facts.get("objects") != [source_id] or event_facts.get("amount") != 3 or len(event_facts.get("seats", [])) != 1:
                raise WorkerFailure("candidate_protocol_error: typed player damage packet differs")
            player_fact_seats.extend(event_facts["seats"])
        elif event.get("kind") == "AbilityResolved":
            if event_facts.get("objects") != [source_id] or event_facts.get("ability") != activation.get("ability"):
                raise WorkerFailure("candidate_protocol_error: typed Blockbuster resolution fact differs")
        elif event.get("kind") == "GameEnded":
            if event_facts.get("seats") != [0]:
                raise WorkerFailure("candidate_protocol_error: terminal event is not a seat-zero win")
    if sorted(damaged_objects) != sorted(tapped) or sorted(player_fact_seats) != [0, 1]:
        raise WorkerFailure("candidate_protocol_error: exact typed tapped-creature and both-player damage set differs")
    sequences = [event.get("sequence") for event in events]
    if any(not isinstance(item, int) for item in sequences) or sequences != sorted(sequences):
        raise WorkerFailure("candidate_protocol_error: typed event sequence is malformed")
    mutations = report.get("mutation_sensitivity")
    if not isinstance(mutations, dict) or any(mutations.get(key) is not True for key in (
        "red_generic_activation_cost_mutation_detected",
        "source_sacrifice_mutation_detected",
        "tapped_creature_and_player_damage_mutation_detected",
    )):
        raise WorkerFailure("candidate_protocol_error: all in-memory Blockbuster semantic mutants must be rejected")

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
        raise FileNotFoundError(f"trusted Blockbuster authority source is missing: {authority}")
    if not shutil.which("docker"):
        raise FileNotFoundError("Docker is required for Magic candidate isolation")
    verify_workspace(workspace, manifest)
    scoring_workspace = make_scoring_snapshot(workspace, manifest_data["files"], output_root / "candidate-workspace")
    candidate_hash = sha256((scoring_workspace / TARGET).read_bytes())
    name = f"cardbench-magic-blockbuster-{uuid.uuid4().hex[:16]}"
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
            "error": "candidate compile or Blockbuster reference trace failed",
            "diagnostics": stderr.decode(errors="replace")[-12000:],
        }
    else:
        result = parse_report(stdout)
        result["candidate"] = {"kind": candidate_kind, "card_definition": "RAV-BLOCKBUSTER", "source_sha256": candidate_hash, "materialized_workspace_sha256": manifest_data["workspace_sha256"]}
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
