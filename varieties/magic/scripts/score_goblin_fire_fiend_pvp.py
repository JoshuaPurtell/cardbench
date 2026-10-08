#!/usr/bin/env python3
"""Run Goblin Fire Fiend in an offline, resource-limited candidate worker."""

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
MATERIALIZER = Path(__file__).with_name("materialize_goblin_fire_fiend_task.py")
TARGET = Path("varieties/magic/sets/ravnica_city_of_guilds/src/cards/goblin_fire_fiend.rs")
AUTHORITY_SOURCE = Path("varieties/magic/session/src/bin/rav_goblin_fire_fiend_pvp.rs")
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
                target = output if key.data == "stdout" else diagnostics
                target.extend(chunk)
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
        "cp /trusted-authority.rs /work/varieties/magic/session/src/bin/rav_goblin_fire_fiend_pvp.rs && "
        "cd /work && cargo run --offline --locked -j 2 --manifest-path varieties/magic/Cargo.toml "
        "-p cardbench-magic-session --bin rav-goblin-fire-fiend-pvp --" + candidate_flag,
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
    raise WorkerFailure("candidate_protocol_error: no valid Goblin Fire Fiend PvP result")


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
        or report.get("viewer_scope") != "Judge"
        or report.get("card") != {"collector_number": 127, "definition_id": "RAV-GOBLIN-FIRE-FIEND", "name": "Goblin Fire Fiend"}
    ):
        raise WorkerFailure("candidate_protocol_error: incomplete Goblin Fire Fiend single-card report")
    candidate = report.get("candidate")
    if not isinstance(candidate, dict) or candidate.get("card_definition") != "RAV-GOBLIN-FIRE-FIEND":
        raise WorkerFailure("candidate_protocol_error: wrong Goblin Fire Fiend candidate identity")
    rules = report.get("rules_authority")
    if not isinstance(rules, dict) or rules.get("comprehensive_rules") != "2026-08-07" or rules.get("authority_cutoff") != "2026-09-23":
        raise WorkerFailure("candidate_protocol_error: Goblin Fire Fiend rules authority is inconsistent")
    traces = report.get("traces")
    if not isinstance(traces, list) or len(traces) != 1:
        raise WorkerFailure("candidate_protocol_error: one Goblin Fire Fiend trace is required")
    trace = traces[0]
    if (
        not isinstance(trace, dict)
        or trace.get("trace_id") != "rav-goblin-fire-fiend-haste-must-block-pump-pvp"
        or trace.get("passed") is not True
        or trace.get("terminal") != "Winner(PlayerId(0))"
        or trace.get("winner_seat") != 0
        or trace.get("rejected_moves") != 0
        or not isinstance(trace.get("shuffle_seed"), int)
    ):
        raise WorkerFailure("candidate_protocol_error: Goblin Fire Fiend did not reach a clean terminal win")
    fiend = trace.get("goblin_fire_fiend")
    blocker = trace.get("blocker")
    activation = trace.get("activation")
    combat = trace.get("combat")
    if not all(isinstance(value, dict) for value in (fiend, blocker, activation, combat)):
        raise WorkerFailure("candidate_protocol_error: cast, blocker, activation, or combat receipt is missing")
    fiend_id, blocker_id = fiend.get("object"), blocker.get("object")
    if (
        not isinstance(fiend_id, int)
        or not isinstance(blocker_id, int)
        or fiend.get("collector_number") != 127
        or fiend.get("mana_cost") != "{3}{R}"
        or fiend.get("mana_paid") != ["Red"] * 4
        or (fiend.get("base_power"), fiend.get("base_toughness")) != (1, 1)
        or fiend.get("haste_attack_same_turn") is not True
        or fiend.get("zone_after_combat") != "Graveyard"
        or blocker.get("definition") != "RAV-LURKING-INFORMANT"
        or blocker.get("power") != 1
        or blocker.get("toughness") != 2
        or blocker.get("damage") != 2
        or blocker.get("zone_after_damage") != "Graveyard"
        or blocker.get("blocked_fiend") != fiend_id
        or activation.get("ability") != "pump-plus-one-power"
        or activation.get("source") != fiend_id
        or activation.get("mana_cost") != "{R}"
        or activation.get("red_mana_paid") is not True
        or activation.get("power_after_resolution") != 2
        or activation.get("toughness_after_resolution") != 1
        or activation.get("effect_resolved_before_combat_damage") is not True
        or combat.get("must_block_requirement_satisfied") is not True
        or combat.get("fiend_dealt_lethal_damage") is not True
        or combat.get("opponent_life_at_terminal", 1) > 0
        or combat.get("life_at_terminal", [0, 1])[1] > 0
    ):
        raise WorkerFailure("candidate_protocol_error: exact cost, same-turn Haste, required block, pump, or lethal combat state differs")
    events = trace.get("events")
    expected_kinds = [
        "SpellManaPaid", "SpellCast", "SpellResolved", "CardMoved", "AttackersDeclared",
        "BlockersDeclared", "AbilityManaPaid", "AbilityActivated", "AbilityResolved",
        "DamageDealtToPermanent", "DamageDealtToPermanent", "StateBasedAction", "CardMoved",
        "StateBasedAction", "CardMoved", "AttackersDeclared", "DamageDealtToPlayer", "GameEnded",
    ]
    if not isinstance(events, list) or [event.get("kind") for event in events if isinstance(event, dict)] != expected_kinds:
        raise WorkerFailure("candidate_protocol_error: ordered spell, same-turn attack, required block, pump, and combat events are incomplete")
    if any(not isinstance(event, dict) for event in events):
        raise WorkerFailure("candidate_protocol_error: event trace contains malformed entries")
    sequence = [event.get("sequence") for event in events]
    if any(not isinstance(value, int) for value in sequence) or sequence != sorted(sequence):
        raise WorkerFailure("candidate_protocol_error: projected event sequence is malformed")
    facts = [event.get("facts") or {} for event in events]
    support_id = next(iter(facts[16].get("objects", [])), None)
    if not isinstance(support_id, int) or support_id == fiend_id:
        raise WorkerFailure("candidate_protocol_error: later combat damage source is missing")
    if not (
        facts[0].get("objects") == [fiend_id] and facts[0].get("seats") == [0]
        and facts[0].get("amount") == 4 and facts[0].get("mana_colors") == ["Red"] * 4
        and facts[1].get("objects") == [fiend_id] and facts[1].get("seats") == [0]
        and facts[2].get("objects") == [fiend_id]
        and facts[3].get("objects") == [fiend_id] and facts[3].get("zone") == "Battlefield"
        and fiend_id in facts[4].get("objects", []) and facts[4].get("seats") == [0]
        and facts[5].get("objects") == [fiend_id, blocker_id] and facts[5].get("seats") == [1]
        and facts[6].get("objects") == [fiend_id] and facts[6].get("seats") == [0]
        and facts[6].get("ability") == "pump-plus-one-power" and facts[6].get("mana_cost_colors") == ["Red"]
        and facts[7].get("objects") == [fiend_id] and facts[7].get("seats") == [0]
        and facts[7].get("ability") == "pump-plus-one-power"
        and facts[8].get("objects") == [fiend_id] and facts[8].get("ability") == "pump-plus-one-power"
        and facts[9].get("objects") == [fiend_id, blocker_id] and facts[9].get("amount") == 2
        and facts[10].get("objects") == [blocker_id, fiend_id] and facts[10].get("amount") == 1
        and facts[11].get("objects") == [fiend_id] and facts[11].get("reason") == "creature has lethal damage"
        and facts[12].get("objects") == [fiend_id] and facts[12].get("zone") == "Graveyard"
        and facts[13].get("objects") == [blocker_id] and facts[13].get("reason") == "creature has lethal damage"
        and facts[14].get("objects") == [blocker_id] and facts[14].get("zone") == "Graveyard"
        and support_id in facts[15].get("objects", []) and facts[15].get("seats") == [0]
        and support_id in facts[16].get("objects", []) and facts[16].get("seats") == [1]
        and (facts[16].get("amount") or 0) > 0 and facts[17].get("seats") == [0]
    ):
        raise WorkerFailure("candidate_protocol_error: event facts do not bind exact payments, object identities, target damage, and later combat")
    mutations = report.get("mutation_sensitivity")
    required = (
        "haste_removal_detected_in_game",
        "must_block_removal_detected_in_game",
        "zero_power_activation_detected_in_game",
        "activation_color_mutation_detected_in_game",
        "cast_cost_mutation_detected_in_game",
        "base_power_mutation_detected_in_game",
    )
    if not isinstance(mutations, dict) or any(mutations.get(key) is not True for key in required):
        raise WorkerFailure("candidate_protocol_error: Haste, must-block, pump, activation, cast-cost, and power mutations are required")

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
        raise FileNotFoundError(f"trusted Goblin Fire Fiend authority is missing: {authority}")
    if not shutil.which("docker"):
        raise FileNotFoundError("Docker is required for Magic candidate isolation")
    verify_workspace(workspace, manifest)
    scoring_workspace = make_scoring_snapshot(workspace, manifest_data["files"], output_root / "candidate-workspace")
    candidate_hash = sha256((scoring_workspace / TARGET).read_bytes())
    name = f"cardbench-magic-goblin-fire-fiend-{uuid.uuid4().hex[:16]}"
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
            "sealed_authority": False,
            "worker_image": worker_image,
            "sandbox": {"network": "none", "rootfs": "read-only", "pids_limit": 128},
            "error": "candidate compile or Goblin Fire Fiend reference trace failed",
            "diagnostics": stderr.decode(errors="replace")[-12000:],
        }
    else:
        result = parse_report(stdout)
        result["candidate"] = {
            "kind": candidate_kind,
            "card_definition": "RAV-GOBLIN-FIRE-FIEND",
            "source_sha256": candidate_hash,
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
            "schema_version": "cardbench.magic.card-pvp.check.v1",
            "task_id": "cardbench/magic/card",
            "passed": False,
            "pilot_card_passed": False,
            "cards_passed": 0,
            "cards_total": 291,
            "printings_total": 306,
            "cards_credited": 0,
            "coverage_complete": False,
            "full_reward": False,
            "harbor_reward": 0.0,
            "sealed_authority": False,
            "error": str(error),
        }
        (args.output_root / "result.json").write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
        (args.output_root / "reward.txt").write_text("0.0\n", encoding="utf-8")
        print(f"Magic candidate verifier failed: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
