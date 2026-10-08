#!/usr/bin/env python3
"""Run the Torpid Moloch pilot in a networkless bounded worker."""

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
MATERIALIZER = Path(__file__).with_name("materialize_torpid_moloch_task.py")
TARGET = Path("varieties/magic/sets/ravnica_city_of_guilds/src/cards/torpid_moloch.rs")
AUTHORITY_SOURCE = Path("varieties/magic/session/src/bin/rav_torpid_moloch_pvp.rs")
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
        "cp /trusted-authority.rs /work/varieties/magic/session/src/bin/rav_torpid_moloch_pvp.rs && "
        "cd /work && cargo run --offline --locked -j 2 --manifest-path varieties/magic/Cargo.toml "
        "-p cardbench-magic-session --bin rav-torpid-moloch-pvp --" + candidate_flag,
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
    raise WorkerFailure("candidate_protocol_error: no valid Torpid Moloch PvP result")


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
        or report.get("deterministic_replay") is not True
        or report.get("card") != {"collector_number": 147, "definition_id": "RAV-TORPID-MOLOCH", "name": "Torpid Moloch"}
    ):
        raise WorkerFailure("candidate_protocol_error: incomplete Torpid Moloch 1/291 diagnostic report")
    traces = report.get("traces")
    if not isinstance(traces, list) or len(traces) != 1:
        raise WorkerFailure("candidate_protocol_error: one Torpid Moloch full-game trace is required")
    trace = traces[0]
    if (
        not isinstance(trace, dict)
        or trace.get("trace_id") != "rav-torpid-moloch-sacrifice-remove-defender-repeat-attack"
        or trace.get("passed") is not True
        or trace.get("terminal") != "Winner(PlayerId(0))"
        or trace.get("winner_seat") != 0
        or trace.get("rejected_moves") != 0
    ):
        raise WorkerFailure("candidate_protocol_error: Torpid Moloch did not reach a clean seat-zero terminal game")
    creature = trace.get("torpid_moloch")
    activation = trace.get("activation")
    combat = trace.get("combat")
    if (
        not isinstance(creature, dict)
        or creature.get("collector_number") != 147
        or creature.get("mana_cost") != "{R}"
        or creature.get("mana_paid") != ["Red"]
        or creature.get("power") != 3
        or creature.get("toughness") != 2
        or creature.get("defender_on_entry") is not True
        or creature.get("defender_before_activations") != [True] * 7
        or creature.get("defender_removed_for_each_attack") is not True
        or not isinstance(activation, dict)
        or activation.get("ability") != "sacrifice-three-lands-remove-defender"
        or activation.get("mana_cost") != "{0}"
        or activation.get("activations") != 7
        or activation.get("lands_sacrificed") != 21
        or activation.get("land_graveyard_moves") != 21
        or activation.get("defender_present_after_resolution") != [False] * 7
        or not isinstance(combat, dict)
        or combat.get("attacks") != 7
        or combat.get("empty_legal_blocks") != 7
        or combat.get("damage_per_attack") != 3
        or not isinstance(combat.get("opponent_life_at_terminal"), int)
        or combat["opponent_life_at_terminal"] > 0
    ):
        raise WorkerFailure("candidate_protocol_error: Torpid Moloch characteristics, exact sacrifice count, temporary keyword removal, or combat outcome differ")
    events = trace.get("events")
    expected = ["SpellManaPaid", "SpellCast", "SpellResolved", "CardMoved"]
    for cycle in range(7):
        expected += ["SacrificedAsAbilityCost", "CardMoved"] * 3
        expected += ["AbilityActivated", "ContinuousEffectCreated", "AbilityResolved", "AttackersDeclared", "BlockersDeclared", "DamageDealtToPlayer"]
        if cycle < 6:
            expected.append("ContinuousEffectExpired")
    expected.append("GameEnded")
    if not isinstance(events, list) or not all(isinstance(event, dict) for event in events) or [event.get("kind") for event in events] != expected:
        raise WorkerFailure("candidate_protocol_error: ordered typed cast, land-payment, ability, effect, combat, expiry, and terminal events differ")
    sequences = [event.get("sequence") for event in events]
    if any(not isinstance(item, int) for item in sequences) or sequences != sorted(sequences):
        raise WorkerFailure("candidate_protocol_error: typed event sequence is malformed")
    first_facts = [event.get("facts", {}) for event in events[:4]]
    source = creature.get("object")
    if first_facts[0].get("seats") != [0] or first_facts[0].get("objects") != [source] or first_facts[0].get("amount") != 1 or first_facts[0].get("mana_colors") != ["Red"]:
        raise WorkerFailure("candidate_protocol_error: exact red spell payment receipt differs")
    if first_facts[1].get("seats") != [0] or first_facts[1].get("objects") != [source] or first_facts[2].get("objects") != [source] or first_facts[3].get("objects") != [source] or first_facts[3].get("zone") != "Battlefield":
        raise WorkerFailure("candidate_protocol_error: Torpid Moloch cast, resolution, or entry receipts differ")
    cursor = 4
    for cycle in range(7):
        land_objects = []
        for _ in range(3):
            if events[cursor].get("facts") is not None:
                raise WorkerFailure("candidate_protocol_error: sacrifice event must use the typed canonical payload")
            move = events[cursor + 1].get("facts", {})
            if len(move.get("objects", [])) != 1 or move.get("zone") != "Graveyard":
                raise WorkerFailure("candidate_protocol_error: each sacrificed land must move to the graveyard")
            land_objects.extend(move["objects"])
            cursor += 2
        if len(set(land_objects)) != 3 or source in land_objects:
            raise WorkerFailure("candidate_protocol_error: activation must pay three distinct lands, not the source")
        activated = events[cursor].get("facts", {})
        created = events[cursor + 1]
        resolved = events[cursor + 2].get("facts", {})
        attackers = events[cursor + 3].get("facts", {})
        blockers = events[cursor + 4].get("facts", {})
        damage = events[cursor + 5].get("facts", {})
        if activated.get("seats") != [0] or activated.get("objects") != [source] or activated.get("ability") != "sacrifice-three-lands-remove-defender":
            raise WorkerFailure("candidate_protocol_error: activation must follow its three land payments")
        created_payload = created.get("canonical", "")
        if created.get("facts") is not None or f"source: ObjectId({source}), target: ObjectId({source}), layer: Ability" not in created_payload or resolved.get("objects") != [source] or resolved.get("ability") != "sacrifice-three-lands-remove-defender":
            raise WorkerFailure("candidate_protocol_error: source-targeted temporary keyword effect must resolve")
        if attackers.get("seats") != [0] or attackers.get("objects") != [source] or blockers.get("seats") != [1] or blockers.get("amount") != 0:
            raise WorkerFailure("candidate_protocol_error: Defender removal must enable a legal attack and empty block")
        if damage.get("seats") != [1] or damage.get("objects") != [source] or damage.get("amount") != 3:
            raise WorkerFailure("candidate_protocol_error: each attack must deal three combat damage to the opponent")
        cursor += 6
        if cycle < 6:
            expired = events[cursor]
            expired_payload = expired.get("canonical", "")
            if expired.get("facts") is not None or f"source: ObjectId({source}), target: ObjectId({source}), layer: Ability" not in expired_payload:
                raise WorkerFailure("candidate_protocol_error: Defender-removal effect must expire after the turn")
            cursor += 1
    terminal = events[-1].get("facts", {})
    if terminal.get("seats") != [0]:
        raise WorkerFailure("candidate_protocol_error: terminal winner receipt differs")
    mutation = report.get("mutation_sensitivity", {})
    if not isinstance(mutation, dict) or any(mutation.get(key) is not True for key in ("printed_defender_removed_mutation_detected", "three_land_cost_mutation_detected", "defender_removal_effect_mutation_detected")):
        raise WorkerFailure("candidate_protocol_error: all three compiled semantic mutants must be rejected")

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
        raise FileNotFoundError(f"trusted Torpid Moloch authority source is missing: {authority}")
    if not shutil.which("docker"):
        raise FileNotFoundError("Docker is required for Magic candidate isolation")
    verify_workspace(workspace, manifest)
    scoring_workspace = make_scoring_snapshot(workspace, manifest_data["files"], output_root / "candidate-workspace")
    candidate_hash = sha256((scoring_workspace / TARGET).read_bytes())
    name = f"cardbench-magic-torpid-moloch-{uuid.uuid4().hex[:16]}"
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
            "error": "candidate compile or Torpid Moloch reference trace failed",
            "diagnostics": stderr.decode(errors="replace")[-12000:],
        }
    else:
        result = parse_report(stdout)
        result["candidate"] = {"kind": candidate_kind, "card_definition": "RAV-TORPID-MOLOCH", "source_sha256": candidate_hash, "materialized_workspace_sha256": manifest_data["workspace_sha256"]}
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
