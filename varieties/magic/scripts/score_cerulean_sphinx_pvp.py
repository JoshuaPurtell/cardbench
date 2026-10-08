#!/usr/bin/env python3
"""Run Cerulean Sphinx in a bounded offline candidate worker and score its trace."""

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
MATERIALIZER = Path(__file__).with_name("materialize_cerulean_sphinx_task.py")
TARGET = Path("varieties/magic/sets/ravnica_city_of_guilds/src/cards/cerulean_sphinx.rs")
AUTHORITY_SOURCE = Path("varieties/magic/session/src/bin/rav_cerulean_sphinx_pvp.rs")
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
        "cp /trusted-authority.rs /work/varieties/magic/session/src/bin/rav_cerulean_sphinx_pvp.rs && "
        "cd /work && cargo run --offline --locked -j 2 --manifest-path varieties/magic/Cargo.toml "
        "-p cardbench-magic-session --bin rav-cerulean-sphinx-pvp --" + candidate_flag,
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
    raise WorkerFailure("candidate_protocol_error: no valid Cerulean Sphinx PvP result")


def validate_report(report: dict) -> None:
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
        or report.get("card") != {"collector_number": 39, "printings": 1, "definition_id": "RAV-CERULEAN-SPHINX", "name": "Cerulean Sphinx"}
    ):
        raise WorkerFailure("candidate_protocol_error: incomplete Cerulean Sphinx diagnostic result")
    candidate = report.get("candidate")
    if not isinstance(candidate, dict) or candidate.get("card_definition") != "RAV-CERULEAN-SPHINX":
        raise WorkerFailure("candidate_protocol_error: wrong Cerulean Sphinx candidate identity")
    traces = report.get("traces")
    if not isinstance(traces, list) or len(traces) != 1:
        raise WorkerFailure("candidate_protocol_error: one complete Cerulean Sphinx PvP trace is required")
    validate_trace(traces[0])
    mutation = report.get("mutation_sensitivity", {})
    required = (
        "flying_omitted_mutation_detected_in_game",
        "source_shuffle_effect_removed_mutation_detected_in_game",
        "activation_cost_changed_mutation_detected_in_game",
    )
    if not isinstance(mutation, dict) or any(mutation.get(key) is not True for key in required):
        raise WorkerFailure("candidate_protocol_error: Cerulean Sphinx semantic controls are incomplete")


def validate_trace(trace: object) -> None:
    if not isinstance(trace, dict) or trace.get("trace_id") != "rav-cerulean-sphinx-owner-library-shuffle-full-game" or trace.get("terminal") != "Winner(PlayerId(0))" or trace.get("winner_seat") != 0 or trace.get("rejected_moves") != 0 or not isinstance(trace.get("accepted_moves"), int) or trace["accepted_moves"] < 1:
        raise WorkerFailure("candidate_protocol_error: Cerulean Sphinx did not finish a clean full game")
    mechanics = trace.get("mechanics")
    if not isinstance(mechanics, dict):
        raise WorkerFailure("candidate_protocol_error: Cerulean Sphinx mechanics are missing")
    paid = mechanics.get("paid_mana_colors")
    if (
        mechanics.get("mana_cost") != "{4}{U}{U}"
        or not isinstance(paid, list)
        or len(paid) != 6
        or paid.count("Blue") < 2
        or not isinstance(mechanics.get("sphinx_object"), int)
        or mechanics.get("power") != 5
        or mechanics.get("toughness") != 5
        or mechanics.get("flying") is not True
        or mechanics.get("ability_id") != "shuffle-source-into-owner-library"
        or mechanics.get("ability_mana_cost") != "{U}"
        or mechanics.get("activation_mana_colors") != ["Blue"]
        or mechanics.get("ability_controller") != 1
        or mechanics.get("source_owner") != 0
        or mechanics.get("source_controller_before_activation") != 1
        or mechanics.get("source_zone_after_resolution") != "Library"
        or mechanics.get("shuffle_owner") != 0
        or not isinstance(mechanics.get("owner_library_count_before"), int)
        or mechanics.get("owner_library_count_after") != mechanics.get("owner_library_count_before", -2) + 1
        or mechanics.get("controller_library_count_after") != mechanics.get("controller_library_count_before")
        or not isinstance(mechanics.get("flash_conscription_object"), int)
        or mechanics.get("flash_conscription_resolved") is not True
        or not isinstance(mechanics.get("flash_conscription_paid_mana_colors"), list)
        or len(mechanics.get("flash_conscription_paid_mana_colors", [])) != 6
        or mechanics.get("flash_conscription_paid_mana_colors", []).count("Red") < 1
        or mechanics.get("attack_count", 0) < 3
        or mechanics.get("damage_to_opponent", 0) < 15
        or not isinstance(mechanics.get("post_activation_attacker_object"), int)
        or mechanics.get("post_activation_attack_count", 0) < 1
        or mechanics.get("post_activation_damage", 0) < 1
        or mechanics.get("deterministic_replay") is not True
        or mechanics.get("terminal_combat_win") is not True
    ):
        raise WorkerFailure("candidate_protocol_error: Cerulean Sphinx payment, control, owner-relative shuffle, flying, replay, or PvP result differs")

    events = trace.get("events")
    if not isinstance(events, list) or not all(isinstance(event, dict) for event in events):
        raise WorkerFailure("candidate_protocol_error: Cerulean Sphinx event trace is malformed")
    sequences = [event.get("sequence") for event in events]
    if any(not isinstance(sequence, int) for sequence in sequences) or sequences != sorted(sequences) or len(set(sequences)) != len(sequences):
        raise WorkerFailure("candidate_protocol_error: Cerulean Sphinx event order is malformed")
    by_kind: dict[str, list[dict]] = {}
    for event in events:
        by_kind.setdefault(event.get("kind"), []).append(event)
    required_kinds = {"SpellManaPaid", "SpellCast", "SpellResolved", "AbilityManaPaid", "AbilityActivated", "CardMoved", "LibraryShuffled", "AbilityResolved", "AttackersDeclared", "DamageDealtToPlayer", "GameEnded"}
    if not required_kinds.issubset(by_kind):
        raise WorkerFailure("candidate_protocol_error: Cerulean Sphinx cast, owner shuffle, combat, or terminal receipts are missing")
    sphinx = mechanics["sphinx_object"]
    combat_sphinx = mechanics["post_activation_attacker_object"]
    conscription = mechanics["flash_conscription_object"]
    payment = next((event for event in by_kind["SpellManaPaid"] if sphinx in event.get("facts", {}).get("objects", []) and event.get("facts", {}).get("seats") == [0]), None)
    cast = next((event for event in by_kind["SpellCast"] if sphinx in event.get("facts", {}).get("objects", []) and event.get("facts", {}).get("seats") == [0]), None)
    resolved = next((event for event in by_kind["SpellResolved"] if sphinx in event.get("facts", {}).get("objects", [])), None)
    conscription_paid = next((event for event in by_kind["SpellManaPaid"] if conscription in event.get("facts", {}).get("objects", []) and event.get("facts", {}).get("seats") == [1]), None)
    conscription_cast = next((event for event in by_kind["SpellCast"] if conscription in event.get("facts", {}).get("objects", []) and event.get("facts", {}).get("seats") == [1]), None)
    conscription_resolved = next((event for event in by_kind["SpellResolved"] if conscription in event.get("facts", {}).get("objects", [])), None)
    ability_paid = next((event for event in by_kind["AbilityManaPaid"] if sphinx in event.get("facts", {}).get("objects", []) and event.get("facts", {}).get("seats") == [1] and event.get("facts", {}).get("ability") == mechanics["ability_id"]), None)
    activation = next((event for event in by_kind["AbilityActivated"] if sphinx in event.get("facts", {}).get("objects", []) and event.get("facts", {}).get("seats") == [1] and event.get("facts", {}).get("ability") == mechanics["ability_id"]), None)
    moved = next((event for event in by_kind["CardMoved"] if sphinx in event.get("facts", {}).get("objects", []) and event.get("facts", {}).get("zone") == "Library"), None)
    shuffled = next((event for event in by_kind["LibraryShuffled"] if "player: PlayerId(0)" in event.get("canonical", "") and moved is not None and event["sequence"] > moved["sequence"]), None)
    ability_resolved = next((event for event in by_kind["AbilityResolved"] if sphinx in event.get("facts", {}).get("objects", []) and event.get("facts", {}).get("ability") == mechanics["ability_id"]), None)
    attackers = [event for event in by_kind["AttackersDeclared"] if combat_sphinx in event.get("facts", {}).get("objects", []) and event.get("facts", {}).get("seats") == [0]]
    combat_hits = [event for event in by_kind["DamageDealtToPlayer"] if event.get("facts", {}).get("seats") == [1] and combat_sphinx in event.get("facts", {}).get("objects", [])]
    ended = next((event for event in by_kind["GameEnded"] if event.get("facts", {}).get("seats") == [0]), None)
    if any(event is None for event in (payment, cast, resolved, conscription_paid, conscription_cast, conscription_resolved, ability_paid, activation, moved, shuffled, ability_resolved, ended)):
        raise WorkerFailure("candidate_protocol_error: exact Cerulean Sphinx/control/ability receipts are missing")
    if payment.get("facts", {}).get("mana_colors") != paid:
        raise WorkerFailure("candidate_protocol_error: typed Sphinx mana payment differs from authority summary")
    activation_colors = ability_paid.get("facts", {}).get("mana_cost_colors")
    if activation_colors != ["Blue"] or ability_paid.get("facts", {}).get("mana_cost_generic") != 0:
        raise WorkerFailure("candidate_protocol_error: activated ability cost is not exactly {U}")
    if len(attackers) < 3 or len(combat_hits) < 3:
        raise WorkerFailure("candidate_protocol_error: Cerulean Sphinx combat receipts are incomplete")
    post_ability_attacks = [event for event in attackers if event["sequence"] > ability_resolved["sequence"]]
    post_ability_hits = [event for event in combat_hits if event["sequence"] > ability_resolved["sequence"]]
    if not post_ability_attacks or not post_ability_hits:
        raise WorkerFailure("candidate_protocol_error: no Sphinx combat receipt follows the owner-library shuffle")
    chain = [payment, cast, resolved, conscription_paid, conscription_cast, conscription_resolved, ability_paid, activation, moved, shuffled, ability_resolved, min(post_ability_attacks, key=lambda event: event["sequence"]), min(post_ability_hits, key=lambda event: event["sequence"]), ended]
    if [event["sequence"] for event in chain] != sorted(event["sequence"] for event in chain):
        raise WorkerFailure("candidate_protocol_error: cast/control/ability/shuffle/combat event order differs")
    if f"source: ObjectId({sphinx})" not in ability_resolved.get("canonical", "") or f"player: PlayerId(0)" not in shuffled.get("canonical", ""):
        raise WorkerFailure("candidate_protocol_error: owner-specific shuffle or source resolution identity differs")

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
        raise FileNotFoundError(f"trusted Cerulean Sphinx authority source is missing: {authority}")
    if not shutil.which("docker"):
        raise FileNotFoundError("Docker is required for Magic candidate isolation")
    verify_workspace(workspace, manifest)
    scoring_workspace = make_scoring_snapshot(workspace, manifest_data["files"], output_root / "candidate-workspace")
    candidate_hash = sha256((scoring_workspace / TARGET).read_bytes())
    name = f"cardbench-magic-cerulean_sphinx-{uuid.uuid4().hex[:16]}"
    command = sandbox_command(scoring_workspace, cargo_registry.resolve(), authority, name, worker_image, candidate_kind)
    code, stdout, stderr = run_bounded(command, timeout=900, name=name)
    (output_root / "worker.stdout").write_bytes(stdout)
    (output_root / "worker.stderr").write_bytes(stderr)
    if code != 0:
        result = {
            "schema_version": "cardbench.magic.card-pvp.check.v1",
            "task_id": "cardbench/magic/card",
            "candidate": {"kind": candidate_kind, "source_sha256": candidate_hash},
            "passed": False,
            "pilot_card_passed": False,
            "cards_passed": 0,
            "cards_total": 291,
            "printings_total": 306,
            "cards_credited": 0,
            "coverage_complete": False,
            "full_reward": False,
            "harbor_reward": 0.0,
            "pilot_reward": 0.0,
            "sealed_authority": False,
            "worker_image": worker_image,
            "sandbox": {"network": "none", "rootfs": "read-only", "pids_limit": 128},
            "error": "candidate compile or Cerulean Sphinx reference traces failed",
            "diagnostics": stderr.decode(errors="replace")[-12000:],
        }
        (output_root / "result.json").write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
        print(json.dumps(result))
        return 1
    report = parse_report(stdout)
    report["candidate"].update({"kind": candidate_kind, "source_sha256": candidate_hash})
    report.update({"sealed_authority": False, "worker_image": worker_image, "sandbox": {"network": "none", "rootfs": "read-only", "pids_limit": 128}})
    (output_root / "result.json").write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    print(json.dumps(report))
    return 0


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--workspace", required=True, type=Path)
    parser.add_argument("--manifest", required=True, type=Path)
    parser.add_argument("--output-root", required=True, type=Path)
    parser.add_argument("--candidate-kind", choices=("agent", "reference"), default="agent")
    args = parser.parse_args()
    return score(args.workspace.resolve(), args.manifest.resolve(), args.output_root.resolve(), args.candidate_kind)


if __name__ == "__main__":
    raise SystemExit(main())
