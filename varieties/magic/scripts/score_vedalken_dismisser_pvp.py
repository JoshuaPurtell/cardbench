#!/usr/bin/env python3
"""Run the Vedalken Dismisser reference authority in a bounded offline worker."""

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
MATERIALIZER = Path(__file__).with_name("materialize_vedalken_dismisser_task.py")
TARGET = Path("varieties/magic/sets/ravnica_city_of_guilds/src/cards/vedalken_dismisser.rs")
AUTHORITY_SOURCE = Path("varieties/magic/session/src/bin/rav_vedalken_dismisser_pvp.rs")
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
        "cp /trusted-authority.rs /work/varieties/magic/session/src/bin/rav_vedalken_dismisser_pvp.rs && "
        "cd /work && cargo run --offline --locked -j 2 --manifest-path varieties/magic/Cargo.toml "
        "-p cardbench-magic-session --bin rav-vedalken-dismisser-pvp --" + candidate_flag,
    ]


def validate_report(report: dict) -> None:
    expected_card = {
        "collector_number": 73,
        "definition_id": "RAV-VEDALKEN-DISMISSER",
        "name": "Vedalken Dismisser",
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
        raise WorkerFailure("candidate_protocol_error: incomplete Vedalken Dismisser single-card report")
    traces = report.get("traces")
    if not isinstance(traces, list) or len(traces) != 1:
        raise WorkerFailure("candidate_protocol_error: exactly one full-game Vedalken Dismisser trace is required")
    trace = traces[0]
    if (
        not isinstance(trace, dict)
        or trace.get("trace_id") != "rav-vedalken-dismisser-etb-opposing-creature-owner-library-top-full-game"
        or trace.get("passed") is not True
        or trace.get("terminal") != "Winner(PlayerId(0))"
        or trace.get("winner_seat") != 0
        or not isinstance(trace.get("turns"), int)
        or trace["turns"] <= 0
        or trace.get("rejected_moves") != 0
    ):
        raise WorkerFailure("candidate_protocol_error: Vedalken Dismisser duel did not reach its expected terminal win")
    dismisser = trace.get("dismisser")
    trigger = trace.get("trigger")
    combat = trace.get("combat")
    if (
        not isinstance(dismisser, dict)
        or dismisser.get("collector_number") != 73
        or dismisser.get("mana_cost") != "{5}{U}"
        or not isinstance(dismisser.get("mana_paid"), list)
        or len(dismisser["mana_paid"]) != 6
        or "Blue" not in dismisser["mana_paid"]
        or dismisser.get("power") != 2
        or dismisser.get("toughness") != 2
        or dismisser.get("resolved") is not True
        or not isinstance(trigger, dict)
        or trigger.get("ability_id") != "etb-target-creature-owner-library-top"
        or trigger.get("mandatory") is not True
        or trigger.get("controller") != 0
        or trigger.get("decision_player") != 0
        or trigger.get("source_object") != dismisser.get("object")
        or trigger.get("source_incarnation") != trigger.get("resolved_source_incarnation")
        or trigger.get("target_controller") != 1
        or trigger.get("target_owner") != 1
        or trigger.get("target_zone_after") != "Some(Library)"
        or trigger.get("target_is_library_top") is not True
        or trigger.get("owner_library_size_after") != trigger.get("owner_library_size_before", -2) + 1
        or trigger.get("owner_library_top_object") != trigger.get("target_object")
        or trigger.get("target_incarnation_before") == trigger.get("target_incarnation_after")
        or not isinstance(trigger.get("target_object"), int)
        or trigger.get("target_object") == dismisser.get("object")
        or not isinstance(combat, dict)
        or combat.get("card") != "Watchwolf"
        or not isinstance(combat.get("object"), int)
        or len(combat.get("attack_turns", [])) < 3
        or combat.get("damage_to_opponent", 0) < 9
        or not isinstance(combat.get("life_at_terminal"), list)
        or len(combat["life_at_terminal"]) != 2
        or combat["life_at_terminal"][0] <= 0
        or combat["life_at_terminal"][1] > 0
    ):
        raise WorkerFailure("candidate_protocol_error: exact Dismisser body, mandatory target choice, owner library move, or combat continuation is missing")
    events = trace.get("events")
    order = trace.get("ordered_event_sequence")
    if not isinstance(events, list) or not isinstance(order, list) or len(order) != 13 or len(events) != 13:
        raise WorkerFailure("candidate_protocol_error: ordered Vedalken Dismisser event receipts are missing")
    sequences = [event.get("sequence") for event in events]
    if any(not isinstance(value, int) for value in sequences) or sequences != sorted(sequences):
        raise WorkerFailure("candidate_protocol_error: Vedalken Dismisser event sequence is malformed")
    if any(not isinstance(value, int) for value in order) or order != sequences:
        raise WorkerFailure("candidate_protocol_error: semantic event order differs from the complete ordered receipt sequence")
    expected_kinds = [
        "SpellManaPaid", "SpellCast", "SpellResolved", "CardMoved", "DecisionOpened",
        "DecisionCompleted", "TriggeredAbilityStacked", "CardMoved", "AbilityResolved",
        "SpellCast", "AttackersDeclared", "DamageDealtToPlayer", "GameEnded",
    ]
    if [event.get("kind") for event in events] != expected_kinds:
        raise WorkerFailure("candidate_protocol_error: Dismisser spell, target choice, trigger, and combat event order differs")
    source = dismisser["object"]
    target = trigger["target_object"]
    finisher = combat["object"]
    canonical = [event.get("canonical", "") for event in events]
    if not (
        f"card: ObjectId({source})" in canonical[0] and "PlayerId(0)" in canonical[0]
        and f"card: ObjectId({source})" in canonical[1]
        and f"card: ObjectId({source})" in canonical[2]
        and f"card: ObjectId({source})" in canonical[3] and "to: Battlefield" in canonical[3]
        and "DecisionId(" in canonical[4] and "PlayerId(0)" in canonical[4] and "TriggeredAbilityTargets" in canonical[4]
        and "DecisionId(" in canonical[5] and "PlayerId(0)" in canonical[5] and "TriggeredAbilityTargets" in canonical[5]
        and canonical[4].split("DecisionId(",1)[1].split(")",1)[0] == canonical[5].split("DecisionId(",1)[1].split(")",1)[0]
        and f"source: ObjectId({source})" in canonical[6] and 'ability: "etb-target-creature-owner-library-top"' in canonical[6]
        and f"card: ObjectId({target})" in canonical[7] and "to: Library" in canonical[7]
        and f"source: ObjectId({source})" in canonical[8] and 'ability: "etb-target-creature-owner-library-top"' in canonical[8]
        and f"card: ObjectId({finisher})" in canonical[9]
        and f"ObjectId({finisher})" in canonical[10]
        and f"source: ObjectId({finisher})" in canonical[11] and "PlayerId(1)" in canonical[11]
        and "Some(PlayerId(0))" in canonical[12]
    ):
        raise WorkerFailure("candidate_protocol_error: event receipts do not identify exact Dismisser, target, and finisher objects")
    mutations = report.get("mutation_sensitivity")
    required_mutations = (
        "etb_trigger_removed_mutation_detected_in_game",
        "trigger_made_optional_mutation_detected_in_game",
        "target_returned_to_hand_mutation_detected_in_game",
        "printed_body_changed_mutation_detected_in_game",
        "printed_cost_changed_mutation_detected_in_game",
    )
    if not isinstance(mutations, dict) or any(mutations.get(key) is not True for key in required_mutations):
        raise WorkerFailure("candidate_protocol_error: all Vedalken Dismisser gameplay mutants must be detected")


def parse_report(stdout: bytes) -> dict:
    for line in reversed(stdout.decode(errors="replace").splitlines()):
        try:
            value = json.loads(line)
        except json.JSONDecodeError:
            continue
        if value.get("schema_version") == "cardbench.magic.card-pvp.check.v1":
            validate_report(value)
            return value
    raise WorkerFailure("candidate_protocol_error: no valid Vedalken Dismisser PvP result")


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
        raise FileNotFoundError(f"trusted Vedalken Dismisser authority source is missing: {authority}")
    if not shutil.which("docker"):
        raise FileNotFoundError("Docker is required for Magic candidate isolation")
    verify_workspace(workspace, manifest)
    scoring_workspace = make_scoring_snapshot(workspace, manifest_data["files"], output_root / "candidate-workspace")
    candidate_hash = sha256((scoring_workspace / TARGET).read_bytes())
    name = f"cardbench-magic-vedalken-dismisser-{uuid.uuid4().hex[:16]}"
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
            "error": "candidate compile or Vedalken Dismisser reference trace failed",
            "diagnostics": stderr.decode(errors="replace")[-12000:],
        }
    else:
        result = parse_report(stdout)
        result["candidate"] = {
            "kind": candidate_kind, "card_definition": "RAV-VEDALKEN-DISMISSER", "source_sha256": candidate_hash,
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
