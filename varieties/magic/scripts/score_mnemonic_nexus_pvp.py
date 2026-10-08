#!/usr/bin/env python3
"""Score Mnemonic Nexus in a bounded, offline isolated Rust worker."""

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
MATERIALIZER = Path(__file__).with_name("materialize_mnemonic_nexus_task.py")
TARGET = Path("varieties/magic/sets/ravnica_city_of_guilds/src/cards/mnemonic_nexus.rs")
AUTHORITY = Path("varieties/magic/session/src/bin/rav_mnemonic_nexus_pvp.rs")
IMAGE = "rust@sha256:d7482085ff5b415f84dba5647ae71606650bdef00db7aeb69f4b3d170c3e4082"
MAX_OUTPUT = 2 * 1024 * 1024


class WorkerFailure(RuntimeError):
    pass


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def verify_workspace(workspace: Path, manifest: Path) -> None:
    subprocess.run([sys.executable, str(MATERIALIZER), "--workspace", str(workspace), "--manifest", str(manifest), "--verify"], check=True, cwd=REPO)


def snapshot_workspace(workspace: Path, files: dict[str, str], destination: Path) -> Path:
    if destination.is_symlink():
        raise WorkerFailure("candidate_snapshot_path_is_symlink")
    if destination.exists():
        shutil.rmtree(destination)
    destination.mkdir(parents=True)
    for relative in sorted(files):
        source = workspace / Path(relative)
        if source.is_symlink() or not source.is_file():
            raise WorkerFailure(f"candidate_snapshot_source_missing: {relative}")
        target = destination / Path(relative)
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(source, target)
    return destination


def run_bounded(command: list[str], timeout: int, container: str) -> tuple[int, bytes, bytes]:
    process = subprocess.Popen(command, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    assert process.stdout is not None and process.stderr is not None
    selector = selectors.DefaultSelector()
    selector.register(process.stdout, selectors.EVENT_READ, "stdout")
    selector.register(process.stderr, selectors.EVENT_READ, "stderr")
    stdout, stderr = bytearray(), bytearray()
    started = time.monotonic()
    try:
        while selector.get_map() or process.poll() is None:
            if time.monotonic() - started > timeout:
                raise WorkerFailure("candidate_timeout: offline authority exceeded its deadline")
            for key, _ in selector.select(0.2):
                chunk = os.read(key.fileobj.fileno(), 65536)
                if not chunk:
                    selector.unregister(key.fileobj)
                    continue
                (stdout if key.data == "stdout" else stderr).extend(chunk)
                if len(stdout) + len(stderr) > MAX_OUTPUT:
                    raise WorkerFailure("candidate_output_limit: worker output exceeded 2 MiB")
        return process.wait(), bytes(stdout), bytes(stderr)
    except BaseException:
        process.kill()
        process.wait()
        subprocess.run(["docker", "rm", "-f", container], capture_output=True, check=False)
        raise


def sandbox_command(workspace: Path, registry: Path, authority: Path, container: str, image: str, candidate_kind: str) -> list[str]:
    if authority.is_symlink() or not authority.is_file():
        raise WorkerFailure("trusted_mnemonic_nexus_authority_missing")
    candidate_flag = " --candidate" if candidate_kind == "agent" else ""
    return [
        "docker", "run", "--rm", "--name", container, "--network", "none", "--read-only",
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
        "cp /trusted-authority.rs /work/varieties/magic/session/src/bin/rav_mnemonic_nexus_pvp.rs && "
        "cd /work && cargo run --offline --locked -j 2 --manifest-path varieties/magic/Cargo.toml "
        "-p cardbench-magic-session --bin rav_mnemonic_nexus_pvp --" + candidate_flag,
    ]


def validate_trace(trace: object) -> None:
    if not isinstance(trace, dict) or trace.get("trace_id") != "rav-mnemonic-nexus-per-player-graveyard-library-shuffle-full-game" or trace.get("passed") is not True:
        raise WorkerFailure("candidate_protocol_error: missing Mnemonic Nexus PvP trace")
    if trace.get("terminal") != "Winner(PlayerId(0))" or trace.get("winner_seat") != 0 or trace.get("rejected_moves") != 0:
        raise WorkerFailure("candidate_protocol_error: Mnemonic Nexus match did not end in a clean pilot win")
    nexus = trace.get("nexus")
    both = trace.get("both_players")
    combat = trace.get("combat")
    if not isinstance(nexus, dict) or not isinstance(both, dict) or not isinstance(combat, dict):
        raise WorkerFailure("candidate_protocol_error: required spell, ownership, and combat receipts are missing")
    if nexus.get("resolved_before_owner_graveyard") is not True or nexus.get("ends_in_owner_graveyard") is not True:
        raise WorkerFailure("candidate_protocol_error: resolving instant ordering is incorrect")
    if nexus.get("mana_cost") != "{3}{U}" or nexus.get("card_colors") != ["Blue"]:
        raise WorkerFailure("candidate_protocol_error: Mnemonic Nexus identity or color is incorrect")
    payment = nexus.get("mana_paid")
    if not isinstance(payment, list) or len(payment) != 4 or payment.count("Blue") != 1:
        raise WorkerFailure("candidate_protocol_error: exact four-mana payment including blue is missing")
    yards, rain_cards, moves, shuffle_order = (both.get(key) for key in ("graveyards_at_cast", "rain_cards", "library_moves", "shuffle_order"))
    if not isinstance(yards, list) or len(yards) != 2 or not isinstance(rain_cards, list) or len(rain_cards) != 2 or any(card is None for card in rain_cards):
        raise WorkerFailure("candidate_protocol_error: each player must have a distinct graveyard contribution before the spell resolves")
    if any(not isinstance(yard, list) or rain_cards[index] not in yard for index, yard in enumerate(yards)):
        raise WorkerFailure("candidate_protocol_error: graveyard contributions are not owner-relative")
    expected_moves = [card for yard in yards for card in yard]
    if moves != expected_moves or shuffle_order != ["PlayerId(0)", "PlayerId(1)"]:
        raise WorkerFailure("candidate_protocol_error: exact per-owner graveyard recovery and both library shuffles are required")
    if nexus.get("object") in moves:
        raise WorkerFailure("candidate_protocol_error: Mnemonic Nexus was incorrectly shuffled by its own effect")
    receipts = trace.get("ordered_event_receipts")
    if not isinstance(receipts, dict):
        raise WorkerFailure("candidate_protocol_error: ordered event receipts are missing")
    cast, seat0_shuffle, seat1_shuffle, resolved, own_graveyard = (receipts.get(key) for key in ("cast", "seat_zero_library_shuffle", "seat_one_library_shuffle", "spell_resolved", "spell_to_owners_graveyard"))
    seat0_moves, seat1_moves = receipts.get("seat_zero_graveyard_to_library_moves"), receipts.get("seat_one_graveyard_to_library_moves")
    if not all(isinstance(value, int) for value in (cast, seat0_shuffle, seat1_shuffle, resolved, own_graveyard)) or not isinstance(seat0_moves, list) or not isinstance(seat1_moves, list):
        raise WorkerFailure("candidate_protocol_error: event indices are malformed")
    if not (cast < seat0_shuffle < seat1_shuffle < resolved < own_graveyard and all(cast < index < seat0_shuffle for index in seat0_moves) and all(seat0_shuffle < index < seat1_shuffle for index in seat1_moves)):
        raise WorkerFailure("candidate_protocol_error: zone movement, each shuffle, resolution, and instant cleanup are out of order")
    if not isinstance(combat.get("attacking_turns"), list) or not combat["attacking_turns"] or not isinstance(combat.get("damage_to_opponent"), int) or combat["damage_to_opponent"] <= 0:
        raise WorkerFailure("candidate_protocol_error: PvP game did not continue through combat")
    if not isinstance(trace.get("turns"), int) or trace["turns"] <= 0 or not isinstance(trace.get("accepted_moves"), int) or trace["accepted_moves"] <= 0:
        raise WorkerFailure("candidate_protocol_error: full-game bounds are missing")


def validate_report(report: dict) -> None:
    expected_card = {"collector_number": 59, "printings": 1, "definition_id": "RAV-MNEMONIC-NEXUS", "name": "Mnemonic Nexus"}
    if (report.get("schema_version") != "cardbench.magic.card-pvp.check.v1" or report.get("task_id") != "cardbench/magic/card" or report.get("expansion") != "RAV" or report.get("passed") is not True or report.get("pilot_card_passed") is not True or report.get("replay_deterministic") is not True or report.get("cards_passed") != 1 or report.get("cards_credited") != 0 or report.get("cards_total") != 291 or report.get("printings_total") != 306 or report.get("coverage_complete") is not False or report.get("full_reward") is not False or report.get("harbor_reward") != 0.0 or report.get("pilot_reward") != 0.0 or report.get("viewer_scope") != "Judge" or report.get("card") != expected_card):
        raise WorkerFailure("candidate_protocol_error: incomplete Mnemonic Nexus zero-credit diagnostic report")
    source_guard = report.get("candidate_source_validation")
    if not isinstance(source_guard, dict) or source_guard.get("direct_literal_ast_checked_before_candidate_calls") is not True or source_guard.get("guard_ran_before_candidate_function_calls") is not True:
        raise WorkerFailure("candidate_protocol_error: direct-literal candidate source guard is missing")
    traces = report.get("traces")
    if not isinstance(traces, list) or len(traces) != 1:
        raise WorkerFailure("candidate_protocol_error: exactly one Mnemonic Nexus full-game trace is required")
    validate_trace(traces[0])
    mutation = report.get("mutation_sensitivity")
    required = ("wrong-cost-mutation-detected-in-game", "wrong-color-mutation-detected-in-game", "missing-shuffle-effect-mutation-detected-in-game")
    if not isinstance(mutation, dict) or any(mutation.get(key) is not True for key in required):
        raise WorkerFailure("candidate_protocol_error: Mnemonic Nexus gameplay mutations are incomplete")
    breakdown = report.get("per_card_breakdown")
    if not isinstance(breakdown, list) or sum(1 for row in breakdown if isinstance(row, dict) and row.get("collector_number") == 59 and row.get("name") == "Mnemonic Nexus" and row.get("definition_id") == "RAV-MNEMONIC-NEXUS" and row.get("trace_status") == "passed") != 1:
        raise WorkerFailure("candidate_protocol_error: RAV #59 zero-credit catalog diagnostic is missing")


def parse_report(stdout: bytes) -> dict:
    for line in reversed(stdout.decode(errors="replace").splitlines()):
        try:
            report = json.loads(line)
        except json.JSONDecodeError:
            continue
        if report.get("schema_version") == "cardbench.magic.card-pvp.check.v1":
            validate_report(report)
            return report
    raise WorkerFailure("candidate_protocol_error: no valid Mnemonic Nexus PvP report")


def score(workspace: Path, manifest: Path, output_root: Path, candidate_kind: str) -> int:
    output_root.mkdir(parents=True, exist_ok=True)
    data = json.loads(manifest.read_text(encoding="utf-8"))
    image = os.environ.get("CARDBENCH_MAGIC_WORKER_IMAGE", IMAGE)
    if not (image.startswith("rust@sha256:") or image.startswith("sha256:")):
        raise ValueError("CARDBENCH_MAGIC_WORKER_IMAGE must be an immutable digest reference")
    registry = Path.home() / ".cargo" / "registry"
    if not registry.is_dir():
        raise FileNotFoundError(f"offline Cargo registry cache is missing: {registry}")
    authority = REPO / AUTHORITY
    if authority.is_symlink() or not authority.is_file():
        raise FileNotFoundError(f"trusted Mnemonic Nexus authority is missing: {authority}")
    if not shutil.which("docker"):
        raise FileNotFoundError("Docker is required for Magic candidate isolation")
    verify_workspace(workspace, manifest)
    snapshot = snapshot_workspace(workspace, data["files"], output_root / "candidate-workspace")
    digest = sha256((snapshot / TARGET).read_bytes())
    container = f"cardbench-magic-mnemonic-nexus-{uuid.uuid4().hex[:16]}"
    code, stdout, stderr = run_bounded(sandbox_command(snapshot, registry.resolve(), authority, container, image, candidate_kind), 900, container)
    (output_root / "worker.stdout").write_bytes(stdout)
    (output_root / "worker.stderr").write_bytes(stderr)
    if code != 0:
        result = {"schema_version":"cardbench.magic.card-pvp.check.v1","task_id":"cardbench/magic/card","candidate":{"kind":candidate_kind,"source_sha256":digest},"passed":False,"pilot_card_passed":False,"cards_passed":0,"cards_total":291,"printings_total":306,"cards_credited":0,"coverage_complete":False,"full_reward":False,"harbor_reward":0.0,"pilot_reward":0.0,"sealed_authority":False,"worker_image":image,"sandbox":{"network":"none","rootfs":"read-only","pids_limit":128},"error":"candidate compile or Mnemonic Nexus reference trace failed","diagnostics":stderr.decode(errors="replace")[-12000:]}
        (output_root / "result.json").write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
        print(json.dumps(result))
        return 1
    report = parse_report(stdout)
    report["candidate"].update({"kind":candidate_kind,"source_sha256":digest})
    report.update({"sealed_authority":False,"worker_image":image,"sandbox":{"network":"none","rootfs":"read-only","pids_limit":128}})
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
