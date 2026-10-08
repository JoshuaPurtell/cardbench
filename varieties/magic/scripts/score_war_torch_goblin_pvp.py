#!/usr/bin/env python3
"""Run War-Torch Goblin in an offline, resource-limited candidate worker."""

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
MATERIALIZER = Path(__file__).with_name("materialize_war_torch_goblin_task.py")
TARGET = Path("varieties/magic/sets/ravnica_city_of_guilds/src/cards/war_torch_goblin.rs")
AUTHORITY_SOURCE = Path("varieties/magic/session/src/bin/rav_war_torch_goblin_pvp.rs")
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
        "cp /trusted-authority.rs /work/varieties/magic/session/src/bin/rav_war_torch_goblin_pvp.rs && "
        "cd /work && cargo run --offline --locked -j 2 --manifest-path varieties/magic/Cargo.toml "
        "-p cardbench-magic-session --bin rav-war-torch-goblin-pvp --" + candidate_flag,
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
    raise WorkerFailure("candidate_protocol_error: no valid War-Torch Goblin PvP result")


def validate_report(report: dict) -> None:
    expected_card = {"collector_number": 151, "definition_id": "RAV-WAR-TORCH-GOBLIN", "name": "War-Torch Goblin"}
    if (
        report.get("task_id") != "cardbench/magic/card"
        or report.get("passed") is not True
        or report.get("pilot_card_passed") is not True
        or report.get("deterministic_replay") is not True
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
    ):
        raise WorkerFailure("candidate_protocol_error: incomplete War-Torch Goblin single-card report")
    candidate = report.get("candidate")
    if not isinstance(candidate, dict) or candidate.get("card_definition") != "RAV-WAR-TORCH-GOBLIN":
        raise WorkerFailure("candidate_protocol_error: wrong War-Torch Goblin candidate identity")
    rules = report.get("rules_authority")
    if (
        not isinstance(rules, dict)
        or rules.get("comprehensive_rules") != "2026-08-07"
        or rules.get("authority_cutoff") != "2026-09-23"
        or "602.1-602.2 Activated abilities, targets, and costs" not in rules.get("checked_sections", [])
        or "701.17 Sacrifice" not in rules.get("checked_sections", [])
    ):
        raise WorkerFailure("candidate_protocol_error: War-Torch Goblin rules authority is inconsistent")
    traces = report.get("traces")
    if not isinstance(traces, list) or len(traces) != 1:
        raise WorkerFailure("candidate_protocol_error: one War-Torch Goblin trace is required")
    trace = traces[0]
    if (
        not isinstance(trace, dict)
        or trace.get("trace_id") != "rav-war-torch-goblin-blocker-sacrifice-pvp"
        or trace.get("passed") is not True
        or trace.get("terminal") != "Winner(PlayerId(0))"
        or trace.get("winner_seat") != 0
        or trace.get("rejected_moves") != 0
        or not isinstance(trace.get("shuffle_seed"), int)
    ):
        raise WorkerFailure("candidate_protocol_error: War-Torch Goblin did not reach a clean terminal win")
    goblin = trace.get("war_torch_goblin")
    attacker = trace.get("attacker")
    blocker = trace.get("blocker")
    activation = trace.get("activation")
    combat = trace.get("combat")
    if not all(isinstance(value, dict) for value in (goblin, attacker, blocker, activation, combat)):
        raise WorkerFailure("candidate_protocol_error: cast, payment, activation, blocker, or combat receipt is missing")
    goblin_id, attacker_id, blocker_id = goblin.get("object"), attacker.get("object"), blocker.get("object")
    if (
        not all(isinstance(value, int) for value in (goblin_id, attacker_id, blocker_id))
        or goblin.get("collector_number") != 151
        or goblin.get("mana_cost") != "{R}"
        or goblin.get("mana_paid") != ["Red"]
        or goblin.get("zone_after_cost") != "Graveyard"
        or attacker.get("definition") != "RAV-GOBLIN-FIRE-FIEND"
        or attacker.get("mana_cost") != "{3}{R}"
        or attacker.get("mana_paid") != ["Red"] * 4
        or attacker.get("haste_attack_same_turn") is not True
        or attacker.get("zone_at_terminal") != "Battlefield"
        or blocker.get("definition") != "RAV-TERRAFORMER"
        or blocker.get("blocked_attacker") != attacker_id
        or blocker.get("damage") != 2
        or blocker.get("zone_after_damage") != "Graveyard"
        or activation.get("ability") != "sacrifice-deal-two-to-blocker"
        or activation.get("source") != goblin_id
        or activation.get("mana_cost") != "{R}"
        or activation.get("red_mana_paid") is not True
        or activation.get("source_sacrificed_as_cost") is not True
        or activation.get("effect_damage_and_resolution_before_combat_damage") is not True
        or combat.get("blocker_died_before_combat_damage") is not True
        or combat.get("fire_fiend_later_attacked") is not True
        or combat.get("opponent_life_at_terminal", 1) > 0
        or combat.get("life_at_terminal", [0, 1])[1] > 0
    ):
        raise WorkerFailure("candidate_protocol_error: exact casting, blocker-target damage, sacrifice, or terminal facts differ")
    events = trace.get("events")
    expected_kinds = [
        "SpellCast", "CardMoved",
        "SpellManaPaid", "SpellCast", "SpellResolved", "CardMoved",
        "SpellManaPaid", "SpellCast", "SpellResolved", "CardMoved",
        "AttackersDeclared", "BlockersDeclared", "ManaAbilityActivated", "AbilityManaPaid",
        "SacrificedAsAbilityCost", "CardMoved", "AbilityActivated",
        "DamageDealtToPermanent", "AbilityResolved", "StateBasedAction",
        "CardMoved", "AttackersDeclared", "DamageDealtToPlayer", "GameEnded",
    ]
    if not isinstance(events, list) or [event.get("kind") for event in events if isinstance(event, dict)] != expected_kinds:
        raise WorkerFailure("candidate_protocol_error: ordered cast, block, sacrifice, damage, and later-combat events are incomplete")
    if any(not isinstance(event, dict) for event in events):
        raise WorkerFailure("candidate_protocol_error: event trace contains malformed entries")
    seq = [event.get("sequence") for event in events]
    if any(not isinstance(value, int) for value in seq) or seq != sorted(seq):
        raise WorkerFailure("candidate_protocol_error: projected event sequence is malformed")
    facts = [event.get("facts") or {} for event in events]
    def has(index: int, *, objects=None, seats=None, amount=None, zone=None, ability=None, reason=None, mana_colors=None, mana_cost_colors=None) -> bool:
        value = facts[index]
        return all((expected is None or value.get(field) == expected) for field, expected in (
            ("objects", objects), ("seats", seats), ("amount", amount), ("zone", zone),
            ("ability", ability), ("reason", reason), ("mana_colors", mana_colors),
            ("mana_cost_colors", mana_cost_colors),
        ))
    if not (
        has(0, objects=[blocker_id], seats=[1])
        and has(1, objects=[blocker_id], zone="Battlefield")
        and has(2, objects=[goblin_id], seats=[0], amount=1, mana_colors=["Red"])
        and has(3, objects=[goblin_id], seats=[0])
        and has(4, objects=[goblin_id])
        and has(5, objects=[goblin_id], zone="Battlefield")
        and has(6, objects=[attacker_id], seats=[0], amount=4, mana_colors=["Red"] * 4)
        and has(7, objects=[attacker_id], seats=[0])
        and has(8, objects=[attacker_id])
        and has(9, objects=[attacker_id], zone="Battlefield")
        and has(10, objects=[attacker_id], seats=[0])
        and has(11, objects=[attacker_id, blocker_id], seats=[1])
        and has(13, objects=[goblin_id], seats=[0], ability="sacrifice-deal-two-to-blocker", mana_cost_colors=["Red"])
        and has(15, objects=[goblin_id], zone="Graveyard")
        and has(16, objects=[goblin_id], seats=[0], ability="sacrifice-deal-two-to-blocker")
        and has(17, objects=[goblin_id, blocker_id], amount=2)
        and has(18, objects=[goblin_id], ability="sacrifice-deal-two-to-blocker")
        and has(19, objects=[blocker_id], reason="creature has lethal damage")
        and has(20, objects=[blocker_id], zone="Graveyard")
        and attacker_id in facts[21].get("objects", [])
        and facts[21].get("seats") == [0]
        and has(22, objects=[attacker_id], seats=[1], amount=1)
        and has(23, seats=[0])
    ):
        raise WorkerFailure("candidate_protocol_error: event facts do not bind costs, source/target, lethal move, and terminal combat")
    mutations = report.get("mutation_sensitivity")
    required = (
        "one_damage_mutation_detected_in_game",
        "activation_color_mutation_detected_in_game",
        "activation_tap_cost_mutation_detected_in_game",
        "source_sacrifice_mutation_detected_in_game",
        "blocker_target_mutation_detected_in_game",
        "red_cast_cost_mutation_detected_in_game",
    )
    if not isinstance(mutations, dict) or any(mutations.get(key) is not True for key in required):
        raise WorkerFailure("candidate_protocol_error: damage, activation, target, sacrifice, and cast-cost mutations are required")

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
        raise FileNotFoundError(f"trusted War-Torch Goblin authority is missing: {authority}")
    if not shutil.which("docker"):
        raise FileNotFoundError("Docker is required for Magic candidate isolation")
    verify_workspace(workspace, manifest)
    scoring_workspace = make_scoring_snapshot(workspace, manifest_data["files"], output_root / "candidate-workspace")
    candidate_hash = sha256((scoring_workspace / TARGET).read_bytes())
    name = f"cardbench-magic-war-torch-goblin-{uuid.uuid4().hex[:16]}"
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
            "error": "candidate compile or War-Torch Goblin reference trace failed",
            "diagnostics": stderr.decode(errors="replace")[-12000:],
        }
    else:
        result = parse_report(stdout)
        result["candidate"] = {
            "kind": candidate_kind,
            "card_definition": "RAV-WAR-TORCH-GOBLIN",
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
