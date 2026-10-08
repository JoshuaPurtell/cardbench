#!/usr/bin/env python3
"""Run the Vedalken Entrancer reference authority in a bounded offline worker."""

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
MATERIALIZER = Path(__file__).with_name("materialize_vedalken_entrancer_task.py")
TARGET = Path("varieties/magic/sets/ravnica_city_of_guilds/src/cards/vedalken_entrancer.rs")
AUTHORITY_SOURCE = Path("varieties/magic/session/src/bin/rav_vedalken_entrancer_pvp.rs")
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
        "cp /trusted-authority.rs /work/varieties/magic/session/src/bin/rav_vedalken_entrancer_pvp.rs && "
        "cd /work && cargo run --offline --locked -j 2 --manifest-path varieties/magic/Cargo.toml "
        "-p cardbench-magic-session --bin rav-vedalken-entrancer-pvp --" + candidate_flag,
    ]


def validate_report(report: dict) -> None:
    expected_card = {
        "collector_number": 74,
        "definition_id": "RAV-VEDALKEN-ENTRANCER",
        "name": "Vedalken Entrancer",
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
        raise WorkerFailure("candidate_protocol_error: incomplete Vedalken Entrancer single-card report")
    traces = report.get("traces")
    if not isinstance(traces, list) or len(traces) != 1:
        raise WorkerFailure("candidate_protocol_error: exactly one full-game Vedalken Entrancer trace is required")
    trace = traces[0]
    if (
        not isinstance(trace, dict)
        or trace.get("trace_id") != "rav-vedalken-entrancer-blue-target-mill-two-full-game"
        or trace.get("passed") is not True
        or trace.get("terminal") != "Winner(PlayerId(0))"
        or trace.get("winner_seat") != 0
        or not isinstance(trace.get("turns"), int)
        or trace["turns"] <= 0
        or trace.get("rejected_moves") != 0
    ):
        raise WorkerFailure("candidate_protocol_error: Vedalken Entrancer duel did not reach the expected terminal win")
    entrancer = trace.get("entrancer")
    activation = trace.get("activation")
    combat = trace.get("combat")
    if (
        not isinstance(entrancer, dict)
        or entrancer.get("collector_number") != 74
        or entrancer.get("mana_cost") != "{3}{U}"
        or len(entrancer.get("mana_paid", [])) != 4
        or "Blue" not in entrancer.get("mana_paid", [])
        or entrancer.get("power") != 1
        or entrancer.get("toughness") != 4
        or entrancer.get("defender") is not True
        or entrancer.get("resolved") is not True
        or not isinstance(activation, dict)
        or activation.get("ability_id") != "tap-blue-mill-two"
        or activation.get("mana_cost") != "{U}"
        or activation.get("tap_cost_paid") is not True
        or activation.get("active_player_at_activation") != 1
        or activation.get("target_player") != 1
        or activation.get("source_zone_after_resolution") != "Some(Battlefield)"
        or not isinstance(activation.get("source_object"), int)
        or activation.get("source_object") != entrancer.get("object")
        or not isinstance(activation.get("expected_top_objects"), list)
        or len(activation["expected_top_objects"]) != 2
        or activation.get("milled_objects") != activation.get("expected_top_objects")
        or len(set(activation["milled_objects"])) != 2
        or activation.get("milled_owners") != [1, 1]
        or activation.get("library_size_before", 0) - activation.get("library_size_after", 0) != 2
        or not isinstance(combat, dict)
        or combat.get("card") != "Watchwolf"
        or len(combat.get("attack_turns", [])) < 3
        or combat.get("damage_to_opponent", 0) < 9
        or not isinstance(combat.get("object"), int)
        or not isinstance(combat.get("life_at_terminal"), list)
        or len(combat["life_at_terminal"]) != 2
        or combat["life_at_terminal"][0] <= 0
        or combat["life_at_terminal"][1] > 0
    ):
        raise WorkerFailure("candidate_protocol_error: exact spell, ability, mill, or post-mill combat semantics are missing")
    events = trace.get("events")
    order = trace.get("ordered_event_sequence")
    if not isinstance(events, list) or not isinstance(order, list) or len(order) != 13:
        raise WorkerFailure("candidate_protocol_error: ordered Vedalken Entrancer event receipts are missing")
    sequences = [event.get("sequence") for event in events]
    if any(not isinstance(value, int) for value in sequences) or sequences != sorted(sequences):
        raise WorkerFailure("candidate_protocol_error: Vedalken Entrancer event sequence is malformed")
    if any(not isinstance(value, int) for value in order) or order != sorted(order):
        raise WorkerFailure("candidate_protocol_error: Vedalken Entrancer semantic event order is malformed")
    if len(events) != 13 or order != sequences:
        raise WorkerFailure("candidate_protocol_error: semantic sequence must preserve every ordered receipt")
    expected_kinds = [
        "SpellManaPaid", "SpellCast", "SpellResolved", "CardMoved",
        "AbilityManaPaid", "AbilityActivated", "CardMoved", "CardMoved",
        "AbilityResolved", "SpellCast", "AttackersDeclared", "DamageDealtToPlayer", "GameEnded",
    ]
    if [event.get("kind") for event in events] != expected_kinds:
        raise WorkerFailure("candidate_protocol_error: Entrancer spell, mill, and combat event order differs")
    source = entrancer["object"]
    target_objects = activation["milled_objects"]
    canonical = [event.get("canonical", "") for event in events]
    if not (
        f"card: ObjectId({source})" in canonical[0]
        and "PlayerId(0)" in canonical[0]
        and f"card: ObjectId({source})" in canonical[1]
        and f"card: ObjectId({source})" in canonical[2]
        and f"card: ObjectId({source})" in canonical[3]
        and "to: Battlefield" in canonical[3]
        and f"source: ObjectId({source})" in canonical[4]
        and 'ability: "tap-blue-mill-two"' in canonical[4]
        and "PlayerId(0)" in canonical[4]
        and f"source: ObjectId({source})" in canonical[5]
        and f"card: ObjectId({target_objects[0]})" in canonical[6]
        and "to: Graveyard" in canonical[6]
        and f"card: ObjectId({target_objects[1]})" in canonical[7]
        and "to: Graveyard" in canonical[7]
        and f"source: ObjectId({source})" in canonical[8]
        and "ability: \"tap-blue-mill-two\"" in canonical[8]
        and f"card: ObjectId({combat['object']})" in canonical[9]
        and "attackers:" in canonical[10]
        and f"ObjectId({combat['object']})" in canonical[10]
        and f"source: ObjectId({combat['object']})" in canonical[11]
        and "PlayerId(1)" in canonical[11]
        and "Some(PlayerId(0))" in canonical[12]
    ):
        raise WorkerFailure("candidate_protocol_error: event receipts do not identify exact Entrancer, target library objects, and Watchwolf")
    mutations = report.get("mutation_sensitivity")
    required_mutations = (
        "mill_amount_changed_mutation_detected_in_game",
        "tap_cost_removed_mutation_detected_in_game",
        "blue_cost_changed_mutation_detected_in_game",
        "player_target_removed_mutation_detected_in_game",
        "defender_removed_mutation_detected_in_game",
    )
    if not isinstance(mutations, dict) or any(mutations.get(key) is not True for key in required_mutations):
        raise WorkerFailure("candidate_protocol_error: all Vedalken Entrancer gameplay mutants must be detected")


def parse_report(stdout: bytes) -> dict:
    for line in reversed(stdout.decode(errors="replace").splitlines()):
        try:
            value = json.loads(line)
        except json.JSONDecodeError:
            continue
        if value.get("schema_version") == "cardbench.magic.card-pvp.check.v1":
            validate_report(value)
            return value
    raise WorkerFailure("candidate_protocol_error: no valid Vedalken Entrancer PvP result")


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
        raise FileNotFoundError(f"trusted Vedalken Entrancer authority source is missing: {authority}")
    if not shutil.which("docker"):
        raise FileNotFoundError("Docker is required for Magic candidate isolation")
    verify_workspace(workspace, manifest)
    scoring_workspace = make_scoring_snapshot(workspace, manifest_data["files"], output_root / "candidate-workspace")
    candidate_hash = sha256((scoring_workspace / TARGET).read_bytes())
    name = f"cardbench-magic-vedalken-entrancer-{uuid.uuid4().hex[:16]}"
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
            "error": "candidate compile or Vedalken Entrancer reference trace failed",
            "diagnostics": stderr.decode(errors="replace")[-12000:],
        }
    else:
        result = parse_report(stdout)
        result["candidate"] = {
            "kind": candidate_kind, "card_definition": "RAV-VEDALKEN-ENTRANCER", "source_sha256": candidate_hash,
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
