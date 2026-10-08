#!/usr/bin/env python3
"""Run Lore Broker in a bounded offline worker and validate its full-game trace."""

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
MATERIALIZER = Path(__file__).with_name("materialize_lore_broker_task.py")
TARGET = Path("varieties/magic/sets/ravnica_city_of_guilds/src/cards/lore_broker.rs")
AUTHORITY = Path("varieties/magic/session/src/bin/rav_lore_broker_pvp.rs")
IMAGE = "rust@sha256:d7482085ff5b415f84dba5647ae71606650bdef00db7aeb69f4b3d170c3e4082"
MAX_OUTPUT = 2 * 1024 * 1024


class WorkerFailure(RuntimeError):
    pass


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def verify_workspace(workspace: Path, manifest: Path) -> None:
    subprocess.run([sys.executable, str(MATERIALIZER), "--workspace", str(workspace), "--manifest", str(manifest), "--verify"], check=True, cwd=REPO)


def snapshot_workspace(workspace: Path, files: dict[str, str], root: Path) -> Path:
    if root.is_symlink():
        raise WorkerFailure("candidate_snapshot_path_is_symlink")
    if root.exists():
        shutil.rmtree(root)
    root.mkdir(parents=True)
    for relative in sorted(files):
        source = workspace / relative
        if source.is_symlink() or not source.is_file():
            raise WorkerFailure(f"candidate_snapshot_source_missing: {relative}")
        destination = root / relative
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(source, destination)
    return root


def run_bounded(command: list[str], timeout: int, container: str) -> tuple[int, bytes, bytes]:
    process = subprocess.Popen(command, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    assert process.stdout is not None and process.stderr is not None
    selector = selectors.DefaultSelector()
    selector.register(process.stdout, selectors.EVENT_READ, "stdout")
    selector.register(process.stderr, selectors.EVENT_READ, "stderr")
    out, err = bytearray(), bytearray()
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
                (out if key.data == "stdout" else err).extend(chunk)
                if len(out) + len(err) > MAX_OUTPUT:
                    raise WorkerFailure("candidate_output_limit: worker output exceeded 2 MiB")
        return process.wait(), bytes(out), bytes(err)
    except BaseException:
        process.kill()
        process.wait()
        subprocess.run(["docker", "rm", "-f", container], capture_output=True, check=False)
        raise


def sandbox_command(workspace: Path, registry: Path, authority: Path, container: str, image: str, candidate: bool) -> list[str]:
    if authority.is_symlink() or not authority.is_file():
        raise WorkerFailure("trusted_authority_source_missing")
    suffix = " --candidate" if candidate else ""
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
        "cp /trusted-authority.rs /work/varieties/magic/session/src/bin/rav_lore_broker_pvp.rs && "
        "cd /work && cargo run --offline --locked -j 2 --manifest-path varieties/magic/Cargo.toml "
        "-p cardbench-magic-session --bin rav-lore-broker-pvp --" + suffix,
    ]


def validate_trace(trace: object) -> None:
    if not isinstance(trace, dict) or trace.get("id") != "rav-lore-broker-each-player-private-draw-discard-full-game" or trace.get("passed") is not True or trace.get("terminal") != "Winner(PlayerId(0))" or trace.get("winner_seat") != 0 or trace.get("rejected_moves") != 0:
        raise WorkerFailure("candidate_protocol_error: Lore Broker did not reach a clean terminal win")
    broker, activation = trace.get("lore_broker"), trace.get("activation")
    choices, combat = trace.get("draw_discard"), trace.get("combat")
    if not all(isinstance(value, dict) for value in (broker, activation, choices, combat)):
        raise WorkerFailure("candidate_protocol_error: Lore Broker trace sections are missing")
    if (not isinstance(broker.get("object"), int) or broker.get("mana_cost") != "{1}{U}"
        or broker.get("paid_colors", []).count("Blue") < 1 or broker.get("power") != 1 or broker.get("toughness") != 2
        or activation.get("source") != broker.get("object") or activation.get("ability_id") != "each-player-draws-then-discards"
        or activation.get("count") != 1 or activation.get("tap_cost_paid_by_state") is not True or activation.get("resolved") is not True
        or choices.get("draws_by_seat_before_first_choice") != [0, 1] or choices.get("drawn_cards_still_in_hand_at_first_choice") is not True
        or choices.get("private_choice_seats") != [0, 1] or choices.get("private_to_each_chooser") != [True, True]
        or not isinstance(choices.get("candidate_counts_by_chooser"), list) or len(choices["candidate_counts_by_chooser"]) != 2
        or any(not isinstance(count, int) or count < 1 for count in choices["candidate_counts_by_chooser"])
        or choices.get("hidden_from_other_player") is not True or choices.get("candidate_ownership_matches_chooser") is not True
        or choices.get("both_players_alive_during_choices") is not True or choices.get("no_discard_before_both_choices") is not True
        or choices.get("discarded_by_seat") != [0, 1] or choices.get("discard_owners_match") is not True
        or combat.get("finisher") != "Goliath Spider" or not isinstance(combat.get("object"), int)
        or not isinstance(combat.get("attack_turns"), list) or len(combat["attack_turns"]) < 3
        or combat.get("damage_to_opponent", 0) < 21):
        raise WorkerFailure("candidate_protocol_error: identity, tap activation, draw/discard privacy, or terminal combat proof differs")
    events = trace.get("events")
    if not isinstance(events, list) or not all(isinstance(event, dict) for event in events):
        raise WorkerFailure("candidate_protocol_error: Lore Broker event stream is malformed")
    sequences = [event.get("sequence") for event in events]
    if any(not isinstance(sequence, int) for sequence in sequences) or sequences != sorted(sequences) or len(set(sequences)) != len(sequences):
        raise WorkerFailure("candidate_protocol_error: Lore Broker event order is not canonical")
    by_kind: dict[str, list[dict]] = {}
    for event in events:
        by_kind.setdefault(event.get("kind"), []).append(event)
        if event.get("kind") == "CardMoved" and event.get("facts", {}).get("zone") == "Hand":
            if event.get("facts", {}).get("objects") not in (None, []) or "<private-hand-object>" not in event.get("canonical", ""):
                raise WorkerFailure("candidate_protocol_error: a private hand object leaked in the trace")
    source = activation["source"]
    activated = next((event for event in by_kind.get("AbilityActivated", []) if event.get("facts", {}).get("objects") == [source] and event.get("facts", {}).get("ability") == "each-player-draws-then-discards"), None)
    resolved = next((event for event in by_kind.get("AbilityResolved", []) if event.get("facts", {}).get("objects") == [source] and event.get("facts", {}).get("ability") == "each-player-draws-then-discards"), None)
    discards = [event for event in by_kind.get("CardDiscarded", []) if event.get("facts", {}).get("seats") in ([0], [1])]
    ended = next((event for event in by_kind.get("GameEnded", []) if event.get("facts", {}).get("seats") == [0]), None)
    if activated is None or resolved is None or len(discards) != 2 or ended is None:
        raise WorkerFailure("candidate_protocol_error: activation, two public discards, ability resolution, or terminal event is missing")
    order = trace.get("ordered_event_sequence")
    if not isinstance(order, list) or order != sorted(order) or len(order) != 13 or any(not isinstance(item, int) for item in order):
        raise WorkerFailure("candidate_protocol_error: ordered event index proof is malformed")
    if any(index < 0 or index >= len(events) for index in order):
        raise WorkerFailure("candidate_protocol_error: ordered event index escapes the trace")
    ordered = [events[index] for index in order]
    expected_kinds = [
        "AbilityActivated", "CardMoved", "CardMoved", "DecisionOpened", "DecisionCompleted",
        "DecisionOpened", "DecisionCompleted", "CardDiscarded", "CardDiscarded", "AbilityResolved",
        "AttackersDeclared", "DamageDealtToPlayer", "GameEnded",
    ]
    if [event.get("kind") for event in ordered] != expected_kinds:
        raise WorkerFailure("candidate_protocol_error: ordered event proof does not match the required draw-choice-discard-resolution sequence")
    if (ordered[3].get("facts", {}).get("seats") != [0]
        or ordered[5].get("facts", {}).get("seats") != [1]
        or ordered[7].get("facts", {}).get("seats") != [0]
        or ordered[8].get("facts", {}).get("seats") != [1]
        or ordered[10].get("facts", {}).get("objects") is None
        or combat.get("object") not in ordered[10].get("facts", {}).get("objects", [])):
        raise WorkerFailure("candidate_protocol_error: the ordered decisions, discards, or finisher do not match their seats and source")


def validate_report(report: dict) -> None:
    expected_card = {"collector_number": 57, "definition_id": "RAV-LORE-BROKER", "name": "Lore Broker"}
    if (report.get("task_id") != "cardbench/magic/card" or report.get("passed") is not True
        or report.get("pilot_card_passed") is not True or report.get("cards_passed") != 1
        or report.get("cards_credited") != 0 or report.get("cards_total") != 291 or report.get("printings_total") != 306
        or report.get("coverage_complete") is not False or report.get("full_reward") is not False
        or report.get("harbor_reward") != 0.0 or report.get("pilot_reward") != 0.0
        or report.get("viewer_scope") != "Judge" or report.get("card") != expected_card):
        raise WorkerFailure("candidate_protocol_error: incomplete Lore Broker diagnostic result")
    candidate = report.get("candidate")
    if not isinstance(candidate, dict) or candidate.get("card_definition") != "RAV-LORE-BROKER":
        raise WorkerFailure("candidate_protocol_error: wrong Lore Broker candidate identity")
    traces = report.get("traces")
    if not isinstance(traces, list) or len(traces) != 1:
        raise WorkerFailure("candidate_protocol_error: exactly one full-game Lore Broker trace is required")
    validate_trace(traces[0])
    source_guard = report.get("source_guard_attestation")
    if (not isinstance(source_guard, dict)
        or source_guard.get("candidate_source_ast_and_literals_validated_before_candidate_calls") is not True
        or source_guard.get("compile_valid_injected_filesystem_write_rejected_before_execution") is not True
        or source_guard.get("candidate_calls_before_malicious_source_rejection") != 0):
        raise WorkerFailure("candidate_protocol_error: Lore Broker pre-call source guard attestation is incomplete")
    mutation = report.get("mutation_sensitivity")
    required = ("wrong_body_detected_in_game", "wrong_spell_cost_detected_in_game", "missing_tap_cost_detected_in_game",
                "draw_discard_effect_removed_detected_in_game", "draw_omitted_by_wrong_effect_detected_in_game",
                "compile_valid_candidate_side_effect_rejected_before_execution")
    if not isinstance(mutation, dict) or any(mutation.get(key) is not True for key in required):
        raise WorkerFailure("candidate_protocol_error: Lore Broker mutation controls are incomplete")


def parse_report(stdout: bytes) -> dict:
    for line in reversed(stdout.decode(errors="replace").splitlines()):
        try:
            report = json.loads(line)
        except json.JSONDecodeError:
            continue
        if report.get("schema_version") == "cardbench.magic.card-pvp.check.v1":
            validate_report(report)
            return report
    raise WorkerFailure("candidate_protocol_error: no Lore Broker PvP result")


def score(workspace: Path, manifest: Path, output_root: Path, candidate_kind: str) -> int:
    output_root.mkdir(parents=True, exist_ok=True)
    manifest_data = json.loads(manifest.read_text(encoding="utf-8"))
    verify_workspace(workspace, manifest)
    snapshot = snapshot_workspace(workspace, manifest_data["files"], output_root / "lore-broker-candidate-snapshot")
    registry = Path(os.environ.get("CARGO_REGISTRY", Path.home() / ".cargo" / "registry"))
    if not registry.is_dir():
        raise WorkerFailure("offline_cargo_registry_missing")
    authority = REPO / AUTHORITY
    container = "cardbench-lore-broker-" + uuid.uuid4().hex[:12]
    image = os.environ.get("CARDBENCH_MAGIC_WORKER_IMAGE", IMAGE)
    command = sandbox_command(snapshot, registry, authority, container, image, candidate_kind == "agent")
    code, stdout, stderr = run_bounded(command, timeout=300, container=container)
    report = parse_report(stdout)
    if code != 0 or report.get("passed") is not True:
        raise WorkerFailure("candidate_execution_failed: Lore Broker authority failed: " + stderr.decode(errors="replace")[-4000:])
    result = {"schema_version": "cardbench.magic.pvp-score.v1", "candidate_kind": candidate_kind,
              "cards_credited": 0, "harbor_reward": 0.0, "pilot_reward": 0.0, "qualification_status": "unreviewed_diagnostic",
              "result": report}
    (output_root / "lore-broker-pvp-score.json").write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
    print(json.dumps(result, separators=(",", ":")))
    return 0


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--workspace", required=True, type=Path)
    parser.add_argument("--manifest", required=True, type=Path)
    parser.add_argument("--output-root", required=True, type=Path)
    parser.add_argument("--candidate-kind", choices=("agent", "reference"), default="agent")
    args = parser.parse_args()
    try:
        return score(args.workspace.resolve(), args.manifest.resolve(), args.output_root.resolve(), args.candidate_kind)
    except (WorkerFailure, OSError, subprocess.CalledProcessError, json.JSONDecodeError) as error:
        print(f"lore_broker_score_failed: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
