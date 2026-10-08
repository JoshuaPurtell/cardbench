#!/usr/bin/env python3
"""Run the bounded offline Wizened Snitches card-task authority."""

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
MATERIALIZER = Path(__file__).with_name("materialize_wizened_snitches_task.py")
TARGET = Path("varieties/magic/sets/ravnica_city_of_guilds/src/cards/wizened_snitches.rs")
AUTHORITY_SOURCE = Path("varieties/magic/session/src/bin/rav_wizened_snitches_pvp.rs")
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
        "cp /trusted-authority.rs /work/varieties/magic/session/src/bin/rav_wizened_snitches_pvp.rs && "
        "cd /work && cargo run --offline --locked -j 2 --manifest-path varieties/magic/Cargo.toml "
        "-p cardbench-magic-session --bin rav-wizened-snitches-pvp --" + candidate_flag,
    ]


def validate_report(report: dict) -> None:
    expected_card = {"collector_number": 75, "definition_id": "RAV-WIZENED-SNITCHES", "name": "Wizened Snitches", "printings": 1}
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
        raise WorkerFailure("candidate_protocol_error: incomplete Wizened Snitches single-card report")
    traces = report.get("traces")
    if not isinstance(traces, list) or len(traces) != 1:
        raise WorkerFailure("candidate_protocol_error: exactly one full-game Wizened Snitches trace is required")
    trace = traces[0]
    if (
        not isinstance(trace, dict)
        or trace.get("trace_id") != "rav-wizened-snitches-global-reveal-terminal-duel"
        or trace.get("passed") is not True
        or trace.get("terminal") != "Winner(PlayerId(0))"
        or trace.get("turns", 0) <= 0
        or trace.get("turns", 61) > 60
        or trace.get("rejected_policy_moves") != 0
        or trace.get("accepted_policy_moves", 0) > 2000
    ):
        raise WorkerFailure("candidate_protocol_error: Wizened Snitches full-game result is invalid")
    mechanics = trace.get("mechanics")
    if not isinstance(mechanics, dict):
        raise WorkerFailure("candidate_protocol_error: Wizened Snitches mechanics report is missing")
    source = mechanics.get("object")
    paid = mechanics.get("mana_paid")
    paid_sequence = mechanics.get("mana_paid_sequence")
    active = mechanics.get("active_reveal_tops")
    revoked = mechanics.get("reveal_counts_after_departure")
    if (
        mechanics.get("collector_number") != 75
        or mechanics.get("cast_from_shuffled_library") is not True
        or mechanics.get("mana_cost") != "{3}{U}"
        or mechanics.get("power_toughness") != [1, 3]
        or mechanics.get("flying") is not True
        or mechanics.get("removed_by_last_gasp") is not True
        or mechanics.get("terminal_winner") != 0
        or not isinstance(source, int)
        or not isinstance(mechanics.get("last_gasp_object"), int)
        or mechanics.get("last_gasp_target_object") != source
        or not isinstance(paid, list)
        or len(paid) != 4
        or paid.count("Blue") < 1
        or any(color not in {"White", "Blue", "Black", "Red", "Green", "Colorless"} for color in paid)
        or not isinstance(paid_sequence, int)
        or not isinstance(active, list)
        or len(active) != 2
        or not isinstance(revoked, list)
        or revoked != [{"viewer": 0, "count": 0}, {"viewer": 1, "count": 0}]
    ):
        raise WorkerFailure("candidate_protocol_error: Wizened Snitches identity, payment, visibility, removal, or terminal result is invalid")
    active_by_viewer = {entry.get("viewer"): entry.get("tops") for entry in active if isinstance(entry, dict)}
    if set(active_by_viewer) != {0, 1} or active_by_viewer[0] != active_by_viewer[1]:
        raise WorkerFailure("candidate_protocol_error: both players must see the same public library tops")
    tops = active_by_viewer[0]
    if (
        not isinstance(tops, list)
        or {entry.get("owner") for entry in tops if isinstance(entry, dict)} != {0, 1}
        or len(tops) != 2
        or any(not isinstance(entry, dict) or not isinstance(entry.get("object"), int) or entry["object"] <= 0 or not isinstance(entry.get("definition"), str) for entry in tops)
    ):
        raise WorkerFailure("candidate_protocol_error: active visibility must name each nonempty library top and owner")

    events = trace.get("events")
    if not isinstance(events, list) or len(events) < 12 or len(events) > 6000:
        raise WorkerFailure("candidate_protocol_error: ordered Wizened Snitches event receipts are missing")
    sequences = [event.get("sequence") for event in events if isinstance(event, dict)]
    if len(sequences) != len(events) or any(not isinstance(value, int) for value in sequences) or sequences != sorted(sequences):
        raise WorkerFailure("candidate_protocol_error: Wizened Snitches event sequence is malformed")

    def find(kind: str, predicate) -> int:
        for index, event in enumerate(events):
            if event.get("kind") == kind and predicate(event.get("canonical", "")):
                return index
        raise WorkerFailure(f"candidate_protocol_error: {kind} receipt is missing")

    paid_index = find("SpellManaPaid", lambda value: f"PlayerId(0), card: ObjectId({source})" in value)
    paid_event = events[paid_index]
    if paid_event.get("sequence") != paid_sequence or paid_event.get("facts", {}).get("mana_colors") != paid:
        raise WorkerFailure("candidate_protocol_error: typed Wizened Snitches payment receipt differs from its report")
    cast = find("SpellCast", lambda value: f"PlayerId(0), card: ObjectId({source})" in value)
    resolved = find("SpellResolved", lambda value: f"card: ObjectId({source})" in value)
    entered = find("CardMoved", lambda value: f"card: ObjectId({source})" in value and "to: Battlefield" in value)
    removal_object = mechanics["last_gasp_object"]
    removal_cast = find("SpellCast", lambda value: f"player: PlayerId(1), card: ObjectId({removal_object})" in value)
    removal_resolved = find("SpellResolved", lambda value: f"card: ObjectId({removal_object})" in value)
    departed = find("CardMoved", lambda value: f"card: ObjectId({source})" in value and "to: Graveyard" in value)
    ended = find("GameEnded", lambda value: "Some(PlayerId(0))" in value)
    if not (paid_index < cast < resolved < entered < removal_cast < removal_resolved < departed < ended):
        raise WorkerFailure("candidate_protocol_error: payment, cast, reveal source entry, removal, departure, and terminal event order is invalid")

    mutations = report.get("mutation_sensitivity")
    required = (
        "flying_removed_detected_in_game",
        "top_reveal_removed_detected_in_game",
        "reveal_scope_narrowed_detected_in_game",
        "printed_cost_changed_detected_in_game",
    )
    if not isinstance(mutations, dict) or any(mutations.get(key) is not True for key in required):
        raise WorkerFailure("candidate_protocol_error: all four Wizened Snitches gameplay mutants must be detected")


def parse_report(stdout: bytes) -> dict:
    for line in reversed(stdout.decode(errors="replace").splitlines()):
        try:
            value = json.loads(line)
        except json.JSONDecodeError:
            continue
        if value.get("schema_version") == "cardbench.magic.card-pvp.check.v1":
            validate_report(value)
            return value
    raise WorkerFailure("candidate_protocol_error: no valid Wizened Snitches PvP result")


def write_result(output_root: Path, result: dict) -> None:
    output_root.mkdir(parents=True, exist_ok=True)
    (output_root / "result.json").write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
    (output_root / "reward.txt").write_text("0.00000000\n", encoding="utf-8")


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
        raise FileNotFoundError(f"trusted Wizened Snitches authority source is missing: {authority}")
    if not shutil.which("docker"):
        raise FileNotFoundError("Docker is required for Magic candidate isolation")
    verify_workspace(workspace, manifest)
    snapshot = make_scoring_snapshot(workspace, manifest_data["files"], output_root / "candidate-workspace")
    candidate_hash = sha256((snapshot / TARGET).read_bytes())
    name = f"cardbench-magic-wizened-snitches-{uuid.uuid4().hex[:16]}"
    command = sandbox_command(snapshot, cargo_registry.resolve(), authority, name, worker_image, candidate_kind)
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
            "error": "candidate compile or Wizened Snitches reference trace failed",
            "diagnostics": stderr.decode(errors="replace")[-12000:],
        }
        write_result(output_root, result)
        return 1
    result = parse_report(stdout)
    result["candidate"] = {"kind": candidate_kind, "card_definition": "RAV-WIZENED-SNITCHES", "source_sha256": candidate_hash, "materialized_workspace_sha256": manifest_data["workspace_sha256"]}
    result["candidate_kind"] = candidate_kind
    result["starting_workspace_sha256"] = manifest_data["workspace_sha256"]
    result["sealed_authority"] = False
    result["worker_image"] = worker_image
    result["sandbox"] = {"network": "none", "rootfs": "read-only", "pids_limit": 128}
    result["harbor_reward"] = 0.0
    result["full_reward"] = False
    write_result(output_root, result)
    print(f"candidate_passed={result.get('pilot_card_passed', False)} cards={result.get('cards_passed', 0)}/291 coverage_complete=false")
    return 0 if result.get("passed") is True and result.get("pilot_card_passed") is True else 1


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--workspace", type=Path)
    parser.add_argument("--manifest", type=Path)
    parser.add_argument("--output-root", required=True, type=Path)
    parser.add_argument("--candidate-kind", choices=["agent", "reference"], default="agent")
    parser.add_argument("--report-file", type=Path, help="validate a local authority JSON-lines capture without launching Docker")
    args = parser.parse_args()
    try:
        if args.report_file:
            result = parse_report(args.report_file.read_bytes())
            result["candidate_kind"] = result.get("candidate", {}).get("kind", args.candidate_kind)
            result["sealed_authority"] = False
            result["harbor_reward"] = 0.0
            result["pilot_reward"] = 0.0
            write_result(args.output_root, result)
            return 0
        if args.workspace is None or args.manifest is None:
            raise ValueError("--workspace and --manifest are required unless --report-file is used")
        return score(args.workspace.resolve(), args.manifest.resolve(), args.output_root.resolve(), args.candidate_kind)
    except Exception as error:
        write_result(args.output_root, {
            "schema_version": "cardbench.magic.card-pvp.check.v1", "task_id": "cardbench/magic/card",
            "passed": False, "pilot_card_passed": False, "cards_passed": 0, "cards_total": 291,
            "printings_total": 306, "cards_credited": 0, "coverage_complete": False,
            "full_reward": False, "harbor_reward": 0.0, "sealed_authority": False,
            "error": f"{type(error).__name__}: {error}",
        })
        print(f"Wizened Snitches candidate verifier failed: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
