#!/usr/bin/env python3
"""Validate a Tidewater Minion report or run an isolated candidate worker."""

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
MATERIALIZER = Path(__file__).with_name("materialize_tidewater_minion_task.py")
TARGET = Path("varieties/magic/sets/ravnica_city_of_guilds/src/cards/tidewater_minion.rs")
AUTHORITY_SOURCE = Path("varieties/magic/session/src/bin/rav_tidewater_minion_pvp.rs")
DEFAULT_WORKER_IMAGE = "rust@sha256:d7482085ff5b415f84dba5647ae71606650bdef00db7aeb69f4b3d170c3e4082"
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
        or report.get("card") != {
            "collector_number": 71,
            "definition_id": "RAV-TIDEWATER-MINION",
            "name": "Tidewater Minion",
        }
        or report.get("rules_authority", {}).get("comprehensive_rules") != "2026-08-07"
    ):
        raise WorkerFailure("candidate_protocol_error: incomplete Tidewater Minion report")
    traces = report.get("traces")
    if not isinstance(traces, list) or len(traces) != 1 or not isinstance(traces[0], dict):
        raise WorkerFailure("candidate_protocol_error: one full-game Tidewater Minion trace is required")
    trace = traces[0]
    if (
        trace.get("id") != "rav-tidewater-minion-two-abilities-full-game"
        or trace.get("terminal") != "Winner(PlayerId(0))"
        or trace.get("winner_seat") != 0
        or trace.get("passed") is not True
        or trace.get("rejected_moves") != 0
    ):
        raise WorkerFailure("candidate_protocol_error: Tidewater Minion did not finish its full-game trace")
    mechanics = trace.get("mechanics", {})
    cast_colors = mechanics.get("cast_payment_colors", [])
    untap = mechanics.get("tap_untap", {})
    lose_defender = mechanics.get("lose_defender", {})
    combat = mechanics.get("combat", {})
    life = combat.get("life_at_terminal", [])
    if (
        mechanics.get("definition_id") != "RAV-TIDEWATER-MINION"
        or mechanics.get("mana_cost") != "{3}{U}{U}"
        or len(cast_colors) != 5
        or cast_colors.count("Blue") != 5
        or mechanics.get("base_power") != 4
        or mechanics.get("base_toughness") != 4
        or mechanics.get("printed_subtypes") != ["Elemental", "Minion"]
        or mechanics.get("printed_subtypes_asserted_at_runtime") is not False
        or mechanics.get("starting_keywords") != ["Defender"]
        or untap.get("ability") != "tap-untap-target-permanent"
        or untap.get("cost") != "{T}"
        or untap.get("mana_cost") != "{0}"
        or untap.get("target_requirement") != "Permanent"
        or untap.get("target_definition_id") != "RAV-BOROS-SIGNET"
        or untap.get("target_controller") != 1
        or untap.get("target_controlled_by_opponent") is not True
        or untap.get("target_was_tapped") is not True
        or untap.get("target_seen_on_stack") is not True
        or untap.get("target_untapped") is not True
        or untap.get("activation_count") != 1
        or untap.get("resolution_count") != 1
        or untap.get("source_tapped_for_cost") is not True
        or lose_defender.get("ability") != "lose-defender-until-end-of-turn"
        or lose_defender.get("cost") != "{4}"
        or lose_defender.get("paid_generic_cost") != "{4}"
        or lose_defender.get("paid_activations", 0) < 5
        or lose_defender.get("resolutions", 0) < 5
        or lose_defender.get("removed_live") is not True
        or lose_defender.get("source_untapped_after_resolution") is not True
        or lose_defender.get("effect_expired") is not True
        or lose_defender.get("defender_restored_after_expiration") is not True
        or combat.get("minion_attack_count", 0) < 5
        or combat.get("four_damage_count", 0) < 5
        or combat.get("terminal_power") != 4
        or combat.get("terminal_toughness") != 4
        or not isinstance(life, list)
        or len(life) != 2
        or life[0] <= 0
        or life[1] != 0
        or mechanics.get("terminal_pilot_win") is not True
    ):
        raise WorkerFailure("candidate_protocol_error: Tidewater printed abilities or game evidence differs")
    ordered = trace.get("ordered_event_sequence")
    if not isinstance(ordered, list) or not all(isinstance(item, int) for item in ordered) or ordered != sorted(ordered):
        raise WorkerFailure("candidate_protocol_error: Tidewater event order is malformed")
    events = trace.get("events")
    if not isinstance(events, list) or not all(isinstance(event, dict) for event in events):
        raise WorkerFailure("candidate_protocol_error: projected Tidewater events are malformed")
    kinds = [event.get("kind") for event in events]
    for required in (
        "SpellManaPaid",
        "SpellCast",
        "SpellResolved",
        "CardMoved",
        "BoundManaAbilityBundleActivated",
        "AbilityActivated",
        "PermanentUntapped",
        "AbilityManaPaid",
        "AbilityResolved",
        "ContinuousEffectCreated",
        "ContinuousEffectExpired",
        "AttackersDeclared",
        "DamageDealtToPlayer",
        "GameEnded",
    ):
        if required not in kinds:
            raise WorkerFailure(f"candidate_protocol_error: missing Tidewater event {required}")
    guard = report.get("source_guard", {})
    if (
        guard.get("side_effect_mutation_parse_valid") is not True
        or guard.get("side_effect_mutation_compile_valid_by_construction") is not True
        or guard.get("side_effect_mutation_rejected") is not True
        or guard.get("candidate_calls_before_guard") != 0
    ):
        raise WorkerFailure("candidate_protocol_error: side-effect candidate was not rejected before invocation")
    mutations = report.get("mutation_sensitivity", {})
    for key in (
        "printed_mana_cost_mutation_detected",
        "base_power_mutation_detected",
        "untap_target_scope_mutation_detected",
        "untap_effect_mutation_detected",
        "defender_ability_cost_mutation_detected",
        "defender_effect_mutation_detected",
        "untap_binding_removal_detected",
        "defender_binding_removal_detected",
        "compile_valid_side_effect_source_rejected_before_candidate_calls",
    ):
        if mutations.get(key) is not True:
            raise WorkerFailure(f"candidate_protocol_error: Tidewater mutation escaped: {key}")


def verify_workspace(workspace: Path, manifest: Path) -> None:
    subprocess.run(
        [sys.executable, str(MATERIALIZER), "--workspace", str(workspace), "--manifest", str(manifest), "--verify"],
        check=True,
        cwd=REPO,
    )


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
                raise WorkerFailure("candidate_timeout: bounded Tidewater Minion worker timed out")
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
    raise WorkerFailure("candidate_protocol_error: no valid Tidewater Minion PvP result")


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
        "cp /trusted-authority.rs /work/varieties/magic/session/src/bin/rav_tidewater_minion_pvp.rs && "
        "cd /work && cargo run --offline --locked -j 2 --manifest-path varieties/magic/Cargo.toml "
        "-p cardbench-magic-session --bin rav-tidewater-minion-pvp --" + candidate_flag,
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
    name = f"cardbench-magic-tidewater-minion-{uuid.uuid4().hex[:16]}"
    code, stdout, stderr = run_bounded(
        sandbox_command(scoring_workspace, registry.resolve(), authority, name, worker_image, candidate_kind),
        1200,
        name,
    )
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
            "error": "candidate compile or Tidewater Minion reference trace failed",
            "diagnostics": stderr.decode(errors="replace")[-12000:],
        }
    else:
        result = parse_report(stdout)
        result["candidate"] = {
            "kind": candidate_kind,
            "card_definition": "RAV-TIDEWATER-MINION",
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
        print("Tidewater Minion report validates; rewards and credit remain zero")
        return 0
    if not args.workspace or not args.manifest or not args.output_root:
        parser.error("--workspace, --manifest, and --output-root are required unless --validate-report is used")
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
