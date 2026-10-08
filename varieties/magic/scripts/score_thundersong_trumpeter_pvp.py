#!/usr/bin/env python3
"""Validate a local Thundersong reference report or run the isolated Harbor worker."""

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
MATERIALIZER = Path(__file__).with_name("materialize_thundersong_trumpeter_task.py")
TARGET = Path("varieties/magic/sets/ravnica_city_of_guilds/src/cards/thundersong_trumpeter.rs")
AUTHORITY_SOURCE = Path("varieties/magic/session/src/bin/rav_thundersong_trumpeter_pvp.rs")
WORKER_IMAGE = "rust@sha256:d7482085ff5b415f84dba5647ae71606650bdef00db7aeb69f4b3d170c3e4082"
MAX_OUTPUT_BYTES = 2 * 1024 * 1024


class WorkerFailure(RuntimeError):
    pass


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def validate_report(report: dict) -> None:
    if (
        report.get("schema_version") != "cardbench.magic.card-pvp.check.v1"
        or report.get("task_id") != "cardbench/magic/card"
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
        or report.get("card") != {
            "collector_number": 235,
            "printings": 1,
            "definition_id": "RAV-THUNDERSONG-TRUMPETER",
            "name": "Thundersong Trumpeter",
        }
    ):
        raise WorkerFailure("candidate_protocol_error: incomplete Thundersong diagnostic report or nonzero reward")

    candidate = report.get("candidate")
    if not isinstance(candidate, dict) or candidate.get("card_definition") != "RAV-THUNDERSONG-TRUMPETER":
        raise WorkerFailure("candidate_protocol_error: wrong Thundersong Trumpeter identity")

    traces = report.get("traces")
    if not isinstance(traces, list) or len(traces) != 1:
        raise WorkerFailure("candidate_protocol_error: one Thundersong full-game trace is required")
    trace = traces[0]
    activation = trace.get("activation") if isinstance(trace, dict) else None
    combat = trace.get("combat") if isinstance(trace, dict) else None
    card = trace.get("thundersong_trumpeter") if isinstance(trace, dict) else None
    if (
        not isinstance(trace, dict)
        or trace.get("id") != "rav-thundersong-trumpeter-restricts-benevolent-ancestor-block-full-game"
        or trace.get("passed") is not True
        or trace.get("terminal") != "Winner(PlayerId(0))"
        or trace.get("winner_seat") != 0
        or trace.get("rejected_moves") != 0
        or not isinstance(trace.get("turns"), int)
        or trace["turns"] < 1
        or not isinstance(card, dict)
        or card.get("collector_number") != 235
        or card.get("power") != 2
        or card.get("toughness") != 1
        or card.get("resolved") is not True
        or not isinstance(card.get("mana_paid"), list)
        or set(card["mana_paid"]) != {"Red", "White"}
        or not isinstance(activation, dict)
        or activation.get("ability_id") != "tap-prevent-target-combat"
        or activation.get("tap_cost") is not True
        or not isinstance(activation.get("activation_count"), int)
        or activation["activation_count"] < 1
        or activation.get("source") != card.get("object")
        or activation.get("target") == activation.get("source")
        or activation.get("active_player") != 0
        or activation.get("source_tapped_after_activation") is not True
        or activation.get("target_definition") != "RAV-BENEVOLENT-ANCESTOR"
        or activation.get("target_can_block_before") is not True
        or activation.get("target_tapped_after_resolution") is not False
        or activation.get("cannot_attack_or_block_effect_created") is not True
        or activation.get("target_has_combat_restriction") is not True
        or activation.get("target_can_block_at_declaration") is not False
        or activation.get("target_assigned_as_blocker") is not False
        or not isinstance(combat, dict)
        or combat.get("target_resolved") is not True
        or combat.get("watchwolf_damage_to_opponent", 0) < 20
        or combat.get("life_at_terminal", [20, 20])[1] > 0
        or combat.get("target_card_payment") != ["White", "White", "White"]
    ):
        raise WorkerFailure("candidate_protocol_error: Thundersong target, live state, or terminal match assertions failed")

    events = trace.get("events")
    spell_group = ["SpellManaPaid", "SpellCast", "SpellResolved", "CardMoved"]
    combat_group = [
        "AbilityActivated", "ContinuousEffectCreated", "AbilityResolved",
        "AttackersDeclared", "BlockersDeclared", "DamageDealtToPlayer", "GameEnded",
    ]
    if not isinstance(events, list) or len(events) != 15:
        raise WorkerFailure("candidate_protocol_error: incomplete projected Thundersong trace")
    kinds = [event.get("kind") for event in events if isinstance(event, dict)]
    sequences = [event.get("sequence") for event in events if isinstance(event, dict)]
    if (
        len(kinds) != 15
        or kinds[8:] != combat_group
        or any(not isinstance(sequence, int) for sequence in sequences)
        or sequences != sorted(sequences)
        or trace.get("ordered_event_sequence") != sequences
        or any(event.get("turn") != activation.get("activation_turn") for event in events[10:13])
    ):
        raise WorkerFailure("candidate_protocol_error: typed Thundersong event order differs")
    facts = [event.get("facts", {}) for event in events]
    trumpet = card.get("object")
    target = activation.get("target")
    source_start = next((index for index, fact in enumerate(facts[:8]) if fact.get("objects") == [trumpet] and fact.get("seats") == [0] and kinds[index] == "SpellManaPaid"), None)
    target_start = next((index for index, fact in enumerate(facts[:8]) if fact.get("objects") == [target] and fact.get("seats") == [1] and kinds[index] == "SpellManaPaid"), None)
    if (
        not isinstance(trumpet, int)
        or not isinstance(target, int)
        or source_start not in {0, 4}
        or target_start not in {0, 4}
        or source_start == target_start
        or kinds[source_start:source_start + 4] != spell_group
        or kinds[target_start:target_start + 4] != spell_group
        or any(fact.get("objects") != [trumpet] for fact in facts[source_start:source_start + 4])
        or any(fact.get("objects") != [target] for fact in facts[target_start:target_start + 4])
        or facts[source_start].get("amount") != 2
        or set(facts[source_start].get("mana_colors", [])) != {"Red", "White"}
        or facts[source_start].get("seats") != [0]
        or facts[target_start].get("seats") != [1]
        or facts[source_start + 3].get("zone") != "Battlefield"
        or facts[target_start + 3].get("zone") != "Battlefield"
        or facts[8].get("objects") != [trumpet]
        or facts[8].get("seats") != [0]
        or f"source: ObjectId({trumpet}), target: ObjectId({target})" not in events[9].get("canonical", "")
        or facts[10].get("objects") != [trumpet]
        or facts[11].get("seats") != [0]
        or facts[12].get("seats") != [1]
        or facts[12].get("amount") != 0
        or facts[13].get("objects") != [combat.get("watchwolf")]
        or facts[13].get("seats") != [1]
        or facts[13].get("amount", 0) <= 0
        or facts[14].get("seats") != [0]
    ):
        raise WorkerFailure("candidate_protocol_error: projected Thundersong event facts differ")

    mutations = report.get("mutation_sensitivity")
    required = (
        "combat_restriction_removed_detected",
        "wrong_tap_effect_detected",
        "source_tap_cost_removed_detected",
        "wrong_ability_mana_cost_detected",
        "wrong_target_requirement_detected",
        "wrong_power_detected",
    )
    if not isinstance(mutations, dict) or any(mutations.get(key) is not True for key in required):
        raise WorkerFailure("candidate_protocol_error: Thundersong gameplay mutation controls are incomplete")


def verify_workspace(workspace: Path, manifest: Path) -> None:
    subprocess.run(
        [sys.executable, str(MATERIALIZER), "--workspace", str(workspace), "--manifest", str(manifest), "--verify"],
        check=True,
        cwd=REPO,
    )


def make_snapshot(workspace: Path, files: dict[str, str], snapshot: Path) -> Path:
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
    output = bytearray()
    diagnostics = bytearray()
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
        "cp /trusted-authority.rs /work/varieties/magic/session/src/bin/rav_thundersong_trumpeter_pvp.rs && "
        "cd /work && cargo run --offline --locked -j 2 --manifest-path varieties/magic/Cargo.toml "
        "-p cardbench-magic-session --bin rav-thundersong-trumpeter-pvp --" + candidate_flag,
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
    raise WorkerFailure("candidate_protocol_error: no valid Thundersong Trumpeter PvP result")


def score(workspace: Path, manifest: Path, output_root: Path, candidate_kind: str) -> int:
    output_root.mkdir(parents=True, exist_ok=True)
    manifest_data = json.loads(manifest.read_text(encoding="utf-8"))
    worker_image = os.environ.get("CARDBENCH_MAGIC_WORKER_IMAGE", WORKER_IMAGE)
    if not (worker_image.startswith("rust@sha256:") or worker_image.startswith("sha256:")):
        raise ValueError("CARDBENCH_MAGIC_WORKER_IMAGE must be an immutable digest reference")
    registry = Path.home() / ".cargo" / "registry"
    if not registry.is_dir():
        raise FileNotFoundError(f"offline Cargo registry cache is missing: {registry}")
    authority = REPO / AUTHORITY_SOURCE
    if authority.is_symlink() or not authority.is_file():
        raise FileNotFoundError(f"trusted Thundersong authority source is missing: {authority}")
    if not shutil.which("docker"):
        raise FileNotFoundError("Docker is required for Magic candidate isolation")
    verify_workspace(workspace, manifest)
    snapshot = make_snapshot(workspace, manifest_data["files"], output_root / "candidate-workspace")
    candidate_hash = sha256((snapshot / TARGET).read_bytes())
    name = f"cardbench-magic-thundersong-{uuid.uuid4().hex[:16]}"
    command = sandbox_command(snapshot, registry.resolve(), authority, name, worker_image, candidate_kind)
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
            "error": "candidate compile or Thundersong full-game reference trace failed",
            "diagnostics": stderr.decode(errors="replace")[-12000:],
        }
    else:
        result = parse_report(stdout)
        result["candidate"] = {
            "kind": candidate_kind,
            "card_definition": "RAV-THUNDERSONG-TRUMPETER",
            "source_sha256": candidate_hash,
            "materialized_workspace_sha256": manifest_data["workspace_sha256"],
        }
        result["candidate_kind"] = candidate_kind
        result["starting_workspace_sha256"] = manifest_data["workspace_sha256"]
        result["sealed_authority"] = False
        result["worker_image"] = worker_image
        result["sandbox"] = {"network": "none", "rootfs": "read-only", "pids_limit": 128}
        result["harbor_reward"] = 0.0
        result["pilot_reward"] = 0.0
        result["full_reward"] = False
    (output_root / "result.json").write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
    (output_root / "reward.txt").write_text("0.0\n", encoding="utf-8")
    print(f"candidate_passed={result.get('pilot_card_passed', False)} cards={result.get('cards_passed', 0)}/291 credited=0 pilot_reward=0.0")
    return 0 if result.get("passed") is True and result.get("pilot_card_passed") is True else 1


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--workspace", type=Path)
    parser.add_argument("--manifest", type=Path)
    parser.add_argument("--output-root", type=Path)
    parser.add_argument("--candidate-kind", choices=["agent", "reference"], default="agent")
    parser.add_argument("--report-file", type=Path, help="validate a local authority report without Docker")
    args = parser.parse_args()
    if args.report_file:
        try:
            validate_report(json.loads(args.report_file.read_text(encoding="utf-8")))
            print("local_reference_report=valid cards=1/291 credited=0 pilot_reward=0.0")
            return 0
        except Exception as error:
            print(f"Thundersong Trumpeter report validation failed: {error}", file=sys.stderr)
            return 1
    if not args.workspace or not args.manifest or not args.output_root:
        parser.error("--workspace, --manifest, and --output-root are required unless --report-file is used")
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
            "pilot_reward": 0.0,
            "sealed_authority": False,
            "error": str(error),
        }
        (args.output_root / "result.json").write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
        (args.output_root / "reward.txt").write_text("0.0\n", encoding="utf-8")
        print(f"Magic candidate verifier failed: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
