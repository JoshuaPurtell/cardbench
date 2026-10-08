#!/usr/bin/env python3
"""Validate the Trophy Hunter authority report and isolate candidate scoring."""

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

from materialize_trophy_hunter_task import verify as verify_workspace

REPO = Path(__file__).resolve().parents[3]
TARGET = Path("varieties/magic/sets/ravnica_city_of_guilds/src/cards/trophy_hunter.rs")
AUTHORITY = Path("varieties/magic/session/src/bin/rav_trophy_hunter_task_pvp.rs")
MATERIALIZER = Path(__file__).with_name("materialize_trophy_hunter_task.py")
DEFAULT_WORKER_IMAGE = "rust@sha256:d7482085ff5b415f84dba5647ae71606650bdef00db7aeb69f4b3d170c3e4082"
MAX_OUTPUT_BYTES = 2 * 1024 * 1024


class WorkerFailure(RuntimeError):
    pass


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
        or report.get("card") != {"collector_number": 187, "definition_id": "RAV-TROPHY-HUNTER", "name": "Trophy Hunter"}
    ):
        raise WorkerFailure("candidate_protocol_error: Trophy Hunter identity/reward fields are invalid")
    candidate = report.get("candidate")
    traces = report.get("traces")
    if not isinstance(candidate, dict) or candidate.get("card_definition") != "RAV-TROPHY-HUNTER":
        raise WorkerFailure("candidate_protocol_error: wrong Trophy Hunter candidate identity")
    if not isinstance(traces, list) or len(traces) != 1:
        raise WorkerFailure("candidate_protocol_error: exactly one Trophy Hunter full-game trace is required")
    trace = traces[0]
    mechanics = trace.get("mechanics")
    events = trace.get("events")
    if not isinstance(mechanics, dict) or not isinstance(events, list):
        raise WorkerFailure("candidate_protocol_error: Trophy Hunter mechanics/events are missing")
    mutations = report.get("mutation_sensitivity")
    source_validation = report.get("candidate_source_validation")
    required_mutations = (
        "activated_damage_removed_detected",
        "activation_tap_cost_removed_detected",
        "wrong_trigger_condition_detected",
        "trigger_counter_removed_detected",
        "wrong_activated_cost_detected",
        "wrong_base_power_detected",
    )
    event_kinds = [event.get("kind") for event in events if isinstance(event, dict)]
    required_kinds = {
        "SpellManaPaid", "SpellCast", "SpellResolved", "CardMoved", "AbilityManaPaid",
        "AbilityActivated", "DamageDealtToPermanent", "TriggeredAbilityStacked",
        "CounterPlaced", "AbilityResolved", "AttackersDeclared", "GameEnded",
    }
    if (
        trace.get("id") != "rav-trophy-hunter-source-aware-death-counter-pvp"
        or trace.get("terminal") != "Winner(PlayerId(0))"
        or trace.get("winner_seat") != 0
        or trace.get("rejected_moves") != 0
        or not isinstance(trace.get("seed"), int)
        or mechanics.get("hunter_mana_cost") != "{2}{G}"
        or mechanics.get("hunter_base_characteristics") != {"power": 2, "toughness": 3}
        or mechanics.get("hunter_characteristics_after_trigger") != {"power": 3, "toughness": 4}
        or mechanics.get("activated_mana_cost") != "{1}{G}"
        or mechanics.get("activated_mana_paid") != "{1}{G}"
        or mechanics.get("activated_tap_cost_paid") is not True
        or mechanics.get("damage_amount") != 1
        or mechanics.get("target_zone_after_damage") != "Graveyard"
        or mechanics.get("trigger_id") != "damaged-creature-dies-add-plus-one-counter"
        or mechanics.get("trigger_stacked") is not True
        or mechanics.get("trigger_resolved") is not True
        or mechanics.get("source_plus_one_counter_total") != 1
        or not isinstance(mechanics.get("hunter_object"), int)
        or not isinstance(mechanics.get("target_object"), int)
        or mechanics.get("hunter_object") == mechanics.get("target_object")
        or mechanics.get("activated_source_incarnation") != mechanics.get("trigger_source_incarnation")
        or not isinstance(mechanics.get("target_incarnation_at_activation"), int)
        or not isinstance(mechanics.get("target_post_move_incarnation"), int)
        or mechanics.get("target_incarnation_at_activation") == mechanics.get("target_post_move_incarnation")
        or not isinstance(mechanics.get("ordered_event_sequence"), list)
        or len(mechanics["ordered_event_sequence"]) != len(events)
        or mechanics["ordered_event_sequence"] != sorted(mechanics["ordered_event_sequence"])
        or not required_kinds.issubset(set(event_kinds))
        or not isinstance(mutations, dict)
        or any(mutations.get(key) is not True for key in required_mutations)
        or not isinstance(source_validation, dict)
        or any(source_validation.get(key) is not True for key in (
            "normalized_literal_source_contract_matched",
            "ast_literal_shape_matched",
            "compile_valid_injected_side_effect_fixture_compiled",
            "compile_valid_injected_side_effect_candidate_rejected",
            "guard_ran_before_candidate_function_calls",
        ))
    ):
        raise WorkerFailure("candidate_protocol_error: Trophy Hunter cost, committed damage, exact-incarnation death trigger, counter, or terminal trace is incomplete")
    rules = report.get("rules_authority")
    if not isinstance(rules, dict) or rules.get("comprehensive_rules") != "2026-08-07" or rules.get("no_provider_trial") is not True:
        raise WorkerFailure("candidate_protocol_error: pinned rules reference or no-provider statement is missing")


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


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
        "cp -R /submission/. /work/ && cp /trusted-authority.rs /work/varieties/magic/session/src/bin/rav_trophy_hunter_task_pvp.rs && "
        "cd /work && cargo run --offline --locked -j 2 --manifest-path varieties/magic/Cargo.toml "
        "-p cardbench-magic-session --bin rav-trophy-hunter-task-pvp --" + candidate_flag,
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
    authority = REPO / AUTHORITY
    if authority.is_symlink() or not authority.is_file():
        raise FileNotFoundError(f"trusted Trophy Hunter authority is missing: {authority}")
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
    name = f"cardbench-magic-trophy-hunter-{uuid.uuid4().hex[:16]}"
    command = sandbox_command(scoring_workspace, cargo_registry.resolve(), authority, name, worker_image, candidate_kind)
    code, stdout, stderr = run_bounded(command, timeout=900, name=name)
    (output_root / "worker.stdout").write_bytes(stdout)
    (output_root / "worker.stderr").write_bytes(stderr)
    if code == 0:
        result = None
        for line in reversed(stdout.decode(errors="replace").splitlines()):
            try:
                parsed = json.loads(line)
            except json.JSONDecodeError:
                continue
            if parsed.get("schema_version") == "cardbench.magic.card-pvp.check.v1":
                validate_report(parsed)
                result = parsed
                break
        if result is None:
            raise WorkerFailure("candidate_protocol_error: no Trophy Hunter PvP report")
        result["candidate"] = {"kind": candidate_kind, "card_definition": "RAV-TROPHY-HUNTER", "source_sha256": candidate_hash, "materialized_workspace_sha256": manifest_data["workspace_sha256"]}
        result["candidate_kind"] = candidate_kind
        result["starting_workspace_sha256"] = manifest_data["workspace_sha256"]
    else:
        result = {"schema_version": "cardbench.magic.card-pvp.check.v1", "task_id": "cardbench/magic/card", "passed": False, "pilot_card_passed": False, "cards_passed": 0, "cards_total": 291, "printings_total": 306, "cards_credited": 0, "coverage_complete": False, "full_reward": False, "harbor_reward": 0.0, "pilot_reward": 0.0, "error": "candidate compilation or Trophy Hunter full-game trace failed", "diagnostics": stderr.decode(errors="replace")[-12000:]}
    result.update({"cards_credited": 0, "coverage_complete": False, "full_reward": False, "harbor_reward": 0.0, "pilot_reward": 0.0, "sealed_authority": False})
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
            output = args.output_root.resolve()
            output.mkdir(parents=True, exist_ok=True)
            failure = {"schema_version": "cardbench.magic.card-pvp.check.v1", "task_id": "cardbench/magic/card", "passed": False, "pilot_card_passed": False, "cards_passed": 0, "cards_total": 291, "printings_total": 306, "cards_credited": 0, "coverage_complete": False, "full_reward": False, "harbor_reward": 0.0, "pilot_reward": 0.0, "error": str(error)}
            (output / "result.json").write_text(json.dumps(failure, indent=2) + "\n", encoding="utf-8")
            (output / "reward.txt").write_text("0.00000000\n", encoding="utf-8")
        print(f"Trophy Hunter verifier failed: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
