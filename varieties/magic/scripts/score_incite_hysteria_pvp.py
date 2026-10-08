#!/usr/bin/env python3
"""Run the Incite Hysteria reference authority in an offline isolated worker."""

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
MATERIALIZER = Path(__file__).with_name("materialize_incite_hysteria_task.py")
TARGET = Path("varieties/magic/sets/ravnica_city_of_guilds/src/cards/incite_hysteria.rs")
AUTHORITY_SOURCE = Path("varieties/magic/session/src/bin/rav_incite_hysteria_pvp.rs")
DEFAULT_WORKER_IMAGE = "rust@sha256:d7482085ff5b415f84dba5647ae71606650bdef00db7aeb69f4b3d170c3e4082"
MAX_OUTPUT_BYTES = 2 * 1024 * 1024


class WorkerFailure(RuntimeError):
    pass


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def verify_workspace(workspace: Path, manifest: Path) -> None:
    subprocess.run([sys.executable, str(MATERIALIZER), "--workspace", str(workspace), "--manifest", str(manifest), "--verify"], check=True, cwd=REPO)


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
    output, diagnostics = bytearray(), bytearray()
    started = time.monotonic()
    try:
        while selector.get_map() or proc.poll() is None:
            if time.monotonic() - started > timeout:
                raise WorkerFailure("candidate_timeout: offline worker exceeded its deadline")
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
        "cp /trusted-authority.rs /work/varieties/magic/session/src/bin/rav_incite_hysteria_pvp.rs && "
        "cd /work && cargo run --offline --locked -j 2 --manifest-path varieties/magic/Cargo.toml "
        "-p cardbench-magic-session --bin rav-incite-hysteria-pvp --" + candidate_flag,
    ]


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
        or report.get("viewer_scope") != "Judge"
        or report.get("card") != {"collector_number": 132, "printings": 1, "definition_id": "RAV-INCITE-HYSTERIA", "name": "Incite Hysteria"}
    ):
        raise WorkerFailure("candidate_protocol_error: incomplete Incite Hysteria PvP report")
    candidate = report.get("candidate")
    if not isinstance(candidate, dict) or candidate.get("card_definition") != "RAV-INCITE-HYSTERIA":
        raise WorkerFailure("candidate_protocol_error: wrong Incite Hysteria candidate identity")
    traces = report.get("traces")
    if not isinstance(traces, list) or len(traces) != 1 or traces[0].get("trace_id") != "rav-incite-hysteria-radiance-cannot-block-pvp":
        raise WorkerFailure("candidate_protocol_error: one Radiance CannotBlock trace is required")
    trace = traces[0]
    if trace.get("terminal") != "Winner(PlayerId(0))" or trace.get("rejected_policy_moves") != 0:
        raise WorkerFailure("candidate_protocol_error: shuffled two-player match did not terminate cleanly")
    mechanics = trace.get("mechanics")
    if not isinstance(mechanics, dict):
        raise WorkerFailure("candidate_protocol_error: Incite mechanics are missing")
    spell, target, attacker = mechanics.get("spell"), mechanics.get("target"), mechanics.get("attacker")
    recipients, expected = mechanics.get("radiance_recipients"), mechanics.get("expected_recipients")
    mana_paid = mechanics.get("mana_paid")
    if (
        mechanics.get("cast_from_shuffled_library") is not True
        or mechanics.get("mana_cost") != "{2}{R}"
        or not isinstance(spell, int)
        or not isinstance(target, int)
        or not isinstance(attacker, int)
        or not isinstance(mana_paid, list)
        or len(mana_paid) != 3
        or mana_paid.count("red") != 1
        or not isinstance(recipients, list)
        or not isinstance(expected, list)
        or len(recipients) < 2
        or sorted(recipients) != sorted(expected)
        or target not in recipients
        or mechanics.get("target_and_shared_red_cannot_block") is not True
        or mechanics.get("unrelated_green_can_block") is not True
        or mechanics.get("target_block_legality_probe_rejected") is not True
        or mechanics.get("probe_state_unchanged") is not True
        or not isinstance(mechanics.get("probe_error"), str)
        or "illegal blocker" not in mechanics["probe_error"]
        or mechanics.get("legal_unrelated_blocker") is None
        or mechanics.get("legal_block_declared") is not True
        or mechanics.get("spell_in_graveyard") is not True
    ):
        raise WorkerFailure("candidate_protocol_error: cost, Radiance scope, legal-block control, or terminal state differs")
    events = trace.get("events")
    if not isinstance(events, list):
        raise WorkerFailure("candidate_protocol_error: canonical Incite event trace is missing")
    kinds = [event.get("kind") for event in events if isinstance(event, dict)]
    expected_kinds = ["SpellManaPaid", "SpellCast"] + ["ContinuousEffectCreated"] * len(recipients) + ["SpellResolved", "CardMoved", "AttackersDeclared", "BlockersDeclared", "GameEnded"]
    if kinds != expected_kinds:
        raise WorkerFailure("candidate_protocol_error: ordered payment, Radiance, resolution, legal block and terminal events are incomplete")
    if (
        events[0].get("facts", {}).get("objects") != [spell]
        or events[0].get("facts", {}).get("seats") != [0]
        or events[0].get("facts", {}).get("mana_colors") is None
        or len(events[0].get("facts", {}).get("mana_colors", [])) != 3
        or events[0].get("facts", {}).get("mana_colors", []).count("Red") != 1
        or events[1].get("facts", {}).get("objects") != [spell]
        or events[1].get("facts", {}).get("seats") != [0]
        or any(f"source: ObjectId({spell})" not in event.get("canonical", "") for event in events[2:2 + len(recipients)])
        or not any(f"target: ObjectId({target})" in event.get("canonical", "") for event in events[2:2 + len(recipients)])
        or events[-5].get("facts", {}).get("objects") != [spell]
        or events[-4].get("facts", {}).get("objects") != [spell]
        or events[-4].get("facts", {}).get("zone") != "Graveyard"
        or events[-3].get("facts", {}).get("seats") != [0]
        or attacker not in events[-3].get("facts", {}).get("objects", [])
        or not isinstance(mechanics.get("legal_unrelated_blocker"), int)
        or events[-2].get("facts", {}).get("objects") != [attacker, mechanics["legal_unrelated_blocker"]]
        or events[-2].get("facts", {}).get("seats") != [1]
        or events[-2].get("facts", {}).get("amount") != 1
        or events[-1].get("facts", {}).get("seats") != [0]
    ):
        raise WorkerFailure("candidate_protocol_error: Incite source, target, payment, or combat receipts differ")
    sequence = [event.get("sequence") for event in events]
    if any(not isinstance(value, int) for value in sequence) or sequence != sorted(sequence):
        raise WorkerFailure("candidate_protocol_error: ordered event sequence is malformed")
    mutations = report.get("mutation_sensitivity")
    required = (
        "radiance_replaced_with_target_only_mutation_detected",
        "cannot_block_keyword_changed_mutation_detected",
        "printed_cost_changed_mutation_detected",
    )
    if not isinstance(mutations, dict) or any(mutations.get(key) is not True for key in required):
        raise WorkerFailure("candidate_protocol_error: Radiance, keyword, and cost mutation controls are required")


def parse_report(stdout: bytes) -> dict:
    for line in reversed(stdout.decode(errors="replace").splitlines()):
        try:
            value = json.loads(line)
        except json.JSONDecodeError:
            continue
        if value.get("schema_version") == "cardbench.magic.card-pvp.check.v1":
            validate_report(value)
            return value
    raise WorkerFailure("candidate_protocol_error: no valid Incite Hysteria PvP result")


def score(workspace: Path, manifest: Path, output_root: Path, candidate_kind: str) -> int:
    output_root.mkdir(parents=True, exist_ok=True)
    manifest_data = json.loads(manifest.read_text(encoding="utf-8"))
    image = os.environ.get("CARDBENCH_MAGIC_WORKER_IMAGE", DEFAULT_WORKER_IMAGE)
    if not (image.startswith("rust@sha256:") or image.startswith("sha256:")):
        raise ValueError("CARDBENCH_MAGIC_WORKER_IMAGE must be an immutable digest reference")
    registry = Path.home() / ".cargo" / "registry"
    if not registry.is_dir():
        raise FileNotFoundError(f"offline Cargo registry cache is missing: {registry}")
    authority = REPO / AUTHORITY_SOURCE
    if authority.is_symlink() or not authority.is_file():
        raise FileNotFoundError(f"trusted Incite Hysteria authority is missing: {authority}")
    if not shutil.which("docker"):
        raise FileNotFoundError("Docker is required for Magic candidate isolation")
    verify_workspace(workspace, manifest)
    snapshot = make_snapshot(workspace, manifest_data["files"], output_root / "candidate-workspace")
    candidate_hash = sha256((snapshot / TARGET).read_bytes())
    name = f"cardbench-magic-incite-hysteria-{uuid.uuid4().hex[:16]}"
    command = sandbox_command(snapshot, registry.resolve(), authority, name, image, candidate_kind)
    code, stdout, stderr = run_bounded(command, timeout=900, name=name)
    (output_root / "worker.stdout").write_bytes(stdout)
    (output_root / "worker.stderr").write_bytes(stderr)
    if code != 0:
        result = {"schema_version":"cardbench.magic.card-pvp.check.v1","task_id":"cardbench/magic/card","candidate":{"kind":candidate_kind,"source_sha256":candidate_hash},"passed":False,"pilot_card_passed":False,"cards_passed":0,"cards_total":291,"printings_total":306,"cards_credited":0,"coverage_complete":False,"full_reward":False,"harbor_reward":0.0,"sealed_authority":False,"worker_image":image,"sandbox":{"network":"none","rootfs":"read-only","pids_limit":128},"error":"candidate compile or Incite Hysteria authority trace failed"}
        (output_root / "result.json").write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
        return 0
    report = parse_report(stdout)
    report["candidate"]["source_sha256"] = candidate_hash
    report["worker_image"] = image
    report["sandbox"] = {"network":"none","rootfs":"read-only","pids_limit":128}
    (output_root / "result.json").write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
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
