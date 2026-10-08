#!/usr/bin/env python3
"""Run the isolated Votary reference authority and validate its diagnostic trace."""

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
MATERIALIZER = Path(__file__).with_name("materialize_votary_of_the_conclave_task.py")
TARGET = Path("varieties/magic/sets/ravnica_city_of_guilds/src/cards/votary_of_the_conclave.rs")
AUTHORITY_SOURCE = Path("varieties/magic/session/src/bin/rav_votary_of_the_conclave_pvp.rs")
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
        "cp /trusted-authority.rs /work/varieties/magic/session/src/bin/rav_votary_of_the_conclave_pvp.rs && "
        "cd /work && cargo run --offline --locked -j 2 --manifest-path varieties/magic/Cargo.toml "
        "-p cardbench-magic-session --bin rav-votary-of-the-conclave-pvp --" + candidate_flag,
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
    raise WorkerFailure("candidate_protocol_error: no valid Votary of the Conclave result")


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
        or report.get("card") != {"collector_number": 35, "definition_id": "RAV-VOTARY-OF-THE-CONCLAVE", "name": "Votary of the Conclave"}
    ):
        raise WorkerFailure("candidate_protocol_error: incomplete Votary diagnostic 1/291 zero-reward result")
    candidate = report.get("candidate")
    if not isinstance(candidate, dict) or candidate.get("card_definition") != "RAV-VOTARY-OF-THE-CONCLAVE":
        raise WorkerFailure("candidate_protocol_error: wrong Votary candidate identity")
    traces = report.get("traces")
    if not isinstance(traces, list) or len(traces) != 1:
        raise WorkerFailure("candidate_protocol_error: exactly one Votary full-game trace is required")
    validate_trace(traces[0])
    mutant = report.get("mutation_sensitivity", {})
    if not isinstance(mutant, dict) or mutant.get("wrong_regeneration_color_mutation_detected_in_game") is not True:
        raise WorkerFailure("candidate_protocol_error: Votary semantic mutant was not detected")


def validate_trace(trace: object) -> None:
    if not isinstance(trace, dict) or trace.get("trace_id") != "rav-votary-targeted-regeneration" or trace.get("terminal") != "Winner(PlayerId(0))" or trace.get("winner_seat") != 0 or trace.get("rejected_policy_moves") != 0:
        raise WorkerFailure("candidate_protocol_error: Votary did not finish a clean full shuffled game")
    votary, brainspoil = trace.get("votary_object"), trace.get("brainspoil_object")
    if not isinstance(votary, int) or not isinstance(brainspoil, int) or trace.get("brainspoil_target_object") != votary:
        raise WorkerFailure("candidate_protocol_error: Brainspoil did not target the live Votary")
    if trace.get("votary_characteristics") != {"power": 1, "toughness": 1}:
        raise WorkerFailure("candidate_protocol_error: Votary did not resolve as a live 1/1")
    if trace.get("votary_mana_paid") != ["White"]:
        raise WorkerFailure("candidate_protocol_error: Votary did not pay exactly one white mana")
    brainspoil_payment = trace.get("brainspoil_mana_paid")
    if not isinstance(brainspoil_payment, list) or len(brainspoil_payment) != 5 or brainspoil_payment.count("Black") != 5:
        raise WorkerFailure("candidate_protocol_error: Brainspoil did not pay its exact two black and three generic cost")
    regen = trace.get("regeneration")
    if not isinstance(regen, dict) or regen.get("mana_cost") != {"generic": 0, "colored": ["Green"]} or regen.get("target_object") != votary or regen.get("shield_created") is not True or regen.get("shield_used") is not True or regen.get("permanent_tapped_after_shield") is not True or regen.get("still_on_battlefield_after_shield") is not True or regen.get("destroyed") is not False or not isinstance(regen.get("source_incarnation"), int) or regen.get("source_incarnation") != regen.get("resolved_incarnation"):
        raise WorkerFailure("candidate_protocol_error: Votary did not pay green, regenerate the target, and survive destruction")
    events = trace.get("events")
    expected = ["SpellManaPaid", "SpellCast", "SpellResolved", "CardMoved", "SpellManaPaid", "SpellCast", "AbilityManaPaid", "AbilityActivated", "RegenerationShieldCreated", "AbilityResolved", "RegenerationShieldUsed", "SpellResolved", "GameEnded"]
    if not isinstance(events, list) or [event.get("kind") for event in events if isinstance(event, dict)] != expected:
        raise WorkerFailure("candidate_protocol_error: ordered Votary payment/cast/resolution/entry/regeneration/end events differ")
    sequences = [event.get("sequence") for event in events]
    if any(not isinstance(sequence, int) for sequence in sequences) or sequences != sorted(sequences) or len(set(sequences)) != len(sequences):
        raise WorkerFailure("candidate_protocol_error: Votary event sequence is malformed")
    facts = [event.get("facts", {}) for event in events]
    if (
        facts[0].get("seats") != [0] or facts[0].get("objects") != [votary] or facts[0].get("mana_colors") != ["White"]
        or facts[1].get("seats") != [0] or facts[1].get("objects") != [votary]
        or facts[2].get("objects") != [votary] or facts[3].get("objects") != [votary] or facts[3].get("zone") != "Battlefield"
        or facts[4].get("seats") != [1] or facts[4].get("objects") != [brainspoil] or facts[4].get("mana_colors") != ["Black"] * 5
        or facts[5].get("seats") != [1] or facts[5].get("objects") != [brainspoil]
        or facts[6].get("seats") != [0] or facts[6].get("objects") != [votary] or facts[6].get("ability") != "regenerate-target-creature" or facts[6].get("mana_cost_generic") != 0 or facts[6].get("mana_cost_colors") != ["Green"]
        or facts[7].get("seats") != [0] or facts[7].get("objects") != [votary] or facts[7].get("ability") != "regenerate-target-creature"
        or facts[8].get("objects") != [votary, votary]
        or facts[9].get("objects") != [votary] or facts[9].get("ability") != "regenerate-target-creature"
        or facts[10].get("objects") != [votary, votary]
        or facts[11].get("objects") != [brainspoil]
        or facts[12].get("seats") != [0]
    ):
        raise WorkerFailure("candidate_protocol_error: Votary typed event facts differ from the paid targeted regeneration trace")


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
        raise FileNotFoundError(f"trusted Votary authority source is missing: {authority}")
    if not shutil.which("docker"):
        raise FileNotFoundError("Docker is required for Magic candidate isolation")
    verify_workspace(workspace, manifest)
    scoring_workspace = make_scoring_snapshot(workspace, manifest_data["files"], output_root / "candidate-workspace")
    candidate_hash = sha256((scoring_workspace / TARGET).read_bytes())
    name = f"cardbench-magic-votary-{uuid.uuid4().hex[:16]}"
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
            "error": "candidate compile or Votary reference trace failed", "diagnostics": stderr.decode(errors="replace")[-12000:],
        }
        (output_root / "result.json").write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
        print(json.dumps(result))
        return 1
    report = parse_report(stdout)
    report["candidate"].update({"kind": candidate_kind, "source_sha256": candidate_hash})
    report.update({"sealed_authority": False, "worker_image": worker_image, "sandbox": {"network": "none", "rootfs": "read-only", "pids_limit": 128}})
    (output_root / "result.json").write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    print(json.dumps(report))
    return 0


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--workspace", required=True, type=Path)
    parser.add_argument("--manifest", required=True, type=Path)
    parser.add_argument("--output-root", required=True, type=Path)
    parser.add_argument("--candidate-kind", choices=("agent", "reference"), default="agent")
    args = parser.parse_args()
    return score(args.workspace.resolve(), args.manifest.resolve(), args.output_root.resolve(), args.candidate_kind)


if __name__ == "__main__":
    raise SystemExit(main())
