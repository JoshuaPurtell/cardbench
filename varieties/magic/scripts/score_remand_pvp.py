#!/usr/bin/env python3
"""Validate local Remand reference reports or run the isolated Harbor worker."""

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
MATERIALIZER = Path(__file__).with_name("materialize_remand_task.py")
TARGET = Path("varieties/magic/sets/ravnica_city_of_guilds/src/cards/remand.rs")
AUTHORITY_SOURCE = Path("varieties/magic/session/src/bin/rav_remand_pvp.rs")
WORKER_IMAGE = "rust@sha256:d7482085ff5b415f84dba5647ae71606650bdef00db7aeb69f4b3d170c3e4082"
MAX_OUTPUT_BYTES = 2 * 1024 * 1024


class WorkerFailure(RuntimeError):
    pass


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def validate_report(report: dict) -> None:
    expected_card = {"collector_number": 63, "definition_id": "RAV-REMAND", "name": "Remand", "printings": 1}
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
        or report.get("card") != expected_card
        or report.get("replay_deterministic") is not True
    ):
        raise WorkerFailure("candidate_protocol_error: incomplete Remand diagnostic report or nonzero reward")

    candidate = report.get("candidate")
    if not isinstance(candidate, dict) or candidate.get("card_definition") != "RAV-REMAND":
        raise WorkerFailure("candidate_protocol_error: wrong Remand identity")
    traces = report.get("traces")
    if not isinstance(traces, list) or len(traces) != 1:
        raise WorkerFailure("candidate_protocol_error: one Remand trace is required")
    trace = traces[0]
    if (
        not isinstance(trace, dict)
        or trace.get("id") != "rav-remand-counter-return-and-draw"
        or trace.get("passed") is not True
        or trace.get("terminal") != "Winner(PlayerId(0))"
        or trace.get("winner_seat") != 0
        or trace.get("rejected_moves") != 0
        or trace.get("target_spell_owner_seat") != 1
        or trace.get("target_spell_zone") != "Some(Hand)"
        or trace.get("drawn_card_owner_seat") != 0
        or trace.get("drawn_card_zone") != "Some(Hand)"
        or trace.get("remand_zone") != "Some(Graveyard)"
    ):
        raise WorkerFailure("candidate_protocol_error: Remand did not satisfy its full-game state assertions")
    if not isinstance(trace.get("turns"), int) or trace["turns"] < 1:
        raise WorkerFailure("candidate_protocol_error: missing terminal match turn count")

    remand = trace.get("remand_object")
    target = trace.get("target_spell_object")
    drawn = trace.get("drawn_card_object")
    if (
        not isinstance(remand, int)
        or not isinstance(target, int)
        or not isinstance(drawn, int)
        or target == remand
        or drawn in {remand, target}
        or trace.get("remand_target_object") != target
    ):
        raise WorkerFailure("candidate_protocol_error: exact target and draw object identities differ")

    mana = trace.get("remand_payment")
    target_mana = trace.get("target_spell_payment")
    if not isinstance(mana, dict) or mana.get("blue_paid") is not True or len(mana.get("colors", [])) != 2:
        raise WorkerFailure("candidate_protocol_error: Remand did not pay {1}{U}")
    if (
        not isinstance(target_mana, dict)
        or target_mana.get("green_paid") is not True
        or target_mana.get("white_paid") is not True
        or len(target_mana.get("colors", [])) != 2
    ):
        raise WorkerFailure("candidate_protocol_error: target Watchwolf did not pay {G}{W}")

    events = trace.get("events")
    kinds = [event.get("kind") for event in events if isinstance(event, dict)] if isinstance(events, list) else []
    expected_kinds = [
        "SpellManaPaid", "SpellCast", "SpellManaPaid", "SpellCast",
        "CounteredSpellHandReplacement", "SpellCountered", "CardMoved",
        "CardMoved", "SpellResolved", "CardMoved", "AttackersDeclared",
        "DamageDealtToPlayer", "GameEnded",
    ]
    sequences = [event.get("sequence") for event in events if isinstance(event, dict)] if isinstance(events, list) else []
    ordered = trace.get("ordered_event_sequence")
    if (
        kinds != expected_kinds
        or any(not isinstance(sequence, int) for sequence in sequences)
        or sequences != sorted(sequences)
        or ordered != sequences
    ):
        raise WorkerFailure("candidate_protocol_error: Remand counter, return, draw, and terminal event order differs")
    facts = [event.get("facts", {}) for event in events]
    if (
        facts[0].get("objects") != [target]
        or facts[0].get("seats") != [1]
        or facts[1].get("objects") != [target]
        or facts[1].get("seats") != [1]
        or facts[2].get("objects") != [remand]
        or facts[2].get("seats") != [0]
        or facts[3].get("objects") != [remand]
        or facts[3].get("seats") != [0]
        or facts[6].get("objects") != [target]
        or facts[6].get("zone") != "Hand"
        or facts[7].get("objects") != [drawn]
        or facts[7].get("zone") != "Hand"
        or facts[8].get("objects") != [remand]
        or facts[9].get("objects") != [remand]
        or facts[9].get("zone") != "Graveyard"
        or facts[12].get("seats") != [0]
    ):
        raise WorkerFailure("candidate_protocol_error: projected Remand event facts differ")
    canonical = [event.get("canonical", "") for event in events]
    if (
        f"CounteredSpellHandReplacement {{ card: ObjectId({target}), source: ObjectId({remand}) }}" not in canonical[4]
        or f"SpellCountered {{ card: ObjectId({target}), source: ObjectId({remand}) }}" not in canonical[5]
    ):
        raise WorkerFailure("candidate_protocol_error: counter provenance does not match Remand and its exact target")

    mutation = report.get("mutation_sensitivity")
    required = (
        "wrong_destination_mutation_detected_in_game",
        "missing_draw_mutation_detected_in_game",
        "missing_counter_mutation_detected_in_game",
    )
    if not isinstance(mutation, dict) or any(mutation.get(key) is not True for key in required):
        raise WorkerFailure("candidate_protocol_error: Remand gameplay mutation controls are incomplete")


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
                destination = output if key.data == "stdout" else diagnostics
                destination.extend(chunk)
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
        "cp /trusted-authority.rs /work/varieties/magic/session/src/bin/rav_remand_pvp.rs && "
        "cd /work && cargo run --offline --locked -j 2 --manifest-path varieties/magic/Cargo.toml "
        "-p cardbench-magic-session --bin rav-remand-pvp --" + candidate_flag,
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
    raise WorkerFailure("candidate_protocol_error: no valid Remand PvP result")


def score(workspace: Path, manifest: Path, output_root: Path, candidate_kind: str) -> int:
    output_root.mkdir(parents=True, exist_ok=True)
    manifest_data = json.loads(manifest.read_text(encoding="utf-8"))
    worker_image = os.environ.get("CARDBENCH_MAGIC_WORKER_IMAGE", WORKER_IMAGE)
    if not (worker_image.startswith("rust@sha256:") or worker_image.startswith("sha256:")):
        raise ValueError("CARDBENCH_MAGIC_WORKER_IMAGE must be an immutable digest reference")
    cargo_registry = Path.home() / ".cargo" / "registry"
    if not cargo_registry.is_dir():
        raise FileNotFoundError(f"offline Cargo registry cache is missing: {cargo_registry}")
    authority = REPO / AUTHORITY_SOURCE
    if authority.is_symlink() or not authority.is_file():
        raise FileNotFoundError(f"trusted Remand authority source is missing: {authority}")
    if not shutil.which("docker"):
        raise FileNotFoundError("Docker is required for Magic candidate isolation")
    verify_workspace(workspace, manifest)
    scoring_workspace = make_snapshot(workspace, manifest_data["files"], output_root / "candidate-workspace")
    candidate_hash = sha256((scoring_workspace / TARGET).read_bytes())
    name = f"cardbench-magic-remand-{uuid.uuid4().hex[:16]}"
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
            "pilot_reward": 0.0,
            "sealed_authority": False,
            "worker_image": worker_image,
            "sandbox": {"network": "none", "rootfs": "read-only", "pids_limit": 128},
            "error": "candidate compile or Remand full-game reference trace failed",
            "diagnostics": stderr.decode(errors="replace")[-12000:],
        }
    else:
        result = parse_report(stdout)
        result["candidate"] = {
            "kind": candidate_kind,
            "card_definition": "RAV-REMAND",
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
    (output_root / "reward.txt").write_text(f"{result['harbor_reward']:.8f}\n", encoding="utf-8")
    print(f"candidate_passed={result.get('pilot_card_passed', False)} cards={result.get('cards_passed', 0)}/291 pilot_reward=0.0")
    return 0 if result.get("passed") is True and result.get("pilot_card_passed") is True else 1


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--workspace", type=Path)
    parser.add_argument("--manifest", type=Path)
    parser.add_argument("--output-root", type=Path)
    parser.add_argument("--candidate-kind", choices=["agent", "reference"], default="agent")
    parser.add_argument("--report-file", type=Path, help="validate a local authority JSON report without Docker")
    args = parser.parse_args()
    if args.report_file:
        try:
            report = json.loads(args.report_file.read_text(encoding="utf-8"))
            validate_report(report)
            print("local_reference_report=valid cards=1/291 credited=0 pilot_reward=0.0")
            return 0
        except Exception as error:
            print(f"Remand report validation failed: {error}", file=sys.stderr)
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
