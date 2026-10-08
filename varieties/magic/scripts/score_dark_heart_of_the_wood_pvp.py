#!/usr/bin/env python3
"""Validate Dark Heart reports and isolate an offline candidate worker."""

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
MATERIALIZER = Path(__file__).with_name("materialize_dark_heart_of_the_wood_task.py")
TARGET = Path("varieties/magic/sets/ravnica_city_of_guilds/src/cards/dark_heart_of_the_wood.rs")
AUTHORITY_SOURCE = Path("varieties/magic/session/src/bin/rav_dark_heart_of_the_wood_pvp.rs")
DEFAULT_WORKER_IMAGE = "rust@sha256:d7482085ff5b415f84dba5647ae71606650bdef00db7aeb69f4b3d1703c4082"
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


def validate_report(report: dict) -> None:
    expected_card = {"collector_number": 200, "definition_id": "RAV-DARK-HEART-OF-THE-WOOD", "name": "Dark Heart of the Wood"}
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
        or report.get("deterministic_replay") is not True
    ):
        raise WorkerFailure("candidate_protocol_error: Dark Heart report identity, replay, or zero-reward fields are invalid")
    candidate = report.get("candidate")
    if not isinstance(candidate, dict) or candidate.get("card_definition") != expected_card["definition_id"]:
        raise WorkerFailure("candidate_protocol_error: wrong Dark Heart candidate identity")
    traces = report.get("traces")
    if not isinstance(traces, list) or len(traces) != 1:
        raise WorkerFailure("candidate_protocol_error: exactly one Dark Heart full-game trace is required")
    trace = traces[0]
    mechanics = trace.get("mechanics")
    events = trace.get("events")
    if not isinstance(mechanics, dict) or not isinstance(events, list):
        raise WorkerFailure("candidate_protocol_error: Dark Heart mechanics/events are missing")
    heart = mechanics.get("heart_object")
    opponent = mechanics.get("opponent_watchwolf")
    green_source = mechanics.get("green_mana_source")
    forest = mechanics.get("sacrificed_permanent")
    cast_payment = mechanics.get("heart_mana_paid")
    sequence = mechanics.get("ordered_event_sequence")
    mutations = report.get("mutation_sensitivity")
    required_mutations = (
        "printed_cost_mutation_detected_in_game",
        "activation_cost_mutation_detected_in_game",
        "forest_type_mutation_detected_in_game",
        "life_gain_mutation_detected_in_game",
    )
    if (
        trace.get("id") != "rav-dark-heart-forest-sacrifice-three-life-pvp"
        or trace.get("terminal") != "Winner(PlayerId(0))"
        or trace.get("winner_seat") != 0
        or trace.get("rejected_moves") != 0
        or not isinstance(trace.get("seed"), int)
        or mechanics.get("heart_mana_cost") != "{B}{G}"
        or not isinstance(cast_payment, list)
        or len(cast_payment) != 2
        or set(cast_payment) != {"Black", "Green"}
        or mechanics.get("activation_mana_cost") != "{G}"
        or mechanics.get("activation_mana_paid") != "{G}"
        or mechanics.get("incoming_combat_damage") != 3
        or not isinstance(heart, int)
        or not isinstance(opponent, int)
        or not isinstance(green_source, int)
        or not isinstance(forest, int)
        or mechanics.get("sacrificed_basic_land_type") != "Forest"
        or mechanics.get("sacrifice_moved_to_graveyard") is not True
        or mechanics.get("life_at_activation") != 17
        or mechanics.get("life_after_resolution") != 20
        or mechanics.get("life_gained") != 3
        or not isinstance(sequence, list)
        or len(sequence) != 18
        or any(not isinstance(value, int) for value in sequence)
        or sequence != sorted(sequence)
        or not isinstance(mutations, dict)
        or any(mutations.get(key) is not True for key in required_mutations)
    ):
        raise WorkerFailure("candidate_protocol_error: Dark Heart costs, typed Forest sacrifice, life gain, or terminal trace is incomplete")
    if trace.get("rejected_moves") != 0 or trace.get("turns", 0) > 60:
        raise WorkerFailure("candidate_protocol_error: Dark Heart match exceeded its turn/action contract")
    if len(events) != 18:
        raise WorkerFailure("candidate_protocol_error: ordered Dark Heart receipt list is incomplete")
    by_kind: dict[str, list[dict]] = {}
    for event in events:
        if not isinstance(event, dict) or not isinstance(event.get("kind"), str) or not isinstance(event.get("facts"), dict):
            raise WorkerFailure("candidate_protocol_error: malformed projected Dark Heart event")
        by_kind.setdefault(event["kind"], []).append(event)
    for kind in ("SpellManaPaid", "SpellCast", "SpellResolved", "CardMoved", "AttackersDeclared", "DamageDealtToPlayer", "ManaAbilityActivated", "ManaAdded", "AbilityManaPaid", "SacrificedAsAbilityCost", "AbilityActivated", "AbilityResolved", "LifeGained", "GameEnded"):
        if not by_kind.get(kind):
            raise WorkerFailure(f"candidate_protocol_error: missing {kind} event")
    def find_event(kind: str, predicate):
        return next((event for event in by_kind.get(kind, []) if predicate(event)), None)
    heart_paid = find_event("SpellManaPaid", lambda event: event["facts"].get("objects") == [heart] and event["facts"].get("seats") == [0] and event["facts"].get("mana_colors") == cast_payment)
    heart_cast = find_event("SpellCast", lambda event: event["facts"].get("objects") == [heart] and event["facts"].get("seats") == [0])
    heart_resolved = find_event("SpellResolved", lambda event: event["facts"].get("objects") == [heart])
    heart_entered = find_event("CardMoved", lambda event: event["facts"].get("objects") == [heart] and event["facts"].get("zone") == "Battlefield")
    opponent_damage = find_event("DamageDealtToPlayer", lambda event: event["facts"].get("objects") == [opponent] and event["facts"].get("seats") == [0] and event["facts"].get("amount") == 3)
    mana_activation = find_event("ManaAbilityActivated", lambda event: f"player: PlayerId(0), land: ObjectId({green_source}), color: Green" in event.get("canonical", ""))
    green_added = find_event("ManaAdded", lambda event: event["facts"].get("seats") == [0] and event["facts"].get("amount") == 1 and "color: Green" in event.get("canonical", ""))
    ability_paid = find_event("AbilityManaPaid", lambda event: event["facts"].get("objects") == [heart] and "green-sacrifice-forest-gain-three-life" in event.get("canonical", "") and "mana_cost: ManaCost { generic: 0, colored: [Green]" in event.get("canonical", ""))
    sacrificed = find_event("SacrificedAsAbilityCost", lambda event: f"source: ObjectId({heart})" in event.get("canonical", "") and f"permanent: ObjectId({forest})" in event.get("canonical", ""))
    forest_moved = find_event("CardMoved", lambda event: event["facts"].get("objects") == [forest] and event["facts"].get("zone") == "Graveyard")
    activated = find_event("AbilityActivated", lambda event: event["facts"].get("objects") == [heart] and event["facts"].get("seats") == [0] and "green-sacrifice-forest-gain-three-life" in event.get("canonical", ""))
    resolved = find_event("AbilityResolved", lambda event: event["facts"].get("objects") == [heart] and "green-sacrifice-forest-gain-three-life" in event.get("canonical", ""))
    gained = find_event("LifeGained", lambda event: event["facts"].get("seats") == [0] and event["facts"].get("amount") == 3)
    terminal = find_event("GameEnded", lambda event: event["facts"].get("seats") == [0])
    if any(event is None for event in (heart_paid, heart_cast, heart_resolved, heart_entered, opponent_damage, mana_activation, green_added, ability_paid, sacrificed, forest_moved, activated, resolved, gained, terminal)):
        raise WorkerFailure("candidate_protocol_error: Dark Heart projected event provenance is incomplete")
    ordered = [heart_paid, heart_cast, heart_resolved, heart_entered, opponent_damage, mana_activation, green_added, ability_paid, sacrificed, forest_moved, activated]
    if any(ordered[index].get("sequence", -1) >= ordered[index + 1].get("sequence", -1) for index in range(len(ordered) - 1)):
        raise WorkerFailure("candidate_protocol_error: Dark Heart cast/payment/sacrifice receipts are out of order")
    if activated["sequence"] >= resolved["sequence"] or activated["sequence"] >= gained["sequence"] or resolved["sequence"] >= terminal["sequence"] or gained["sequence"] >= terminal["sequence"]:
        raise WorkerFailure("candidate_protocol_error: Dark Heart life gain or terminal result is out of order")
    if find_event("AttackersDeclared", lambda event: event["facts"].get("seats") == [1] and opponent in event["facts"].get("objects", [])) is None:
        raise WorkerFailure("candidate_protocol_error: opponent Watchwolf attack is missing")
    if find_event("AttackersDeclared", lambda event: event["facts"].get("seats") == [0] and event["facts"].get("objects")) is None:
        raise WorkerFailure("candidate_protocol_error: post-resolution seat-zero combat continuation is missing")
    event_sequences = [event.get("sequence") for event in events]
    if any(not isinstance(value, int) for value in event_sequences) or event_sequences != sorted(event_sequences):
        raise WorkerFailure("candidate_protocol_error: Dark Heart projected event order is malformed")


def parse_report(stdout: bytes) -> dict:
    for line in reversed(stdout.decode(errors="replace").splitlines()):
        try:
            report = json.loads(line)
        except json.JSONDecodeError:
            continue
        if report.get("schema_version") == "cardbench.magic.card-pvp.check.v1":
            validate_report(report)
            return report
    raise WorkerFailure("candidate_protocol_error: no valid Dark Heart PvP report")


def run_bounded(command: list[str], *, timeout: int, name: str) -> tuple[int, bytes, bytes]:
    process = subprocess.Popen(command, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    assert process.stdout is not None and process.stderr is not None
    selector = selectors.DefaultSelector()
    selector.register(process.stdout, selectors.EVENT_READ, "stdout")
    selector.register(process.stderr, selectors.EVENT_READ, "stderr")
    output, diagnostics = bytearray(), bytearray()
    started = time.monotonic()
    try:
        while selector.get_map() or process.poll() is None:
            if time.monotonic() - started > timeout:
                raise WorkerFailure("candidate_timeout: isolated Docker worker exceeded its deadline")
            for key, _ in selector.select(0.2):
                chunk = os.read(key.fileobj.fileno(), 65536)
                if not chunk:
                    selector.unregister(key.fileobj)
                    continue
                target = output if key.data == "stdout" else diagnostics
                target.extend(chunk)
                if len(output) + len(diagnostics) > MAX_OUTPUT_BYTES:
                    raise WorkerFailure("candidate_output_limit: worker output exceeded 2 MiB")
        return process.wait(), bytes(output), bytes(diagnostics)
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
        "cp -R /submission/. /work/ && cp /trusted-authority.rs /work/varieties/magic/session/src/bin/rav_dark_heart_of_the_wood_pvp.rs && "
        "cd /work && cargo run --offline --locked -j 2 --manifest-path varieties/magic/Cargo.toml "
        "-p cardbench-magic-session --bin rav-dark-heart-of-the-wood-pvp --" + candidate_flag,
    ]


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
        raise FileNotFoundError(f"trusted Dark Heart authority is missing: {authority}")
    if not shutil.which("docker"):
        raise FileNotFoundError("Docker is required for isolated Magic candidate scoring")
    verify_workspace(workspace, manifest)
    scoring_workspace = output_root / "candidate-workspace"
    if scoring_workspace.is_symlink():
        raise WorkerFailure("candidate_snapshot_path_is_symlink")
    if scoring_workspace.exists():
        shutil.rmtree(scoring_workspace)
    scoring_workspace.mkdir(parents=True)
    for relative in sorted(manifest_data["files"]):
        source = workspace / Path(relative)
        if source.is_symlink() or not source.is_file():
            raise WorkerFailure(f"candidate_snapshot_source_missing: {relative}")
        destination = scoring_workspace / Path(relative)
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(source, destination)
    candidate_hash = sha256((scoring_workspace / TARGET).read_bytes())
    name = f"cardbench-magic-dark-heart-{uuid.uuid4().hex[:16]}"
    command = sandbox_command(scoring_workspace, cargo_registry.resolve(), authority, name, worker_image, candidate_kind)
    code, stdout, stderr = run_bounded(command, timeout=900, name=name)
    (output_root / "worker.stdout").write_bytes(stdout)
    (output_root / "worker.stderr").write_bytes(stderr)
    if code == 0:
        result = parse_report(stdout)
        result["candidate"] = {"kind": candidate_kind, "card_definition": "RAV-DARK-HEART-OF-THE-WOOD", "source_sha256": candidate_hash, "materialized_workspace_sha256": manifest_data["workspace_sha256"]}
        result["candidate_kind"] = candidate_kind
        result["starting_workspace_sha256"] = manifest_data["workspace_sha256"]
    else:
        result = {"schema_version": "cardbench.magic.card-pvp.check.v1", "task_id": "cardbench/magic/card", "passed": False, "pilot_card_passed": False, "cards_passed": 0, "cards_total": 291, "printings_total": 306, "cards_credited": 0, "coverage_complete": False, "full_reward": False, "harbor_reward": 0.0, "pilot_reward": 0.0, "error": "candidate compilation or Dark Heart full-game trace failed", "diagnostics": stderr.decode(errors="replace")[-12000:]}
    result["cards_credited"] = 0
    result["coverage_complete"] = False
    result["full_reward"] = False
    result["harbor_reward"] = 0.0
    result["pilot_reward"] = 0.0
    result["sealed_authority"] = False
    (output_root / "result.json").write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
    (output_root / "reward.txt").write_text("0.00000000\n", encoding="utf-8")
    print(f"candidate_passed={result.get('pilot_card_passed', False)} cards={result.get('cards_passed', 0)}/291 reward=0.0 credited=0")
    return 0 if result.get("passed") is True and result.get("pilot_card_passed") is True else 1


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--workspace", type=Path)
    parser.add_argument("--manifest", type=Path)
    parser.add_argument("--output-root", type=Path)
    parser.add_argument("--candidate-kind", choices=["agent", "reference"], default="agent")
    parser.add_argument("--validate-report", type=Path)
    args = parser.parse_args()
    try:
        if args.validate_report:
            validate_report(json.loads(args.validate_report.read_text(encoding="utf-8")))
            print("report_valid=true pilot_reward=0.0 harbor_reward=0.0 cards_credited=0")
            return 0
        if not args.workspace or not args.manifest or not args.output_root:
            parser.error("--workspace, --manifest and --output-root are required unless --validate-report is used")
        return score(args.workspace.resolve(), args.manifest.resolve(), args.output_root.resolve(), args.candidate_kind)
    except Exception as error:
        if args.output_root:
            root = args.output_root.resolve()
            root.mkdir(parents=True, exist_ok=True)
            failure = {"schema_version": "cardbench.magic.card-pvp.check.v1", "task_id": "cardbench/magic/card", "passed": False, "pilot_card_passed": False, "cards_passed": 0, "cards_total": 291, "printings_total": 306, "cards_credited": 0, "coverage_complete": False, "full_reward": False, "harbor_reward": 0.0, "pilot_reward": 0.0, "error": str(error)}
            (root / "result.json").write_text(json.dumps(failure, indent=2) + "\n", encoding="utf-8")
            (root / "reward.txt").write_text("0.00000000\n", encoding="utf-8")
        print(f"Dark Heart verifier failed: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
