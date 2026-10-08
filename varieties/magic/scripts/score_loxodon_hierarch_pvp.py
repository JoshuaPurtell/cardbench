#!/usr/bin/env python3
"""Validate an Loxodon Hierarch report or score a candidate in a bounded worker."""

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
MATERIALIZER = Path(__file__).with_name("materialize_loxodon_hierarch_task.py")
TARGET = Path("varieties/magic/sets/ravnica_city_of_guilds/src/cards/loxodon_hierarch.rs")
AUTHORITY_SOURCE = Path("varieties/magic/session/src/bin/rav_loxodon_hierarch_pvp.rs")
DEFAULT_WORKER_IMAGE = "rust@sha256:d7482085ff5b415f84dba5647ae71606650bdef00db7aeb69f4b3d170c3e4082"
MAX_OUTPUT_BYTES = 2 * 1024 * 1024


class WorkerFailure(RuntimeError):
    pass


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def has_subsequence(actual: list[str], expected: list[str]) -> bool:
    cursor = iter(actual)
    return all(any(item == target for item in cursor) for target in expected)


def validate_report(report: dict) -> None:
    if (
        report.get("schema_version") != "cardbench.magic.card-pvp.check.v1"
        or report.get("task_id") != "cardbench/magic/card"
        or report.get("passed") is not True
        or report.get("pilot_card_passed") is not True
        or report.get("replay_deterministic") is not True
        or report.get("cards_passed") != 1
        or report.get("cards_credited") != 0
        or report.get("cards_total") != 291
        or report.get("printings_total") != 306
        or report.get("coverage_complete") is not False
        or report.get("full_reward") is not False
        or report.get("harbor_reward") != 0.0
        or report.get("pilot_reward") != 0.0
        or report.get("viewer_scope") != "Judge"
        or report.get("card") != {"collector_number": 214, "definition_id": "RAV-LOXODON-HIERARCH", "name": "Loxodon Hierarch"}
        or report.get("rules_authority", {}).get("comprehensive_rules") != "2026-08-07"
    ):
        raise WorkerFailure("candidate_protocol_error: incomplete Loxodon Hierarch single-card report")
    traces = report.get("traces")
    if not isinstance(traces, list) or len(traces) != 1 or not isinstance(traces[0], dict):
        raise WorkerFailure("candidate_protocol_error: one full-game Loxodon Hierarch trace is required")
    trace = traces[0]
    if (trace.get("id") != "rav-loxodon-hierarch-sacrifice-team-regeneration-full-game" or trace.get("passed") is not True
        or trace.get("terminal") != "Winner(PlayerId(0))" or trace.get("winner_seat") != 0 or trace.get("rejected_moves") != 0):
        raise WorkerFailure("candidate_protocol_error: Loxodon Hierarch full-game terminal trace did not pass")
    card = trace.get("loxodon_hierarch", {})
    paid = card.get("cast_mana_paid", [])
    if card.get("mana_cost") != "{2}{G}{W}" or len(paid) != 4 or paid.count("Green") != 2 or paid.count("White") != 2:
        raise WorkerFailure("candidate_protocol_error: Loxodon Hierarch must cast for exactly {2}{G}{W}")
    if (card.get("life_gained") != 4 or card.get("life_after_entry") != 24
        or card.get("trigger_stacks") != 1 or card.get("trigger_resolutions") != 1):
        raise WorkerFailure("candidate_protocol_error: Loxodon Hierarch must resolve exactly one mandatory four-life entry trigger")
    activation = trace.get("activation", {})
    if (activation.get("ability") != "sacrifice-source-regenerate-controller-creatures"
        or activation.get("mana_cost") != "{G}{W}"
        or sorted(activation.get("mana_abilities_after_char", [])) != ["Green", "White"]
        or sorted(activation.get("cost_paid_colors", [])) != ["Green", "White"]
        or activation.get("activated") != 1 or activation.get("resolved") != 1
        or activation.get("source_sacrificed") is not True):
        raise WorkerFailure("candidate_protocol_error: Loxodon Hierarch must pay {G}{W}, sacrifice itself, and resolve once")
    creatures = activation.get("creatures_at_resolution", [])
    shields = activation.get("shields_created", [])
    opposing = activation.get("opponent_creatures_at_resolution", [])
    if not creatures or sorted(creatures) != sorted(shields) or not opposing or activation.get("opponent_creature_shielded") is not False:
        raise WorkerFailure("candidate_protocol_error: regeneration shields must cover each controlled creature and no opponent creature")
    target = trace.get("target", {})
    if (target.get("definition_id") != "RAV-WATCHWOLF" or target.get("char_damage") != 4
        or target.get("shield_used") is not True or target.get("post_damage_zone") != "Some(Battlefield)"
        or target.get("post_damage_tapped") is not True or target.get("post_damage_marked_damage") != 0):
        raise WorkerFailure("candidate_protocol_error: lethal Char damage must consume the Watchwolf shield")
    if target.get("object") not in creatures:
        raise WorkerFailure("candidate_protocol_error: the shielded Char target was not controlled at ability resolution")
    opponent = trace.get("opponent", {})
    if opponent.get("watchwolf") not in opposing or opponent.get("char_mana_cost") != "{2}{R}{R}":
        raise WorkerFailure("candidate_protocol_error: opposing Watchwolf and Char evidence is missing")
    events = trace.get("events")
    if not isinstance(events, list) or not all(isinstance(event, dict) for event in events):
        raise WorkerFailure("candidate_protocol_error: projected event list is malformed")
    kinds = [event.get("kind") for event in events]
    if not has_subsequence(kinds, ["SpellManaPaid", "SpellCast", "TriggeredAbilityStacked", "LifeGained", "AbilityResolved", "SpellCast", "ManaAbilityActivated", "AbilityManaPaid", "CardMoved", "AbilityActivated", "RegenerationShieldCreated", "AbilityResolved", "DamageDealtToPermanent", "RegenerationShieldUsed", "GameEnded"]):
        raise WorkerFailure("candidate_protocol_error: Loxodon Hierarch trigger, paid activation, team shields, lethal damage, and terminal events are out of order")
    ordered = trace.get("ordered_event_sequence")
    if not isinstance(ordered, list) or any(not isinstance(item, int) for item in ordered) or ordered != sorted(ordered):
        raise WorkerFailure("candidate_protocol_error: required event receipt order is malformed")
    mutation = report.get("mutation_sensitivity", {})
    if (
        mutation.get("entry_trigger_removal_detected_in_game") is not True
        or mutation.get("wrong_life_gain_mutation_detected_in_game") is not True
        or mutation.get("wrong_activation_cost_mutation_detected_in_game") is not True
        or mutation.get("wrong_regeneration_scope_mutation_detected_in_game") is not True
        or mutation.get("activation_binding_removal_detected_in_game") is not True
        or mutation.get("parse_valid_side_effect_source_rejected_before_candidate_calls") is not True
    ):
        raise WorkerFailure("candidate_protocol_error: Loxodon Hierarch gameplay or source mutations were not detected")


def verify_workspace(workspace: Path, manifest: Path) -> None:
    subprocess.run([sys.executable, str(MATERIALIZER), "--workspace", str(workspace), "--manifest", str(manifest), "--verify"], check=True, cwd=REPO)


def snapshot(workspace: Path, files: dict[str, str], destination: Path) -> Path:
    if destination.is_symlink():
        raise WorkerFailure("candidate_snapshot_path_is_symlink")
    if destination.exists():
        shutil.rmtree(destination)
    destination.mkdir(parents=True)
    for relative in sorted(files):
        source = workspace / Path(relative)
        if source.is_symlink() or not source.is_file():
            raise WorkerFailure(f"candidate_snapshot_source_missing: {relative}")
        output = destination / Path(relative)
        output.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(source, output)
    return destination


def run_bounded(command: list[str], timeout: int, name: str) -> tuple[int, bytes, bytes]:
    process = subprocess.Popen(command, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    assert process.stdout is not None and process.stderr is not None
    selector = selectors.DefaultSelector()
    selector.register(process.stdout, selectors.EVENT_READ, "stdout")
    selector.register(process.stderr, selectors.EVENT_READ, "stderr")
    output, diagnostics = bytearray(), bytearray()
    started = time.monotonic()
    try:
        while selector.get_map() or process.poll() is None:
            if time.monotonic() - started > timeout:
                raise WorkerFailure("candidate_timeout: bounded Loxodon Hierarch worker timed out")
            for key, _ in selector.select(0.2):
                chunk = os.read(key.fileobj.fileno(), 65536)
                if not chunk:
                    selector.unregister(key.fileobj)
                    continue
                (output if key.data == "stdout" else diagnostics).extend(chunk)
                if len(output) + len(diagnostics) > MAX_OUTPUT_BYTES:
                    raise WorkerFailure("candidate_output_limit: worker output exceeded 2 MiB")
        return process.wait(), bytes(output), bytes(diagnostics)
    except BaseException:
        process.kill()
        process.wait()
        subprocess.run(["docker", "rm", "-f", name], capture_output=True, check=False)
        raise


def parse_report(stdout: bytes) -> dict:
    for line in reversed(stdout.decode(errors="replace").splitlines()):
        try:
            value = json.loads(line)
        except json.JSONDecodeError:
            continue
        if value.get("schema_version") == "cardbench.magic.card-pvp.check.v1":
            validate_report(value)
            return value
    raise WorkerFailure("candidate_protocol_error: no valid Loxodon Hierarch PvP result")


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
        "cp /trusted-authority.rs /work/varieties/magic/session/src/bin/rav_loxodon_hierarch_pvp.rs && "
        "cd /work && cargo run --offline --locked -j 2 --manifest-path varieties/magic/Cargo.toml "
        "-p cardbench-magic-session --bin rav-loxodon_hierarch-pvp --" + candidate_flag,
    ]


def score(workspace: Path, manifest: Path, output_root: Path, candidate_kind: str) -> int:
    output_root.mkdir(parents=True, exist_ok=True)
    manifest_data = json.loads(manifest.read_text(encoding="utf-8"))
    worker_image = os.environ.get("CARDBENCH_MAGIC_WORKER_IMAGE", DEFAULT_WORKER_IMAGE)
    if not (worker_image.startswith("rust@sha256:") or worker_image.startswith("sha256:")):
        raise ValueError("CARDBENCH_MAGIC_WORKER_IMAGE must be an immutable digest reference")
    registry = Path.home() / ".cargo" / "registry"
    if not registry.is_dir():
        raise FileNotFoundError(f"offline Cargo registry cache is missing: {registry}")
    authority = REPO / AUTHORITY_SOURCE
    if not shutil.which("docker"):
        raise FileNotFoundError("Docker is required for Magic candidate isolation")
    verify_workspace(workspace, manifest)
    scoring_workspace = snapshot(workspace, manifest_data["files"], output_root / "candidate-workspace")
    candidate_hash = sha256((scoring_workspace / TARGET).read_bytes())
    name = f"cardbench-magic-loxodon_hierarch-{uuid.uuid4().hex[:16]}"
    code, stdout, stderr = run_bounded(sandbox_command(scoring_workspace, registry.resolve(), authority, name, worker_image, candidate_kind), 900, name)
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
            "error": "candidate compile or Loxodon Hierarch reference trace failed",
            "diagnostics": stderr.decode(errors="replace")[-12000:],
        }
    else:
        result = parse_report(stdout)
        result["candidate"] = {"kind": candidate_kind, "card_definition": "RAV-LOXODON-HIERARCH", "source_sha256": candidate_hash, "materialized_workspace_sha256": manifest_data["workspace_sha256"]}
        result["candidate_kind"] = candidate_kind
        result["starting_workspace_sha256"] = manifest_data["workspace_sha256"]
        result["sealed_authority"] = False
        result["worker_image"] = worker_image
        result["sandbox"] = {"network": "none", "rootfs": "read-only", "pids_limit": 128}
        result["harbor_reward"] = 0.0
        result["pilot_reward"] = 0.0
    (output_root / "result.json").write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
    (output_root / "reward.txt").write_text(f"{result['harbor_reward']:.8f}\n", encoding="utf-8")
    return 0 if result.get("passed") is True and result.get("pilot_card_passed") is True else 1


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--workspace", type=Path)
    parser.add_argument("--manifest", type=Path)
    parser.add_argument("--output-root", type=Path)
    parser.add_argument("--candidate-kind", choices=["agent", "reference"], default="agent")
    parser.add_argument("--validate-report", type=Path)
    args = parser.parse_args()
    if args.validate_report:
        validate_report(json.loads(args.validate_report.read_text(encoding="utf-8")))
        print("Loxodon Hierarch report validates; rewards and credit remain zero")
        return 0
    if not args.workspace or not args.manifest or not args.output_root:
        parser.error("--workspace, --manifest, and --output-root are required unless --validate-report is used")
    try:
        return score(args.workspace.resolve(), args.manifest.resolve(), args.output_root.resolve(), args.candidate_kind)
    except Exception as error:
        args.output_root.mkdir(parents=True, exist_ok=True)
        result = {"schema_version": "cardbench.magic.card-pvp.check.v1", "task_id": "cardbench/magic/card", "passed": False, "pilot_card_passed": False, "cards_passed": 0, "cards_total": 291, "printings_total": 306, "cards_credited": 0, "coverage_complete": False, "full_reward": False, "harbor_reward": 0.0, "pilot_reward": 0.0, "sealed_authority": False, "error": str(error)}
        (args.output_root / "result.json").write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
        (args.output_root / "reward.txt").write_text("0.0\n", encoding="utf-8")
        print(f"Magic candidate verifier failed: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
