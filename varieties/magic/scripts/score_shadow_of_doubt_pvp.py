#!/usr/bin/env python3
"""Run Shadow of Doubt in a bounded offline candidate worker and score its PvP trace."""

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
MATERIALIZER = Path(__file__).with_name("materialize_shadow_of_doubt_task.py")
TARGET = Path("varieties/magic/sets/ravnica_city_of_guilds/src/cards/shadow_of_doubt.rs")
AUTHORITY_SOURCE = Path("varieties/magic/session/src/bin/rav_shadow_of_doubt_pvp.rs")
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
        "cp /trusted-authority.rs /work/varieties/magic/session/src/bin/rav_shadow_of_doubt_pvp.rs && "
        "cd /work && cargo run --offline --locked -j 2 --manifest-path varieties/magic/Cargo.toml "
        "-p cardbench-magic-session --bin rav-shadow-of-doubt-pvp --" + candidate_flag,
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
    raise WorkerFailure("candidate_protocol_error: no valid Shadow of Doubt PvP result")


def validate_report(report: dict) -> None:
    if (
        report.get("task_id") != "cardbench/magic/card"
        or report.get("passed") is not True
        or report.get("pilot_card_passed") is not True
        or report.get("deterministic_replay") is not True
        or report.get("cards_passed") != 1
        or report.get("cards_credited") != 0
        or report.get("cards_total") != 291
        or report.get("printings_total") != 306
        or report.get("coverage_complete") is not False
        or report.get("full_reward") is not False
        or report.get("harbor_reward") != 0.0
        or report.get("pilot_reward") != 0.0
        or report.get("viewer_scope") != "Judge"
        or report.get("card") != {"collector_number": 253, "definition_id": "RAV-SHADOW-OF-DOUBT", "name": "Shadow of Doubt"}
    ):
        raise WorkerFailure("candidate_protocol_error: incomplete Shadow of Doubt 1/291 diagnostic result")
    candidate = report.get("candidate")
    if not isinstance(candidate, dict) or candidate.get("card_definition") != "RAV-SHADOW-OF-DOUBT":
        raise WorkerFailure("candidate_protocol_error: wrong Shadow of Doubt candidate identity")
    traces = report.get("traces")
    if not isinstance(traces, list) or len(traces) != 1:
        raise WorkerFailure("candidate_protocol_error: one complete Shadow of Doubt full-game trace is required")
    validate_trace(traces[0])
    mutation = report.get("mutation_sensitivity", {})
    required = (
        "omitted_search_prevention_detected_in_game",
        "omitted_draw_detected_in_game",
        "duplicate_draw_detected_in_game",
        "reversed_effect_order_detected_in_game",
    )
    if not isinstance(mutation, dict) or any(mutation.get(key) is not True for key in required):
        raise WorkerFailure("candidate_protocol_error: Shadow of Doubt gameplay mutation controls are incomplete")


def validate_trace(trace: object) -> None:
    if not isinstance(trace, dict) or trace.get("trace_id") != "rav-shadow-of-doubt-same-turn-search-prevention-and-expiry" or trace.get("terminal") != "Winner(PlayerId(0))" or trace.get("winner") != 0 or trace.get("rejected_policy_moves") != 0:
        raise WorkerFailure("candidate_protocol_error: Shadow of Doubt trace did not finish a clean seat-zero full game")
    mechanics = trace.get("mechanics")
    if not isinstance(mechanics, dict):
        raise WorkerFailure("candidate_protocol_error: Shadow of Doubt mechanics are missing")
    paid = mechanics.get("paid_mana_colors")
    second_turn = mechanics.get("search_after_expiry_turn")
    until_turn = mechanics.get("prevention_until_turn")
    options = mechanics.get("later_candidate_definitions")
    searches = [event for event in trace.get("events", []) if isinstance(event, dict) and event.get("kind") == "LibrarySearchResolved"]
    if (
        mechanics.get("mana_cost") != "{U/B}{U/B}"
        or not isinstance(paid, list)
        or len(paid) != 2
        or any(color not in {"Blue", "Black"} for color in paid)
        or mechanics.get("shadow_cast_turn") != until_turn
        or not isinstance(second_turn, int)
        or not isinstance(until_turn, int)
        or second_turn <= until_turn
        or len(searches) < 2
        or "found: None" not in searches[0].get("canonical", "")
        or "found: Some" not in searches[1].get("canonical", "")
        or mechanics.get("prevented_search_same_turn") is not True
        or mechanics.get("shadow_draw_count") != 1
        or mechanics.get("draw_after_prevention") is not True
        or mechanics.get("draw_before_shadow_resolution") is not True
        or mechanics.get("later_search_found") is not True
        or mechanics.get("later_selected_definition") != "RAV-MOUNTAIN"
        or mechanics.get("later_selected_mountain") is not True
        or mechanics.get("later_selected_tapped_at_entry") is not True
        or mechanics.get("later_selection_private_to_controller") is not True
        or mechanics.get("later_selection_hidden_from_opponent") is not True
        or not isinstance(options, list)
        or "RAV-MOUNTAIN" not in options
        or mechanics.get("forest_excluded_from_later_candidates") is not True
        or mechanics.get("shuffle_after_each_search") != [True, True]
        or mechanics.get("combat_to_terminal_win") is not True
    ):
        raise WorkerFailure("candidate_protocol_error: Shadow effect order, same-turn prevention, expiration, draw, private search, or PvP result differs")
    events = trace.get("events")
    required_kinds = {"SpellManaPaid", "SpellCast", "SpellResolved", "LibrarySearchesPrevented", "CardMoved", "LibrarySearchResolved", "LibraryShuffled", "DecisionOpened", "DecisionCompleted", "AttackersDeclared", "DamageDealtToPlayer", "GameEnded"}
    if not isinstance(events, list) or not required_kinds.issubset({event.get("kind") for event in events if isinstance(event, dict)}):
        raise WorkerFailure("candidate_protocol_error: Shadow casting, draw, search, shuffle, or terminal combat receipts are missing")
    if sum(event.get("kind") == "DecisionOpened" for event in events) != 1 or sum(event.get("kind") == "DecisionCompleted" for event in events) != 1:
        raise WorkerFailure("candidate_protocol_error: only the later unprevented Farseek may open and complete a private search decision")
    sequences = [event.get("sequence") for event in events if isinstance(event, dict)]
    if any(not isinstance(sequence, int) for sequence in sequences) or sequences != sorted(sequences) or len(set(sequences)) != len(sequences):
        raise WorkerFailure("candidate_protocol_error: Shadow of Doubt event order is malformed")
    prevent_i = next(i for i, event in enumerate(events) if event.get("kind") == "LibrarySearchesPrevented")
    first_search_i = next(i for i, event in enumerate(events) if event.get("kind") == "LibrarySearchResolved")
    if not (prevent_i < first_search_i) or "until_turn" not in events[prevent_i].get("canonical", ""):
        raise WorkerFailure("candidate_protocol_error: current-turn search prevention receipt is missing or out of order")
    opened = next(event for event in events if event.get("kind") == "DecisionOpened")
    if "visibility: Private" not in opened.get("canonical", ""):
        raise WorkerFailure("candidate_protocol_error: later library search decision was not private")


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
        raise FileNotFoundError(f"trusted Shadow of Doubt authority source is missing: {authority}")
    if not shutil.which("docker"):
        raise FileNotFoundError("Docker is required for Magic candidate isolation")
    verify_workspace(workspace, manifest)
    scoring_workspace = make_scoring_snapshot(workspace, manifest_data["files"], output_root / "candidate-workspace")
    candidate_hash = sha256((scoring_workspace / TARGET).read_bytes())
    name = f"cardbench-magic-shadow-of-doubt-{uuid.uuid4().hex[:16]}"
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
            "error": "candidate compile or Shadow of Doubt reference trace failed",
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
