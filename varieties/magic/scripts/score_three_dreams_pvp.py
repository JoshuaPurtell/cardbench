#!/usr/bin/env python3
"""Run Three Dreams in a bounded offline candidate worker and score its trace."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import selectors
import shutil
import subprocess
import sys
import time
import uuid
from pathlib import Path

REPO = Path(__file__).resolve().parents[3]
MATERIALIZER = Path(__file__).with_name("materialize_three_dreams_task.py")
TARGET = Path("varieties/magic/sets/ravnica_city_of_guilds/src/cards/three_dreams.rs")
AUTHORITY_SOURCE = Path("varieties/magic/session/src/bin/rav_three_dreams_pvp.rs")
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
        "cp /trusted-authority.rs /work/varieties/magic/session/src/bin/rav_three_dreams_pvp.rs && "
        "cd /work && cargo run --offline --locked -j 2 --manifest-path varieties/magic/Cargo.toml "
        "-p cardbench-magic-session --bin rav-three_dreams-pvp --" + candidate_flag,
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
    raise WorkerFailure("candidate_protocol_error: no valid Three Dreams PvP result")


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
        or report.get("card") != {"collector_number": 32, "definition_id": "RAV-THREE-DREAMS", "name": "Three Dreams"}
    ):
        raise WorkerFailure("candidate_protocol_error: incomplete Three Dreams diagnostic result")
    candidate = report.get("candidate")
    if not isinstance(candidate, dict) or candidate.get("card_definition") != "RAV-THREE-DREAMS":
        raise WorkerFailure("candidate_protocol_error: wrong Three Dreams candidate identity")
    traces = report.get("traces")
    if not isinstance(traces, list) or len(traces) != 2:
        raise WorkerFailure("candidate_protocol_error: both complete Three Dreams PvP traces are required")
    by_id = {trace.get("trace_id"): trace for trace in traces if isinstance(trace, dict)}
    expected_ids = {"rav-three-dreams-select-three-distinct-auras", "rav-three-dreams-select-zero"}
    if set(by_id) != expected_ids:
        raise WorkerFailure("candidate_protocol_error: Three Dreams trace identities differ")
    validate_trace(by_id["rav-three-dreams-select-three-distinct-auras"], expected_count=3)
    validate_trace(by_id["rav-three-dreams-select-zero"], expected_count=0)
    mutation = report.get("mutation_sensitivity", {})
    required = (
        "maximum_lowered_mutation_detected_in_game",
        "duplicate_name_allowed_mutation_detected_in_game",
        "selected_reveals_suppressed_mutation_detected_in_game",
    )
    if not isinstance(mutation, dict) or any(mutation.get(key) is not True for key in required):
        raise WorkerFailure("candidate_protocol_error: Three Dreams semantic controls are incomplete")


def validate_trace(trace: object, *, expected_count: int) -> None:
    if not isinstance(trace, dict) or trace.get("terminal") != "Winner(PlayerId(0))" or trace.get("winner") != 0 or trace.get("rejected_policy_moves") != 0:
        raise WorkerFailure("candidate_protocol_error: Three Dreams did not finish a clean full game")
    mechanics = trace.get("mechanics")
    if not isinstance(mechanics, dict):
        raise WorkerFailure("candidate_protocol_error: Three Dreams mechanics are missing")
    paid = mechanics.get("paid_mana_colors")
    selected = mechanics.get("selected_aura_names")
    revealed = mechanics.get("revealed_aura_names")
    moved = mechanics.get("hand_aura_names")
    legal_count = mechanics.get("legal_aura_option_count")
    if (
        mechanics.get("mana_cost") != "{4}{W}"
        or not isinstance(paid, list)
        or len(paid) != 5
        or paid.count("White") != 1
        or sum(color != "White" for color in paid) != 4
        or not isinstance(legal_count, int)
        or legal_count < 3
        or mechanics.get("selected_count") != expected_count
        or not isinstance(selected, list)
        or not isinstance(revealed, list)
        or not isinstance(moved, list)
        or mechanics.get("private_to_controller") is not True
        or mechanics.get("hidden_from_opponent") is not True
        or mechanics.get("library_shuffled_after_search") is not True
        or mechanics.get("deterministic_replay") is not True
        or mechanics.get("terminal_combat_win") is not True
    ):
        raise WorkerFailure("candidate_protocol_error: Three Dreams payment, private search, selection, shuffle, replay, or PvP result differs")
    if expected_count == 3:
        if len(selected) != 3 or len(set(selected)) != 3 or any(not isinstance(name, str) or not name.startswith("RAV-") for name in selected):
            raise WorkerFailure("candidate_protocol_error: Three Dreams must select three distinct Aura definition IDs")
        if revealed != selected or moved != selected:
            raise WorkerFailure("candidate_protocol_error: selected Three Dreams Auras were not all revealed and moved to hand in order")
    elif selected or revealed or moved:
        raise WorkerFailure("candidate_protocol_error: zero-selection Three Dreams trace revealed or moved an Aura")

    events = trace.get("events")
    base_required = {"SpellManaPaid", "SpellCast", "DecisionOpened", "DecisionCompleted", "LibrarySearchBatchResolved", "LibraryShuffled", "SpellResolved", "GameEnded"}
    if not isinstance(events, list):
        raise WorkerFailure("candidate_protocol_error: Three Dreams event trace is missing")
    kinds = {event.get("kind") for event in events if isinstance(event, dict)}
    required = base_required | ({"CardRevealed", "CardMoved"} if expected_count else set())
    if not required.issubset(kinds):
        raise WorkerFailure("candidate_protocol_error: Three Dreams payment, private decision, search, reveal, transfer, shuffle, or terminal receipts are missing")
    sequences = [event.get("sequence") for event in events if isinstance(event, dict)]
    if any(not isinstance(sequence, int) for sequence in sequences) or sequences != sorted(sequences) or len(set(sequences)) != len(sequences):
        raise WorkerFailure("candidate_protocol_error: Three Dreams event order is malformed")

    batch = next((event for event in events if event.get("kind") == "LibrarySearchBatchResolved"), None)
    if batch is None:
        raise WorkerFailure("candidate_protocol_error: Three Dreams batch search receipt is missing")
    batch_sequence = batch["sequence"]
    shuffle = next((event for event in events if event.get("kind") == "LibraryShuffled" and "player: PlayerId(0)" in event.get("canonical", "") and event.get("sequence", -1) > batch_sequence), None)
    resolved = next((event for event in events if event.get("kind") == "SpellResolved" and event.get("sequence", -1) > (shuffle or {}).get("sequence", 1 << 62)), None)
    if resolved is None:
        # Keep the failure deterministic if no spell resolution follows the search shuffle.
        resolved_objects = []
    else:
        resolved_objects = resolved.get("facts", {}).get("objects", [])
    cast = next((event for event in events if event.get("kind") == "SpellCast" and event.get("facts", {}).get("seats") == [0] and event.get("facts", {}).get("objects") == resolved_objects and resolved_objects), None)
    payment = next((event for event in events if event.get("kind") == "SpellManaPaid" and event.get("facts", {}).get("seats") == [0] and event.get("facts", {}).get("objects") == resolved_objects and resolved_objects), None)
    opened_candidates = [event for event in events if event.get("kind") == "DecisionOpened" and "LibrarySearch" in event.get("canonical", "") and event.get("sequence", -1) < batch_sequence]
    completed_candidates = [event for event in events if event.get("kind") == "DecisionCompleted" and "LibrarySearch" in event.get("canonical", "") and event.get("sequence", -1) < batch_sequence]
    opened = max(opened_candidates, key=lambda event: event["sequence"], default=None)
    completed = max(completed_candidates, key=lambda event: event["sequence"], default=None)
    ended = next((event for event in events if event.get("kind") == "GameEnded"), None)
    if any(event is None for event in (payment, cast, opened, completed, shuffle, resolved, ended)):
        raise WorkerFailure("candidate_protocol_error: Three Dreams ordered receipt lookup failed")
    reveal_events = [event for event in events if event.get("kind") == "CardRevealed"]
    hand_moves = [event for event in events if event.get("kind") == "CardMoved" and event.get("facts", {}).get("zone") == "Hand"]
    before_selection = [payment["sequence"], cast["sequence"], opened["sequence"], completed["sequence"]]
    if before_selection != sorted(before_selection):
        raise WorkerFailure("candidate_protocol_error: Three Dreams payment and private decision order is incorrect")
    if len(reveal_events) != expected_count or len(hand_moves) != expected_count:
        raise WorkerFailure("candidate_protocol_error: Three Dreams revealed or transferred the wrong number of selected Auras")
    batch_canonical = batch.get("canonical", "")
    found_match = re.search(r"found: \[([^\]]*)\]", batch_canonical)
    search_objects = re.findall(r"ObjectId\((\d+)\)", found_match.group(1)) if found_match else None
    if "destination: Hand" not in batch_canonical or search_objects is None or len(search_objects) != expected_count:
        raise WorkerFailure("candidate_protocol_error: batch search receipt does not match the selected Aura count and hand destination")
    if expected_count:
        selected_receipts = [event["sequence"] for event in reveal_events + hand_moves]
        if min(selected_receipts) <= completed["sequence"] or max(selected_receipts) >= batch["sequence"]:
            raise WorkerFailure("candidate_protocol_error: Three Dreams reveal and hand transfers must follow the decision and precede search completion")
        revealed_definitions = [re.search(r'definition: "([^"]+)"', event.get("canonical", "")) for event in reveal_events]
        if any(match is None for match in revealed_definitions) or [match.group(1) for match in revealed_definitions if match] != selected:
            raise WorkerFailure("candidate_protocol_error: public reveal receipts do not identify the selected Aura cards in order")
        revealed_objects = [re.search(r'card: ObjectId\((\d+)\)', event.get("canonical", "")) for event in reveal_events]
        moved_objects = [re.search(r'card: ObjectId\((\d+)\)', event.get("canonical", "")) for event in hand_moves]
        if any(match is None for match in revealed_objects + moved_objects):
            raise WorkerFailure("candidate_protocol_error: Three Dreams reveal or hand-transfer receipts lack card identities")
        reveal_positions = {match.group(1): event["sequence"] for match, event in zip(revealed_objects, reveal_events) if match}
        move_positions = {match.group(1): event["sequence"] for match, event in zip(moved_objects, hand_moves) if match}
        if set(reveal_positions) != set(move_positions) or any(reveal_positions[card] >= move_positions[card] for card in reveal_positions):
            raise WorkerFailure("candidate_protocol_error: each selected Aura must be revealed before its matching move to hand")
        if search_objects != [match.group(1) for match in revealed_objects if match]:
            raise WorkerFailure("candidate_protocol_error: batch search result and public reveal receipts name different cards")
    if not (batch["sequence"] < shuffle["sequence"] < resolved["sequence"] < ended["sequence"]):
        raise WorkerFailure("candidate_protocol_error: Three Dreams search/shuffle/resolution/terminal order is incorrect")
    opened_text = opened.get("canonical", "")
    if "Private" not in opened_text or "min_selections: 0" not in opened_text or "max_selections: 3" not in opened_text:
        raise WorkerFailure("candidate_protocol_error: Three Dreams choice was not private with a legal zero-to-three selection range")
    if payment.get("facts", {}).get("mana_colors") != paid:
        raise WorkerFailure("candidate_protocol_error: Three Dreams typed payment receipt differs from the reported five-mana payment")

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
        raise FileNotFoundError(f"trusted Three Dreams authority source is missing: {authority}")
    if not shutil.which("docker"):
        raise FileNotFoundError("Docker is required for Magic candidate isolation")
    verify_workspace(workspace, manifest)
    scoring_workspace = make_scoring_snapshot(workspace, manifest_data["files"], output_root / "candidate-workspace")
    candidate_hash = sha256((scoring_workspace / TARGET).read_bytes())
    name = f"cardbench-magic-three_dreams-{uuid.uuid4().hex[:16]}"
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
            "error": "candidate compile or Three Dreams reference traces failed",
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
