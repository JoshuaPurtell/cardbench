#!/usr/bin/env python3
"""Run the isolated Conclave Phalanx reference authority and score a candidate."""

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
MATERIALIZER = Path(__file__).with_name("materialize_conclave_phalanx_task.py")
TARGET = Path("varieties/magic/sets/ravnica_city_of_guilds/src/cards/conclave_phalanx.rs")
AUTHORITY_SOURCE = Path("varieties/magic/session/src/bin/rav_conclave_phalanx_pvp.rs")
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
        "cp /trusted-authority.rs /work/varieties/magic/session/src/bin/rav_conclave_phalanx_pvp.rs && "
        "cd /work && cargo run --offline --locked -j 2 --manifest-path varieties/magic/Cargo.toml "
        "-p cardbench-magic-session --bin rav-conclave-phalanx-pvp --" + candidate_flag,
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
    raise WorkerFailure("candidate_protocol_error: no valid Conclave Phalanx result")


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
        or report.get("card") != {"collector_number": 10, "definition_id": "RAV-CONCLAVE-PHALANX", "name": "Conclave Phalanx"}
    ):
        raise WorkerFailure("candidate_protocol_error: incomplete Phalanx diagnostic 1/291 zero-reward result")
    candidate = report.get("candidate")
    if not isinstance(candidate, dict) or candidate.get("card_definition") != "RAV-CONCLAVE-PHALANX":
        raise WorkerFailure("candidate_protocol_error: wrong Conclave Phalanx candidate identity")
    traces = report.get("traces")
    if not isinstance(traces, list) or len(traces) != 1:
        raise WorkerFailure("candidate_protocol_error: exactly one Phalanx full-game trace is required")
    validate_trace(traces[0])
    mutation = report.get("mutation_sensitivity", {})
    if not isinstance(mutation, dict) or any(mutation.get(key) is not True for key in (
        "convoke_removed_mutation_detected_in_game",
        "convoke_generic_cost_increased_mutation_detected_in_game",
        "global_instead_of_controlled_creature_count_detected_in_game",
    )):
        raise WorkerFailure("candidate_protocol_error: a compiled Phalanx semantic mutant escaped")


def validate_trace(trace: object) -> None:
    if not isinstance(trace, dict) or trace.get("trace_id") != "rav-conclave-phalanx-convoke-etb-life" or trace.get("terminal") != "Winner(PlayerId(0))" or trace.get("winner_seat") != 0:
        raise WorkerFailure("candidate_protocol_error: Phalanx did not finish a clean full shuffled game")
    mechanics = trace.get("mechanics")
    if not isinstance(mechanics, dict):
        raise WorkerFailure("candidate_protocol_error: Phalanx mechanics projection is missing")
    source, fixture, opponent = (mechanics.get(key) for key in ("source_spell", "fixture_spell", "opponent_creature"))
    if not all(isinstance(value, int) for value in (source, fixture, opponent)):
        raise WorkerFailure("candidate_protocol_error: Phalanx fixture object provenance is missing")
    contributors = mechanics.get("convoke_contributors")
    if not isinstance(contributors, list) or len(contributors) != 4 or len(set(contributors)) != 4 or not all(isinstance(value, int) for value in contributors):
        raise WorkerFailure("candidate_protocol_error: four distinct Convoke contributors are required")
    if (
        mechanics.get("mana_cost") != "{4}{W}"
        or mechanics.get("generic_paid_by_convoke") != 4
        or mechanics.get("convoke_contribution") != "Generic"
        or mechanics.get("convoke_creatures_tapped") != [True] * 4
        or mechanics.get("colored_mana_paid") != ["White"]
        or mechanics.get("live_characteristics") != {"power": 2, "toughness": 4, "convoke": True}
        or len(mechanics.get("fixture_tokens_created", [])) != 3
        or mechanics.get("trigger_stacked") is not True
        or mechanics.get("trigger_resolved") is not True
        or mechanics.get("controlled_creatures_at_life_gain") != 5
        or mechanics.get("opponent_creatures_at_life_gain") != 1
        or mechanics.get("life_gained") != 5
        or mechanics.get("controller_life_after_trigger") != 25
        or mechanics.get("attacked_and_dealt_combat_damage") is not True
    ):
        raise WorkerFailure("candidate_protocol_error: Phalanx cost, Convoke, live ETB life count, or combat differs")
    if len(set(mechanics["fixture_tokens_created"])) != 3:
        raise WorkerFailure("candidate_protocol_error: token fixture objects are not distinct")
    events = trace.get("events")
    expected = [
        "CardMoved", "SpellManaPaid", "SpellCast", "SpellResolved", "CardMoved",
        "CardMoved", "SpellManaPaid", "SpellCast", "SpellResolved", "CardMoved",
        "CardMoved", "SpellManaPaid", "SpellCast", "TokenCreated", "TokenCreated", "TokenCreated",
        "SpellResolved", "CardMoved", "ConvokeUsed", "ConvokeUsed", "ConvokeUsed", "ConvokeUsed",
        "SpellManaPaid", "SpellCast", "SpellResolved", "CardMoved", "TriggeredAbilityStacked",
        "LifeGained", "AbilityResolved", "AttackersDeclared", "BlockersDeclared",
        "DamageDealtToPlayer", "GameEnded",
    ]
    if not isinstance(events, list) or [event.get("kind") for event in events if isinstance(event, dict)] != expected:
        raise WorkerFailure("candidate_protocol_error: ordered Phalanx payment/cast/entry/trigger/combat event trace differs")
    sequences = [event.get("sequence") for event in events]
    if any(not isinstance(value, int) for value in sequences) or sequences != sorted(sequences) or len(set(sequences)) != len(sequences):
        raise WorkerFailure("candidate_protocol_error: Phalanx typed event sequence is malformed")
    facts = [event.get("facts", {}) for event in events]
    if (
        facts[0].get("objects") != [opponent]
        or facts[1].get("objects") != [opponent]
        or facts[2].get("seats") != [1] or facts[2].get("objects") != [opponent]
        or facts[3].get("objects") != [opponent]
        or facts[4].get("objects") != [opponent] or facts[4].get("zone") != "Battlefield"
        or facts[7].get("seats") != [0]
        or facts[11].get("objects") != [fixture]
        or facts[12].get("objects") != [fixture]
        or [event.get("facts", {}).get("objects", []) for event in events[13:16]] != [[token] for token in mechanics["fixture_tokens_created"]]
        or facts[17].get("objects") != [fixture]
        or [facts[index].get("objects") for index in range(18, 22)] != [[token] for token in contributors]
        or any(facts[index].get("contribution") != "Generic" or facts[index].get("seats") != [0] for index in range(18, 22))
        or facts[22].get("seats") != [0] or facts[22].get("objects") != [source] or facts[22].get("mana_colors") != ["White"]
        or facts[23].get("seats") != [0] or facts[23].get("objects") != [source]
        or facts[24].get("objects") != [source]
        or facts[25].get("objects") != [source] or facts[25].get("zone") != "Battlefield"
        or facts[26].get("seats") != [0] or facts[26].get("objects") != [source] or facts[26].get("ability") != "etb-gain-life-per-controlled-creature"
        or facts[27].get("seats") != [0] or facts[27].get("amount") != 5
        or facts[28].get("objects") != [source] or facts[28].get("ability") != "etb-gain-life-per-controlled-creature"
        or facts[29].get("seats") != [0] or source not in facts[29].get("objects", [])
        or facts[30].get("seats") != [1] or facts[30].get("amount") != 0
        or facts[31].get("seats") != [1] or facts[31].get("objects") != [source] or facts[31].get("amount") != 2
        or facts[32].get("seats") != [0]
    ):
        raise WorkerFailure("candidate_protocol_error: typed Phalanx witness event facts differ")


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
        raise FileNotFoundError(f"trusted Conclave Phalanx authority is missing: {authority}")
    if not shutil.which("docker"):
        raise FileNotFoundError("Docker is required for Magic candidate isolation")
    verify_workspace(workspace, manifest)
    scoring_workspace = make_scoring_snapshot(workspace, manifest_data["files"], output_root / "candidate-workspace")
    candidate_hash = sha256((scoring_workspace / TARGET).read_bytes())
    name = f"cardbench-magic-phalanx-{uuid.uuid4().hex[:16]}"
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
            "error": "candidate compile or Conclave Phalanx reference trace failed", "diagnostics": stderr.decode(errors="replace")[-12000:],
        }
    else:
        try:
            reference = parse_report(stdout)
        except WorkerFailure as error:
            result = {"schema_version": "cardbench.magic.card-pvp.check.v1", "task_id": "cardbench/magic/card", "candidate": {"kind": candidate_kind, "source_sha256": candidate_hash}, "passed": False, "pilot_card_passed": False, "cards_passed": 0, "cards_total": 291, "printings_total": 306, "cards_credited": 0, "coverage_complete": False, "full_reward": False, "harbor_reward": 0.0, "pilot_reward": 0.0, "sealed_authority": False, "worker_image": worker_image, "sandbox": {"network": "none", "rootfs": "read-only", "pids_limit": 128}, "error": str(error)}
        else:
            reference["candidate"] = {"kind": candidate_kind, "card_definition": "RAV-CONCLAVE-PHALANX", "source_sha256": candidate_hash}
            reference["candidate_kind"] = candidate_kind
            reference["worker_image"] = worker_image
            reference["sandbox"] = {"network": "none", "rootfs": "read-only", "pids_limit": 128}
            result = reference
    payload = json.dumps(result, separators=(",", ":")) + "\n"
    (output_root / "report.json").write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
    sys.stdout.write(payload)
    return 0 if result.get("passed") else 1


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--workspace", required=True, type=Path)
    parser.add_argument("--manifest", required=True, type=Path)
    parser.add_argument("--output-root", required=True, type=Path)
    parser.add_argument("--candidate-kind", choices=("agent", "reference"), default="agent")
    args = parser.parse_args()
    try:
        return score(args.workspace.resolve(), args.manifest.resolve(), args.output_root.resolve(), args.candidate_kind)
    except Exception as error:
        result = {"schema_version": "cardbench.magic.card-pvp.check.v1", "task_id": "cardbench/magic/card", "passed": False, "pilot_card_passed": False, "cards_passed": 0, "cards_total": 291, "cards_credited": 0, "coverage_complete": False, "full_reward": False, "harbor_reward": 0.0, "pilot_reward": 0.0, "sealed_authority": False, "error": f"{type(error).__name__}: {error}"}
        sys.stderr.write(json.dumps(result, separators=(",", ":")) + "\n")
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
