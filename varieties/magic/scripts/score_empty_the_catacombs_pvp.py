#!/usr/bin/env python3
"""Score Empty the Catacombs in a bounded, offline isolated Rust worker."""

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
MATERIALIZER = Path(__file__).with_name("materialize_empty_the_catacombs_task.py")
TARGET = Path("varieties/magic/sets/ravnica_city_of_guilds/src/cards/empty_the_catacombs.rs")
AUTHORITY = Path("varieties/magic/session/src/bin/rav_empty_the_catacombs_pvp.rs")
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
        raise WorkerFailure("trusted_empty_the_catacombs_authority_missing")
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
        "cp /trusted-authority.rs /work/varieties/magic/session/src/bin/rav_empty_the_catacombs_pvp.rs && "
        "cd /work && cargo run --offline --locked -j 2 --manifest-path varieties/magic/Cargo.toml "
        "-p cardbench-magic-session --bin rav_empty_the_catacombs_pvp --" + candidate_flag,
    ]


def validate_trace(trace: object) -> None:
    expected_trace = "rav-empty-the-catacombs-return-all-creatures-from-each-graveyard-full-game"
    if not isinstance(trace, dict) or trace.get("trace_id") != expected_trace or trace.get("passed") is not True:
        raise WorkerFailure("candidate_protocol_error: missing Empty the Catacombs PvP trace")
    if trace.get("terminal") != "Winner(PlayerId(0))" or trace.get("winner_seat") != 0 or trace.get("rejected_moves") != 0:
        raise WorkerFailure("candidate_protocol_error: Empty the Catacombs match did not end in a clean pilot win")
    card = trace.get("catacombs")
    both = trace.get("both_players")
    combat = trace.get("combat")
    receipts = trace.get("ordered_event_receipts")
    if not all(isinstance(value, dict) for value in (card, both, combat, receipts)):
        raise WorkerFailure("candidate_protocol_error: required card, graveyard, combat, and event receipts are missing")
    if card.get("resolved_before_owner_graveyard") is not True or card.get("ends_in_owner_graveyard") is not True:
        raise WorkerFailure("candidate_protocol_error: sorcery terminal-zone ordering is incorrect")
    if card.get("mana_cost") != "{3}{B}" or card.get("card_colors") != ["Black"]:
        raise WorkerFailure("candidate_protocol_error: Empty the Catacombs identity or color is incorrect")
    payment = card.get("mana_paid")
    if not isinstance(payment, list) or len(payment) != 4 or payment.count("Black") != 1:
        raise WorkerFailure("candidate_protocol_error: exact four-mana payment including black is missing")
    yards = both.get("graveyards_at_cast")
    creatures = both.get("creatures_died")
    returns = both.get("returned_to_hand")
    remaining = both.get("creatures_left_in_graveyards")
    rain = both.get("rain_spell")
    if not all(isinstance(value, list) and len(value) == 2 for value in (yards, creatures, remaining)) or not isinstance(returns, list) or not isinstance(rain, int):
        raise WorkerFailure("candidate_protocol_error: both players' exact graveyard setup is missing")
    if any(not isinstance(yards[i], list) or not isinstance(creatures[i], list) or len(creatures[i]) < 2 or any(card_id not in yards[i] for card_id in creatures[i]) for i in range(2)):
        raise WorkerFailure("candidate_protocol_error: each owner needs at least two dead creature cards in their graveyard at casting")
    expected_returns = [card_id for owner_cards in creatures for card_id in owner_cards]
    if returns != expected_returns or any(not isinstance(yard, list) or yard for yard in remaining):
        raise WorkerFailure("candidate_protocol_error: every eligible creature and only those creature cards must return")
    if card.get("object") in returns or rain in returns:
        raise WorkerFailure("candidate_protocol_error: the sorcery or a noncreature card was returned by the effect")
    if trace.get("public_creature_choice_opened") is not False:
        raise WorkerFailure("candidate_protocol_error: the all-creature effect requested a card selection")
    cast = receipts.get("cast")
    resolved = receipts.get("spell_resolved")
    own_graveyard = receipts.get("spell_to_owners_graveyard")
    seat0 = receipts.get("seat_zero_creature_returns")
    seat1 = receipts.get("seat_one_creature_returns")
    if not all(isinstance(value, int) for value in (cast, resolved, own_graveyard)) or not isinstance(seat0, list) or not isinstance(seat1, list):
        raise WorkerFailure("candidate_protocol_error: ordered event indices are malformed")
    if len(seat0) != len(creatures[0]) or len(seat1) != len(creatures[1]) or not (cast < resolved < own_graveyard and all(cast < index < resolved for index in seat0) and all(cast < index < resolved for index in seat1) and max(seat0) < min(seat1)):
        raise WorkerFailure("candidate_protocol_error: per-owner creature returns must precede resolution and spell cleanup")
    if not isinstance(combat.get("attacking_turns"), list) or not combat["attacking_turns"] or not isinstance(combat.get("damage_to_opponent"), int) or combat["damage_to_opponent"] <= 0:
        raise WorkerFailure("candidate_protocol_error: PvP game did not continue through combat")
    if not isinstance(trace.get("turns"), int) or trace["turns"] <= 0 or not isinstance(trace.get("accepted_moves"), int) or trace["accepted_moves"] <= 0:
        raise WorkerFailure("candidate_protocol_error: full-game bounds are missing")


def validate_report(report: dict) -> None:
    expected_card = {"collector_number": 86, "printings": 1, "definition_id": "RAV-EMPTY-THE-CATACOMBS", "name": "Empty the Catacombs"}
    if (report.get("schema_version") != "cardbench.magic.card-pvp.check.v1" or report.get("task_id") != "cardbench/magic/card" or report.get("expansion") != "RAV" or report.get("passed") is not True or report.get("pilot_card_passed") is not True or report.get("replay_deterministic") is not True or report.get("cards_passed") != 1 or report.get("cards_credited") != 0 or report.get("cards_total") != 291 or report.get("printings_total") != 306 or report.get("coverage_complete") is not False or report.get("full_reward") is not False or report.get("harbor_reward") != 0.0 or report.get("pilot_reward") != 0.0 or report.get("viewer_scope") != "Judge" or report.get("card") != expected_card):
        raise WorkerFailure("candidate_protocol_error: incomplete Empty the Catacombs zero-credit diagnostic report")
    source_guard = report.get("candidate_source_validation")
    if not isinstance(source_guard, dict) or source_guard.get("direct_literal_ast_checked_before_candidate_calls") is not True or source_guard.get("guard_ran_before_candidate_function_calls") is not True:
        raise WorkerFailure("candidate_protocol_error: direct-literal candidate source guard is missing")
    traces = report.get("traces")
    if not isinstance(traces, list) or len(traces) != 1:
        raise WorkerFailure("candidate_protocol_error: exactly one Empty the Catacombs full-game trace is required")
    validate_trace(traces[0])
    mutation = report.get("mutation_sensitivity")
    required = ("wrong-cost-mutation-detected-in-game", "wrong-color-mutation-detected-in-game", "missing-all-creature-return-mutation-detected-in-game")
    if not isinstance(mutation, dict) or any(mutation.get(key) is not True for key in required):
        raise WorkerFailure("candidate_protocol_error: Empty the Catacombs gameplay mutations are incomplete")
    breakdown = report.get("per_card_breakdown")
    if not isinstance(breakdown, list) or sum(1 for row in breakdown if isinstance(row, dict) and row.get("collector_number") == 86 and row.get("name") == "Empty the Catacombs" and row.get("definition_id") == "RAV-EMPTY-THE-CATACOMBS" and row.get("trace_status") == "passed") != 1:
        raise WorkerFailure("candidate_protocol_error: RAV #86 zero-credit catalog diagnostic is missing")


def parse_report(stdout: bytes) -> dict:
    for line in reversed(stdout.decode(errors="replace").splitlines()):
        try:
            report = json.loads(line)
        except json.JSONDecodeError:
            continue
        if report.get("schema_version") == "cardbench.magic.card-pvp.check.v1":
            validate_report(report)
            return report
    raise WorkerFailure("candidate_protocol_error: no valid Empty the Catacombs PvP report")


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
        raise FileNotFoundError(f"trusted Empty the Catacombs authority is missing: {authority}")
    if not shutil.which("docker"):
        raise FileNotFoundError("Docker is required for Magic candidate isolation")
    verify_workspace(workspace, manifest)
    snapshot = snapshot_workspace(workspace, data["files"], output_root / "candidate-workspace")
    digest = sha256((snapshot / TARGET).read_bytes())
    container = f"cardbench-magic-empty-the-catacombs-{uuid.uuid4().hex[:16]}"
    code, stdout, stderr = run_bounded(sandbox_command(snapshot, registry.resolve(), authority, container, image, candidate_kind), 900, container)
    (output_root / "worker.stdout").write_bytes(stdout)
    (output_root / "worker.stderr").write_bytes(stderr)
    if code != 0:
        result = {"schema_version":"cardbench.magic.card-pvp.check.v1","task_id":"cardbench/magic/card","candidate":{"kind":candidate_kind,"source_sha256":digest},"passed":False,"pilot_card_passed":False,"cards_passed":0,"cards_total":291,"printings_total":306,"cards_credited":0,"coverage_complete":False,"full_reward":False,"harbor_reward":0.0,"pilot_reward":0.0,"sealed_authority":False,"worker_image":image,"sandbox":{"network":"none","rootfs":"read-only","pids_limit":128},"error":"candidate compile or Empty the Catacombs all-creature-return trace failed","diagnostics":stderr.decode(errors="replace")[-12000:]}
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
