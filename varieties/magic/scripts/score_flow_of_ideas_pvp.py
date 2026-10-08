#!/usr/bin/env python3
"""Run Flow of Ideas in a networkless worker and validate its PvP report."""

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
MATERIALIZER = Path(__file__).with_name("materialize_flow_of_ideas_task.py")
TARGET = Path("varieties/magic/sets/ravnica_city_of_guilds/src/cards/flow_of_ideas.rs")
AUTHORITY_SOURCE = Path("varieties/magic/session/src/bin/rav_flow_of_ideas_pvp.rs")
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
                raise WorkerFailure("candidate_timeout: Docker worker exceeded its deadline")
            for key, _ in selector.select(0.2):
                chunk = os.read(key.fileobj.fileno(), 65536)
                if not chunk:
                    selector.unregister(key.fileobj)
                    continue
                destination = stdout if key.data == "stdout" else stderr
                destination.extend(chunk)
                if len(stdout) + len(stderr) > MAX_OUTPUT_BYTES:
                    raise WorkerFailure("candidate_output_limit: worker output exceeded 2 MiB")
        return process.wait(), bytes(stdout), bytes(stderr)
    except BaseException:
        process.kill()
        process.wait()
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
        "cp /trusted-authority.rs /work/varieties/magic/session/src/bin/rav_flow_of_ideas_pvp.rs && "
        "cd /work && cargo run --offline --locked -j 2 --manifest-path varieties/magic/Cargo.toml "
        "-p cardbench-magic-session --bin rav-flow-of-ideas-pvp --" + candidate_flag,
    ]


def fail(message: str) -> None:
    raise WorkerFailure(f"candidate_protocol_error: {message}")


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
        or report.get("deterministic_replay") is not True
        or report.get("card") != {"collector_number": 50, "definition_id": "RAV-FLOW-OF-IDEAS", "name": "Flow of Ideas"}
    ):
        fail("incomplete Flow of Ideas diagnostic or incorrect RAV denominator")
    candidate = report.get("candidate")
    if not isinstance(candidate, dict) or candidate.get("card_definition") != "RAV-FLOW-OF-IDEAS":
        fail("wrong Flow of Ideas candidate identity")
    traces = report.get("traces")
    if not isinstance(traces, list) or len(traces) != 1:
        fail("one full-game Flow of Ideas trace is required")
    trace = traces[0]
    if (
        not isinstance(trace, dict)
        or trace.get("trace_id") != "rav-flow-of-ideas-draw-per-controlled-island-full-game"
        or trace.get("passed") is not True
        or trace.get("terminal") != "Winner(PlayerId(0))"
        or trace.get("winner_seat") != 0
        or trace.get("rejected_moves") != 0
        or trace.get("decks") != ["rav_flow_of_ideas_watchwolf_60", "rav_island_opponent_60"]
        or trace.get("policies") != ["rav.flow-of-ideas-island-draw.v1", "rav.flow-of-ideas-island-opponent.v1"]
        or not isinstance(trace.get("accepted_moves"), int)
        or trace["accepted_moves"] <= 0
        or not isinstance(trace.get("turns"), int)
        or trace["turns"] <= 0
    ):
        fail("Flow of Ideas did not reach a clean terminal full-game result")
    flow = trace.get("flow_of_ideas")
    if not isinstance(flow, dict) or (
        flow.get("collector_number") != 50
        or flow.get("mana_cost") != "{5}{U}"
        or flow.get("mana_paid") != ["Blue"] * 6
        or flow.get("colors") != ["Blue"]
        or flow.get("card_types") != ["Sorcery"]
        or flow.get("zone_after_resolution") != "Graveyard"
        or flow.get("controller_island_count") != 6
        or not isinstance(flow.get("opponent_island_count_ignored"), int)
        or flow["opponent_island_count_ignored"] <= 0
        or flow.get("draw_count") != 6
        or not isinstance(flow.get("cards_drawn"), list)
        or len(set(flow["cards_drawn"])) != 6
    ):
        fail("Flow identity, six-blue payment, live Island counts, or six distinct draws differ")
    events = trace.get("events")
    expected_kinds = [
        "SpellManaPaid", "SpellCast", *(["CardMoved"] * 6), "SpellResolved", "CardMoved",
        "SpellManaPaid", "SpellCast", "SpellResolved", "CardMoved", "AttackersDeclared",
        "DamageDealtToPlayer", "GameEnded",
    ]
    if not isinstance(events, list) or [event.get("kind") for event in events] != expected_kinds:
        fail("typed event trace omitted or reordered payment, draw, resolution, entry, or combat")
    sequences = [event.get("sequence") for event in events]
    if any(type(sequence) is not int for sequence in sequences) or sequences != sorted(set(sequences)):
        fail("typed event sequence is not strictly increasing")
    facts = [event.get("facts", {}) for event in events]
    flow_id = flow.get("object")
    wolf = trace.get("combat", {}).get("watchwolf")
    if (
        type(flow_id) is not int
        or facts[0].get("objects") != [flow_id]
        or facts[0].get("seats") != [0]
        or facts[0].get("amount") != 6
        or facts[0].get("mana_colors") != ["Blue"] * 6
        or facts[1].get("objects") != [flow_id]
        or any(facts[index].get("zone") != "Hand" for index in range(2, 8))
        or [facts[index].get("objects", [None])[0] for index in range(2, 8)] != flow.get("cards_drawn")
        or facts[8].get("objects") != [flow_id]
        or facts[9].get("objects") != [flow_id]
        or facts[9].get("zone") != "Graveyard"
        or facts[10].get("mana_colors") != ["Green", "White"]
        or facts[10].get("seats") != [0]
        or facts[11].get("objects") != [wolf]
        or facts[13].get("objects") != [wolf]
        or facts[13].get("zone") != "Battlefield"
        or facts[14].get("objects") != [wolf]
        or facts[15].get("objects") != [wolf]
        or facts[15].get("amount") != 3
        or facts[15].get("seats") != [1]
        or facts[16].get("seats") != [0]
    ):
        fail("event facts do not bind six draws and the follow-up terminal combat to the right objects")
    combat = trace.get("combat")
    if not isinstance(combat, dict) or (
        combat.get("power") != 3
        or combat.get("toughness") != 3
        or combat.get("total_damage_to_opponent", 0) < 20
        or combat.get("terminal_life", [1, 1])[1] > 0
    ):
        fail("terminal Watchwolf combat state is not established")
    mutation = report.get("mutation_sensitivity", {})
    required = (
        "wrong_exact_sorcery_cost_detected",
        "wrong_card_color_detected",
        "wrong_land_type_detected",
        "fixed_one_card_draw_detected",
    )
    if (
        not isinstance(mutation, dict)
        or any(mutation.get(key) is not True for key in required)
        or mutation.get("sealed_mutation_authority") is not False
    ):
        fail("compiled Flow of Ideas semantic mutation controls are incomplete")


def parse_report(stdout: bytes) -> dict:
    for line in reversed(stdout.decode(errors="replace").splitlines()):
        try:
            report = json.loads(line)
        except json.JSONDecodeError:
            continue
        if report.get("schema_version") == "cardbench.magic.card-pvp.check.v1":
            validate_report(report)
            return report
    fail("no valid Flow of Ideas PvP result")


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
        raise FileNotFoundError(f"trusted Flow of Ideas authority source is missing: {authority}")
    if not shutil.which("docker"):
        raise FileNotFoundError("Docker is required for Magic candidate isolation")
    verify_workspace(workspace, manifest)
    scoring_workspace = make_scoring_snapshot(workspace, manifest_data["files"], output_root / "candidate-workspace")
    candidate_hash = sha256((scoring_workspace / TARGET).read_bytes())
    name = f"cardbench-magic-flow-of-ideas-{uuid.uuid4().hex[:16]}"
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
            "error": "candidate compile or Flow of Ideas reference trace failed",
            "diagnostics": stderr.decode(errors="replace")[-12000:],
        }
    else:
        result = parse_report(stdout)
        result["candidate"] = {
            "kind": candidate_kind,
            "card_definition": "RAV-FLOW-OF-IDEAS",
            "source_sha256": candidate_hash,
            "materialized_workspace_sha256": manifest_data["workspace_sha256"],
        }
        result["candidate_kind"] = candidate_kind
        result["starting_workspace_sha256"] = manifest_data["workspace_sha256"]
        result["sealed_authority"] = False
        result["worker_image"] = worker_image
        result["sandbox"] = {"network": "none", "rootfs": "read-only", "pids_limit": 128}
        result["harbor_reward"] = 0.0
        result["full_reward"] = False
    (output_root / "result.json").write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
    (output_root / "reward.txt").write_text(f"{result['harbor_reward']:.8f}\n", encoding="utf-8")
    print(f"candidate_passed={result.get('pilot_card_passed', False)} cards={result.get('cards_passed', 0)}/291 coverage_complete=false")
    return 0 if result.get("passed") is True and result.get("pilot_card_passed") is True else 1


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--workspace", required=True, type=Path)
    parser.add_argument("--manifest", required=True, type=Path)
    parser.add_argument("--output-root", required=True, type=Path)
    parser.add_argument("--candidate-kind", choices=["agent", "reference"], default="agent")
    args = parser.parse_args()
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
            "sealed_authority": False,
            "error": str(error),
        }
        (args.output_root / "result.json").write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
        (args.output_root / "reward.txt").write_text("0.0\n", encoding="utf-8")
        print(f"Flow of Ideas candidate verifier failed: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
