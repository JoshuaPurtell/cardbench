#!/usr/bin/env python3
"""Validate and, when invoked by Harbor, isolate a Coalhauler Swine candidate."""

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
MATERIALIZER = Path(__file__).with_name("materialize_coalhauler_swine_task.py")
TARGET = Path("varieties/magic/sets/ravnica_city_of_guilds/src/cards/coalhauler_swine.rs")
AUTHORITY_SOURCE = Path("varieties/magic/session/src/bin/rav_coalhauler_swine_pvp.rs")
DEFAULT_WORKER_IMAGE = "rust@sha256:d7482085ff5b415f84dba5647ae71606650bdef00db7aeb69f4b3d1703c4082"
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
        or report.get("card") != {"collector_number": 119, "definition_id": "RAV-COALHAULER-SWINE", "name": "Coalhauler Swine"}
    ):
        raise WorkerFailure("candidate_protocol_error: Coalhauler report identity/reward fields are invalid")
    candidate = report.get("candidate")
    if not isinstance(candidate, dict) or candidate.get("card_definition") != "RAV-COALHAULER-SWINE":
        raise WorkerFailure("candidate_protocol_error: wrong Coalhauler candidate identity")
    traces = report.get("traces")
    if not isinstance(traces, list) or len(traces) != 1:
        raise WorkerFailure("candidate_protocol_error: exactly one Coalhauler full-game trace is required")
    trace = traces[0]
    mechanics = trace.get("mechanics")
    events = trace.get("events")
    if not isinstance(mechanics, dict) or not isinstance(events, list):
        raise WorkerFailure("candidate_protocol_error: Coalhauler mechanics/events are missing")
    swine_payment = mechanics.get("swine_mana_paid")
    fangtail_payment = mechanics.get("fangtail_mana_paid")
    sequence = mechanics.get("ordered_event_sequence")
    player_damage = mechanics.get("player_damage")
    mutations = report.get("mutation_sensitivity")
    required_mutations = (
        "trigger_removed_detected_in_game",
        "wrong_received_damage_condition_detected_in_game",
        "fixed_damage_amount_detected_in_game",
        "wrong_mana_cost_detected_in_game",
        "wrong_base_stats_detected_in_game",
    )
    if (
        trace.get("id") != "rav-coalhauler-swine-received-damage-to-each-player-pvp"
        or trace.get("terminal") != "Winner(PlayerId(0))"
        or trace.get("winner_seat") != 0
        or trace.get("rejected_moves") != 0
        or not isinstance(trace.get("seed"), int)
        or mechanics.get("swine_mana_cost") != "{4}{R}{R}"
        or mechanics.get("swine_characteristics") != {"power": 4, "toughness": 4}
        or not isinstance(mechanics.get("swine_object"), int)
        or not isinstance(swine_payment, list)
        or len(swine_payment) != 6
        or swine_payment.count("Red") < 2
        or not isinstance(fangtail_payment, list)
        or len(fangtail_payment) != 4
        or fangtail_payment.count("Red") < 2
        or mechanics.get("damage_source_controller_seat") != 1
        or mechanics.get("received_damage_amount") != 1
        or mechanics.get("trigger_id") != "damage-each-player"
        or mechanics.get("trigger_controller_seat") != 0
        or mechanics.get("trigger_stacked") is not True
        or mechanics.get("trigger_resolved") is not True
        or mechanics.get("life_totals_before_trigger") != [20, 20]
        or mechanics.get("life_totals_after_trigger") != [19, 19]
        or player_damage != [{"seat": 0, "amount": 1}, {"seat": 1, "amount": 1}]
        or not isinstance(sequence, list)
        or len(sequence) != 16
        or any(not isinstance(item, int) for item in sequence)
        or sequence != sorted(sequence)
        or [event.get("kind") for event in events if isinstance(event, dict)] != [
            "SpellManaPaid", "SpellCast", "SpellResolved", "CardMoved",
            "SpellManaPaid", "SpellCast", "SpellResolved", "CardMoved",
            "AbilityActivated", "DamageDealtToPermanent", "TriggeredAbilityStacked",
            "DamageDealtToPlayer", "DamageDealtToPlayer", "AbilityResolved",
            "AttackersDeclared", "GameEnded",
        ]
        or not isinstance(mutations, dict)
        or any(mutations.get(key) is not True for key in required_mutations)
    ):
        raise WorkerFailure("candidate_protocol_error: Coalhauler cost, damage provenance, per-player damage, or terminal trace is incomplete")
    swine = mechanics["swine_object"]
    source = mechanics.get("damage_source_object")
    if not isinstance(source, int) or mechanics.get("trigger_source_incarnation") is None:
        raise WorkerFailure("candidate_protocol_error: exact damage source or Swine incarnation is missing")
    facts = [event.get("facts", {}) for event in events]
    if (
        facts[0].get("objects") != [source] or facts[0].get("seats") != [1] or facts[0].get("amount") != 4
        or facts[0].get("mana_colors") != fangtail_payment
        or facts[1].get("objects") != [source] or facts[1].get("seats") != [1]
        or facts[2].get("objects") != [source] or facts[3].get("objects") != [source] or facts[3].get("zone") != "Battlefield"
        or facts[4].get("objects") != [swine] or facts[4].get("seats") != [0] or facts[4].get("amount") != 6
        or facts[4].get("mana_colors") != swine_payment
        or facts[5].get("objects") != [swine] or facts[5].get("seats") != [0]
        or facts[6].get("objects") != [swine] or facts[7].get("objects") != [swine] or facts[7].get("zone") != "Battlefield"
        or facts[8].get("objects") != [source] or facts[8].get("seats") != [1]
        or facts[9].get("objects") != [source, swine] or facts[9].get("amount") != 1
        or facts[10].get("objects") != [swine] or facts[10].get("seats") != [0]
        or facts[11].get("objects") != [swine] or facts[11].get("seats") != [0] or facts[11].get("amount") != 1
        or facts[12].get("objects") != [swine] or facts[12].get("seats") != [1] or facts[12].get("amount") != 1
        or facts[13].get("objects") != [swine]
        or facts[14].get("seats") != [0] or swine not in facts[14].get("objects", [])
        or facts[15].get("seats") != [0]
    ):
        raise WorkerFailure("candidate_protocol_error: Coalhauler payment, trigger source, or per-player damage receipts differ")
    event_sequences = [event.get("sequence") for event in events]
    if any(not isinstance(value, int) for value in event_sequences) or event_sequences != sorted(event_sequences):
        raise WorkerFailure("candidate_protocol_error: projected Coalhauler event sequence is malformed")
    rules = report.get("rules_authority")
    if not isinstance(rules, dict) or rules.get("comprehensive_rules") != "2026-08-07":
        raise WorkerFailure("candidate_protocol_error: pinned Comprehensive Rules snapshot is missing")


def parse_report(stdout: bytes) -> dict:
    for line in reversed(stdout.decode(errors="replace").splitlines()):
        try:
            report = json.loads(line)
        except json.JSONDecodeError:
            continue
        if report.get("schema_version") == "cardbench.magic.card-pvp.check.v1":
            validate_report(report)
            return report
    raise WorkerFailure("candidate_protocol_error: no valid Coalhauler PvP report")


def run_bounded(command: list[str], *, timeout: int, name: str) -> tuple[int, bytes, bytes]:
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
                raise WorkerFailure("candidate_timeout: isolated Docker worker exceeded its deadline")
            for key, _ in selector.select(0.2):
                chunk = os.read(key.fileobj.fileno(), 65536)
                if not chunk:
                    selector.unregister(key.fileobj)
                    continue
                target = output if key.data == "stdout" else diagnostics
                target.extend(chunk)
                if len(output) + len(diagnostics) > MAX_OUTPUT_BYTES:
                    raise WorkerFailure("candidate_output_limit: worker output exceeded 2 MiB")
        return process.wait(), bytes(output), bytes(diagnostics)
    except BaseException:
        process.kill()
        process.wait()
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
        "cp -R /submission/. /work/ && cp /trusted-authority.rs /work/varieties/magic/session/src/bin/rav_coalhauler_swine_pvp.rs && "
        "cd /work && cargo run --offline --locked -j 2 --manifest-path varieties/magic/Cargo.toml "
        "-p cardbench-magic-session --bin rav-coalhauler-swine-pvp --" + candidate_flag,
    ]


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
        raise FileNotFoundError(f"trusted Coalhauler Swine authority is missing: {authority}")
    if not shutil.which("docker"):
        raise FileNotFoundError("Docker is required for isolated Magic candidate scoring")
    verify_workspace(workspace, manifest)
    scoring_workspace = output_root / "candidate-workspace"
    if scoring_workspace.is_symlink():
        raise WorkerFailure("candidate_snapshot_path_is_symlink")
    if scoring_workspace.exists():
        shutil.rmtree(scoring_workspace)
    scoring_workspace.mkdir(parents=True)
    for relative in sorted(manifest_data["files"]):
        source = workspace / Path(relative)
        if source.is_symlink() or not source.is_file():
            raise WorkerFailure(f"candidate_snapshot_source_missing: {relative}")
        destination = scoring_workspace / Path(relative)
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(source, destination)
    candidate_hash = sha256((scoring_workspace / TARGET).read_bytes())
    name = f"cardbench-magic-coalhauler-swine-{uuid.uuid4().hex[:16]}"
    command = sandbox_command(scoring_workspace, cargo_registry.resolve(), authority, name, worker_image, candidate_kind)
    code, stdout, stderr = run_bounded(command, timeout=900, name=name)
    (output_root / "worker.stdout").write_bytes(stdout)
    (output_root / "worker.stderr").write_bytes(stderr)
    if code == 0:
        result = parse_report(stdout)
        result["candidate"] = {"kind": candidate_kind, "card_definition": "RAV-COALHAULER-SWINE", "source_sha256": candidate_hash, "materialized_workspace_sha256": manifest_data["workspace_sha256"]}
        result["candidate_kind"] = candidate_kind
        result["starting_workspace_sha256"] = manifest_data["workspace_sha256"]
    else:
        result = {"schema_version": "cardbench.magic.card-pvp.check.v1", "task_id": "cardbench/magic/card", "passed": False, "pilot_card_passed": False, "cards_passed": 0, "cards_total": 291, "printings_total": 306, "cards_credited": 0, "coverage_complete": False, "full_reward": False, "harbor_reward": 0.0, "pilot_reward": 0.0, "error": "candidate compilation or Coalhauler full-game trace failed", "diagnostics": stderr.decode(errors="replace")[-12000:]}
    result["cards_credited"] = 0
    result["coverage_complete"] = False
    result["full_reward"] = False
    result["harbor_reward"] = 0.0
    result["pilot_reward"] = 0.0
    result["sealed_authority"] = False
    (output_root / "result.json").write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
    (output_root / "reward.txt").write_text("0.00000000\n", encoding="utf-8")
    print(f"candidate_passed={result.get('pilot_card_passed', False)} cards={result.get('cards_passed', 0)}/291 reward=0.0 credited=0")
    return 0 if result.get("passed") is True and result.get("pilot_card_passed") is True else 1


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--workspace", type=Path)
    parser.add_argument("--manifest", type=Path)
    parser.add_argument("--output-root", type=Path)
    parser.add_argument("--candidate-kind", choices=["agent", "reference"], default="agent")
    parser.add_argument("--validate-report", type=Path)
    args = parser.parse_args()
    try:
        if args.validate_report:
            validate_report(json.loads(args.validate_report.read_text(encoding="utf-8")))
            print("report_valid=true pilot_reward=0.0 harbor_reward=0.0 cards_credited=0")
            return 0
        if not args.workspace or not args.manifest or not args.output_root:
            parser.error("--workspace, --manifest and --output-root are required unless --validate-report is used")
        return score(args.workspace.resolve(), args.manifest.resolve(), args.output_root.resolve(), args.candidate_kind)
    except Exception as error:
        if args.output_root:
            output_root = args.output_root.resolve()
            output_root.mkdir(parents=True, exist_ok=True)
            failure = {
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
                "error": str(error),
            }
            (output_root / "result.json").write_text(json.dumps(failure, indent=2) + "\n", encoding="utf-8")
            (output_root / "reward.txt").write_text("0.00000000\n", encoding="utf-8")
        print(f"Coalhauler Swine verifier failed: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
