#!/usr/bin/env python3
"""Run the Auratouched Mage pilot in a networkless bounded worker."""

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
MATERIALIZER = Path(__file__).with_name("materialize_auratouched_mage_task.py")
TARGET = Path("varieties/magic/sets/ravnica_city_of_guilds/src/cards/auratouched_mage.rs")
AUTHORITY_SOURCE = Path("varieties/magic/session/src/bin/rav_auratouched_mage_pvp.rs")
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
        "cp /trusted-authority.rs /work/varieties/magic/session/src/bin/rav_auratouched_mage_pvp.rs && "
        "cd /work && cargo run --offline --locked -j 2 --manifest-path varieties/magic/Cargo.toml "
        "-p cardbench-magic-session --bin rav-auratouched-mage-pvp --" + candidate_flag,
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
    raise WorkerFailure("candidate_protocol_error: no valid Auratouched Mage PvP result")


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
        or report.get("card") != {"collector_number": 1, "definition_id": "RAV-AURATOUCHED-MAGE", "name": "Auratouched Mage"}
        or report.get("deterministic_replay") is not True
    ):
        raise WorkerFailure("candidate_protocol_error: incomplete Auratouched Mage single-card report")
    traces = report.get("traces")
    expected_traces = [
        ("rav-auratouched-mage-finds-and-attaches-moldervine-cloak", "find"),
        ("rav-auratouched-mage-declines-with-compatible-aura-available", "decline"),
    ]
    if not isinstance(traces, list) or len(traces) != len(expected_traces):
        raise WorkerFailure("candidate_protocol_error: both full-game Aura choice traces are required")
    common_seed = None
    for trace, (trace_id, choice) in zip(traces, expected_traces, strict=True):
        if (
            not isinstance(trace, dict)
            or trace.get("id") != trace_id
            or trace.get("choice") != choice
            or trace.get("passed") is not True
            or trace.get("terminal") != "Winner(PlayerId(0))"
            or trace.get("winner_seat") != 0
            or trace.get("turns", 0) <= 0
            or trace.get("rejected_moves") != 0
            or trace.get("private_search_choice") is not True
            or trace.get("opponent_saw_no_search_choices") is not True
            or trace.get("search_option_count", 0) <= 0
            or trace.get("mage_combat_damage_to_opponent", 0) < 20
        ):
            raise WorkerFailure("candidate_protocol_error: full-game private Aura search and combat result is invalid")
        if common_seed is None:
            common_seed = trace.get("seed")
        elif trace.get("seed") != common_seed:
            raise WorkerFailure("candidate_protocol_error: paired branches must use one shared shuffle seed")
        mage = trace.get("mage_object")
        incarnation = trace.get("mage_incarnation_at_trigger")
        requested = trace.get("requested_selection_object")
        if not isinstance(mage, int) or not isinstance(incarnation, int) or not isinstance(trace.get("events_digest"), str):
            raise WorkerFailure("candidate_protocol_error: exact Mage object and trigger incarnation are required")
        if choice == "find":
            aura = trace.get("aura_object")
            if (
                not isinstance(aura, int)
                or trace.get("aura_definition") != "RAV-MOLDERVINE-CLOAK"
                or requested != aura
                or trace.get("aura_attached_to_mage") is not True
                or trace.get("mage_power_toughness_after_choice") != [6, 6]
            ):
                raise WorkerFailure("candidate_protocol_error: selected Cloak must attach to the exact Mage object and make it 6/6")
            expected = [
                "SpellCast", "SpellResolved", "CardMoved", "TriggeredAbilityStacked",
                "DecisionOpened", "DecisionCompleted", "CardMoved", "AuraAttached", "LibrarySearchResolved",
                "LibraryShuffled", "AbilityResolved", "AttackersDeclared", "DamageDealtToPlayer", "GameEnded",
            ]
            canonical = {
                "AuraAttached": f"AuraAttached {{ aura: ObjectId({aura}), target: ObjectId({mage}) }}",
                "LibrarySearchResolved": f"LibrarySearchResolved {{ player: PlayerId(0), source: ObjectId({mage}), found: Some(ObjectId({aura})), destination: Battlefield }}",
            }
        else:
            aura = trace.get("aura_object")
            if (
                aura is not None
                or requested is not None
                or trace.get("aura_attached_to_mage") is not False
                or trace.get("mage_power_toughness_after_choice") != [3, 3]
            ):
                raise WorkerFailure("candidate_protocol_error: decline branch must leave all compatible Auras and the 3/3 Mage unchanged")
            expected = [
                "SpellCast", "SpellResolved", "CardMoved", "TriggeredAbilityStacked",
                "DecisionOpened", "DecisionCompleted", "LibrarySearchResolved",
                "LibraryShuffled", "AbilityResolved", "AttackersDeclared", "DamageDealtToPlayer", "GameEnded",
            ]
            canonical = {
                "LibrarySearchResolved": f"LibrarySearchResolved {{ player: PlayerId(0), source: ObjectId({mage}), found: None, destination: Battlefield }}",
            }
        events = trace.get("events")
        if not isinstance(events, list) or not all(isinstance(event, dict) for event in events):
            raise WorkerFailure("candidate_protocol_error: event trace is malformed")
        if [event.get("kind") for event in events] != expected:
            raise WorkerFailure("candidate_protocol_error: Mage, trigger, search, attachment, combat, and terminal event order differs")
        sequences = [event.get("sequence") for event in events]
        if any(not isinstance(item, int) for item in sequences) or sequences != sorted(sequences):
            raise WorkerFailure("candidate_protocol_error: event sequence is malformed")
        for event in events:
            kind = event["kind"]
            if kind in {"SpellCast", "SpellResolved", "TriggeredAbilityStacked", "AbilityResolved", "AttackersDeclared", "DamageDealtToPlayer"}:
                if mage not in event.get("facts", {}).get("objects", []):
                    raise WorkerFailure("candidate_protocol_error: event receipt is not bound to the exact Mage object")
            if kind in canonical and event.get("canonical") != canonical[kind]:
                raise WorkerFailure("candidate_protocol_error: Aura/source/object receipt identity differs")
        moved_objects = [event.get("facts", {}).get("objects") for event in events if event.get("kind") == "CardMoved"]
        expected_moved_objects = [[mage], [aura]] if choice == "find" else [[mage]]
        if moved_objects != expected_moved_objects:
            raise WorkerFailure("candidate_protocol_error: exact Mage and selected Aura zone-change identities differ")
        if choice == "find":
            aura_move = events[6]
            if aura_move.get("canonical") != f"CardMoved {{ card: ObjectId({aura}), to: Battlefield }}":
                raise WorkerFailure("candidate_protocol_error: selected Aura did not enter under its exact object identity")
        trigger_event = next(event for event in events if event["kind"] == "TriggeredAbilityStacked")
        if f"source_incarnation: {incarnation}" not in trigger_event.get("canonical", ""):
            raise WorkerFailure("candidate_protocol_error: trigger receipt does not bind the captured Mage incarnation")
    mutation = report.get("mutation_sensitivity", {})
    if not isinstance(mutation, dict) or any(
        mutation.get(key) is not True
        for key in (
            "etb_trigger_removed_mutation_detected_in_game",
            "incompatible_aura_allowed_mutation_detected_in_game",
            "aura_attachment_removed_mutation_detected_in_game",
        )
    ):
        raise WorkerFailure("candidate_protocol_error: all Auratouched Mage semantic mutants must fail")

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
        raise FileNotFoundError(f"trusted Auratouched Mage authority source is missing: {authority}")
    if not shutil.which("docker"):
        raise FileNotFoundError("Docker is required for Magic candidate isolation")
    verify_workspace(workspace, manifest)
    scoring_workspace = make_scoring_snapshot(workspace, manifest_data["files"], output_root / "candidate-workspace")
    candidate_hash = sha256((scoring_workspace / TARGET).read_bytes())
    name = f"cardbench-magic-auratouched-mage-{uuid.uuid4().hex[:16]}"
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
            "error": "candidate compile or Auratouched Mage reference trace failed",
            "diagnostics": stderr.decode(errors="replace")[-12000:],
        }
    else:
        result = parse_report(stdout)
        result["candidate"] = {"kind": candidate_kind, "card_definition": "RAV-AURATOUCHED-MAGE", "source_sha256": candidate_hash, "materialized_workspace_sha256": manifest_data["workspace_sha256"]}
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
