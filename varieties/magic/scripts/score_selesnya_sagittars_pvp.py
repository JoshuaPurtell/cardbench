#!/usr/bin/env python3
"""Run Selesnya Sagittars in a bounded offline candidate worker and score its trace."""

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
MATERIALIZER = Path(__file__).with_name("materialize_selesnya_sagittars_task.py")
TARGET = Path("varieties/magic/sets/ravnica_city_of_guilds/src/cards/selesnya_sagittars.rs")
AUTHORITY_SOURCE = Path("varieties/magic/session/src/bin/rav_selesnya_sagittars_pvp.rs")
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
        "cp /trusted-authority.rs /work/varieties/magic/session/src/bin/rav_selesnya_sagittars_pvp.rs && "
        "cd /work && cargo run --offline --locked -j 2 --manifest-path varieties/magic/Cargo.toml "
        "-p cardbench-magic-session --bin rav-selesnya-sagittars-pvp --" + candidate_flag,
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
    raise WorkerFailure("candidate_protocol_error: no valid Selesnya Sagittars PvP result")


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
        or report.get("card") != {"collector_number": 229, "definition_id": "RAV-SELESNYA-SAGITTARS", "name": "Selesnya Sagittars"}
    ):
        raise WorkerFailure("candidate_protocol_error: incomplete Selesnya Sagittars 1/291 diagnostic result")
    candidate = report.get("candidate")
    if not isinstance(candidate, dict) or candidate.get("card_definition") != "RAV-SELESNYA-SAGITTARS":
        raise WorkerFailure("candidate_protocol_error: wrong Selesnya Sagittars candidate identity")
    traces = report.get("traces")
    if not isinstance(traces, list) or len(traces) != 1:
        raise WorkerFailure("candidate_protocol_error: one Selesnya Sagittars full-game trace is required")
    validate_trace(traces[0])
    mutation = report.get("mutation_sensitivity", {})
    required = (
        "damage_removed_mutation_detected_in_game",
        "tap_cost_removed_mutation_detected_in_game",
        "reach_removed_mutation_detected_in_game",
        "target_boundary_broadened_mutation_detected_by_probe",
    )
    if not isinstance(mutation, dict) or any(mutation.get(key) is not True for key in required):
        raise WorkerFailure("candidate_protocol_error: Selesnya Sagittars semantic controls are incomplete")


def validate_trace(trace: object) -> None:
    if not isinstance(trace, dict) or trace.get("trace_id") != "rav-selesnya-sagittars-reach-and-tap-damage-pvp" or trace.get("terminal") != "Winner(PlayerId(0))" or trace.get("winner_seat") != 0 or trace.get("rejected_moves") != 0:
        raise WorkerFailure("candidate_protocol_error: Selesnya Sagittars did not finish a clean full game")
    sag = trace.get("sagittars")
    reach = trace.get("reach")
    activation = trace.get("activation")
    finisher = trace.get("finisher")
    if not isinstance(sag, dict) or sag.get("mana_cost") != "{3}{G}{W}" or sag.get("mana_paid") != ["Green", "White", "Green", "Green", "White"] or sag.get("zone") != "Battlefield" or sag.get("power") != 2 or sag.get("toughness") != 5:
        raise WorkerFailure("candidate_protocol_error: Sagittars cost, payment, or live characteristics differ")
    if not isinstance(reach, dict) or reach.get("legal_block") is not True or reach.get("blocker") != sag.get("object") or reach.get("hawk_zone_after_combat") != "Graveyard":
        raise WorkerFailure("candidate_protocol_error: Reach did not block and defeat the flying attacker")
    if not isinstance(activation, dict) or activation.get("ability") != "tap-damage-attacker-or-blocker" or activation.get("source") != sag.get("object") or activation.get("target_was_attacking") is not True or activation.get("damage") != 1 or activation.get("tap_cost_paid") is not True or activation.get("target_zone_after_resolution") != "Graveyard":
        raise WorkerFailure("candidate_protocol_error: attacking-creature tap-damage ability differs")
    if not isinstance(finisher, dict) or not isinstance(finisher.get("watchwolf"), int) or finisher.get("p1_life", 1) > 0 or finisher.get("p0_life", 0) <= 0:
        raise WorkerFailure("candidate_protocol_error: full-game combat finish is missing")
    probe = trace.get("legality_probe")
    if not isinstance(probe, dict) or probe.get("target_was_attacking_or_blocking") is not False or probe.get("rejected_by_engine") is not True or probe.get("match_state_unchanged") is not True or "IllegalTarget" not in probe.get("rejection", ""):
        raise WorkerFailure("candidate_protocol_error: noncombat-target legality probe is missing or stateful")
    expected = ["SpellManaPaid", "SpellCast", "SpellResolved", "CardMoved", "AttackersDeclared", "BlockersDeclared", "AbilityActivated", "DamageDealtToPermanent", "AbilityResolved", "StateBasedAction", "CardMoved", "DamageDealtToPermanent", "DamageDealtToPermanent", "StateBasedAction", "CardMoved", "AttackersDeclared", "DamageDealtToPlayer", "GameEnded"]
    events = trace.get("events")
    if not isinstance(events, list) or [event.get("kind") for event in events if isinstance(event, dict)] != expected:
        raise WorkerFailure("candidate_protocol_error: ordered Selesnya Sagittars event sequence differs")
    sequences = [event.get("sequence") for event in events]
    if any(not isinstance(sequence, int) for sequence in sequences) or sequences != sorted(sequences) or len(set(sequences)) != len(sequences):
        raise WorkerFailure("candidate_protocol_error: Selesnya Sagittars event order is malformed")
    facts = [event.get("facts", {}) for event in events]
    sagittars_id = sag.get("object")
    hawk_id = reach.get("attacking_flying_hawk")
    elf_id = activation.get("target")
    watchwolf_id = finisher.get("watchwolf")
    if not all(isinstance(value, int) for value in (sagittars_id, hawk_id, elf_id, watchwolf_id)):
        raise WorkerFailure("candidate_protocol_error: trace object identities are malformed")
    if facts[0].get("seats") != [0] or facts[0].get("mana_colors") != sag.get("mana_paid") or facts[0].get("objects") != [sagittars_id]:
        raise WorkerFailure("candidate_protocol_error: colored spell payment receipt differs")
    if facts[1].get("seats") != [0] or facts[1].get("objects") != [sagittars_id] or facts[2].get("objects") != [sagittars_id] or facts[3].get("objects") != [sagittars_id] or facts[3].get("zone") != "Battlefield":
        raise WorkerFailure("candidate_protocol_error: Sagittars cast, resolution, and entry receipts differ")
    if facts[4].get("seats") != [1] or facts[4].get("objects") != [elf_id, hawk_id]:
        raise WorkerFailure("candidate_protocol_error: both intended opponents did not attack")
    if facts[5].get("seats") != [0] or facts[5].get("objects") != [hawk_id, sagittars_id]:
        raise WorkerFailure("candidate_protocol_error: Reach block assignment differs")
    if facts[6].get("seats") != [0] or facts[6].get("objects") != [sagittars_id] or facts[6].get("ability") != "tap-damage-attacker-or-blocker":
        raise WorkerFailure("candidate_protocol_error: activation stack receipt differs")
    if facts[7].get("objects") != [sagittars_id, elf_id] or facts[7].get("amount") != 1:
        raise WorkerFailure("candidate_protocol_error: one-damage activation receipt differs")
    if facts[8].get("objects") != [sagittars_id] or facts[8].get("ability") != "tap-damage-attacker-or-blocker":
        raise WorkerFailure("candidate_protocol_error: ability resolution receipt differs")
    if facts[9].get("objects") != [elf_id] or facts[10].get("objects") != [elf_id] or facts[10].get("zone") != "Graveyard":
        raise WorkerFailure("candidate_protocol_error: attacking Elf did not die before combat damage")
    if facts[11].get("objects") != [hawk_id, sagittars_id] or facts[11].get("amount") != 1 or facts[12].get("objects") != [sagittars_id, hawk_id] or facts[12].get("amount") != 2:
        raise WorkerFailure("candidate_protocol_error: Reach combat exchange differs")
    if facts[13].get("objects") != [hawk_id] or facts[14].get("objects") != [hawk_id] or facts[14].get("zone") != "Graveyard":
        raise WorkerFailure("candidate_protocol_error: block-killed Hawk did not move to graveyard")
    if watchwolf_id not in facts[15].get("objects", []) or facts[15].get("seats") != [0] or facts[16].get("objects") != [watchwolf_id] or facts[16].get("amount") != 3 or facts[16].get("seats") != [1] or facts[17].get("seats") != [0]:
        raise WorkerFailure("candidate_protocol_error: later Watchwolf terminal game finish differs")


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
        raise FileNotFoundError(f"trusted Selesnya Sagittars authority source is missing: {authority}")
    if not shutil.which("docker"):
        raise FileNotFoundError("Docker is required for Magic candidate isolation")
    verify_workspace(workspace, manifest)
    scoring_workspace = make_scoring_snapshot(workspace, manifest_data["files"], output_root / "candidate-workspace")
    candidate_hash = sha256((scoring_workspace / TARGET).read_bytes())
    name = f"cardbench-magic-selesnya-sagittars-{uuid.uuid4().hex[:16]}"
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
            "error": "candidate compile or Selesnya Sagittars reference trace failed",
            "diagnostics": stderr.decode(errors="replace")[-12000:],
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
