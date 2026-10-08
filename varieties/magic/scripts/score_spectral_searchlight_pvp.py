#!/usr/bin/env python3
"""Validate and score an isolated Spectral Searchlight reference/candidate run."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import shutil
import uuid
from pathlib import Path

from score_watchwolf_pvp import WorkerFailure, make_scoring_snapshot, run_bounded
from materialize_spectral_searchlight_task import verify

REPO = Path(__file__).resolve().parents[3]
AUTHORITY_SOURCE = Path("varieties/magic/session/src/bin/rav_spectral_searchlight_pvp.rs")
TARGET = Path("varieties/magic/sets/ravnica_city_of_guilds/src/cards/spectral_searchlight.rs")
DEFAULT_WORKER_IMAGE = "rust@sha256:d7482085ff5b415f84dba5647ae71606650bdef00db7aeb69f4b3d170c3e4082"


def validate_report(report: dict) -> None:
    if (
        report.get("schema_version") != "cardbench.magic.card-pvp.check.v1"
        or report.get("task_id") != "cardbench/magic/card"
        or report.get("passed") is not True
        or report.get("pilot_card_passed") is not True
        or report.get("replay_deterministic") is not True
        or report.get("cards_passed") != 1
        or report.get("cards_credited") != 0
        or report.get("cards_total") != 291
        or report.get("printings_total") != 306
        or report.get("coverage_complete") is not False
        or report.get("full_reward") is not False
        or report.get("harbor_reward") != 0.0
        or report.get("pilot_reward") != 0.0
        or report.get("viewer_scope") != "Judge"
        or report.get("card") != {
            "collector_number": 271,
            "definition_id": "RAV-SPECTRAL-SEARCHLIGHT",
            "name": "Spectral Searchlight",
        }
    ):
        raise WorkerFailure("candidate_protocol_error: incomplete Spectral Searchlight single-card report")
    mutations = report.get("mutation_sensitivity")
    required_mutations = (
        "wrong_spell_cost_mutation_detected",
        "tap_cost_removed_mutation_detected",
        "wrong_target_requirement_mutation_detected",
        "target_player_mana_effect_replaced_mutation_detected",
    )
    if not isinstance(mutations, dict) or any(mutations.get(key) is not True for key in required_mutations):
        raise WorkerFailure("candidate_protocol_error: Spectral Searchlight semantic mutation controls are incomplete")
    traces = report.get("traces")
    if not isinstance(traces, list) or len(traces) != 1:
        raise WorkerFailure("candidate_protocol_error: one Spectral Searchlight trace is required")
    trace = traces[0]
    if (
        not isinstance(trace, dict)
        or trace.get("id") != "rav-spectral-searchlight-target-player-color-choice"
        or trace.get("passed") is not True
        or trace.get("terminal") != "Winner(PlayerId(0))"
        or trace.get("winner_seat") != 0
        or trace.get("rejected_moves") != 0
        or trace.get("controller_choice_rejected") is not True
        or trace.get("recipient_choice_order")
        != ["AbilityActivated", "DecisionOpened", "DecisionCompleted", "ManaAdded", "AbilityResolved"]
    ):
        raise WorkerFailure("candidate_protocol_error: Spectral Searchlight terminal and recipient-choice trace differs")
    spell = trace.get("spell")
    activations = trace.get("activations")
    courier = trace.get("courier_hawk")
    if (
        not isinstance(spell, dict)
        or spell.get("name") != "Spectral Searchlight"
        or spell.get("mana_cost") != "{3}"
        or spell.get("payment") != ["White", "White", "White"]
        or len(spell.get("casts", [])) < 2
        or not isinstance(activations, list)
        or len(activations) < 2
        or not isinstance(courier, dict)
        or courier.get("mana_cost") != "{1}{W}"
        or courier.get("payment") != ["White", "White"]
        or courier.get("cast_and_resolved") is not True
        or courier.get("attacked") is not True
        or courier.get("damage") != 1
    ):
        raise WorkerFailure("candidate_protocol_error: Searchlight cast or Courier Hawk use is incomplete")
    first, second = activations[:2]
    if (
        not isinstance(first, dict)
        or not isinstance(second, dict)
        or first.get("source") == second.get("source")
        or [first.get("target_player"), second.get("target_player")] != [0, 1]
        or any(item.get("controller") != 0 or item.get("source_tapped") is not True or item.get("mana_added_to_target") is not True for item in (first, second))
        or [first.get("chosen_color"), second.get("chosen_color")] != ["White", "Blue"]
    ):
        raise WorkerFailure("candidate_protocol_error: distinct tapped sources and both recipient choices are required")
    events = trace.get("events")
    if not isinstance(events, list) or not all(isinstance(event, dict) for event in events):
        raise WorkerFailure("candidate_protocol_error: Spectral Searchlight event trace is malformed")
    sequences = [event.get("sequence") for event in events]
    if any(not isinstance(value, int) for value in sequences) or sequences != sorted(sequences):
        raise WorkerFailure("candidate_protocol_error: Spectral Searchlight event sequence is malformed")
    for source in (first["source"], second["source"]):
        if not any(
            event.get("kind") == "AbilityActivated"
            and event.get("facts", {}).get("objects") == [source]
            and event.get("facts", {}).get("ability") == "tap-target-player-chosen-color-mana"
            for event in events
        ):
            raise WorkerFailure("candidate_protocol_error: Searchlight activation receipt missing")
    for activation in (first, second):
        source = activation["source"]
        target = activation["target_player"]
        activated = next(
            (index for index, event in enumerate(events)
             if event.get("kind") == "AbilityActivated"
             and event.get("facts", {}).get("objects") == [source]),
            None,
        )
        opened = next(
            (index for index, event in enumerate(events)
             if event.get("kind") == "DecisionOpened"
             and f"player: PlayerId({target})" in event.get("canonical", "")),
            None,
        )
        completed = next(
            (index for index, event in enumerate(events)
             if event.get("kind") == "DecisionCompleted"
             and f"player: PlayerId({target})" in event.get("canonical", "")),
            None,
        )
        resolved = next(
            (index for index, event in enumerate(events)
             if event.get("kind") == "AbilityResolved"
             and event.get("facts", {}).get("objects") == [source]),
            None,
        )
        if None in (activated, opened, completed, resolved) or not activated < opened < completed < resolved:
            raise WorkerFailure("candidate_protocol_error: target player decision did not resolve for its activation")
    for card, colors in ((spell["casts"][0], ["White", "White", "White"]), (courier["object"], ["White", "White"])):
        if not any(
            event.get("kind") == "SpellManaPaid"
            and event.get("facts", {}).get("objects") == [card]
            and event.get("facts", {}).get("mana_colors") == colors
            for event in events
        ):
            raise WorkerFailure("candidate_protocol_error: exact spell mana payment receipt missing")
    if not any(
        event.get("kind") == "GameEnded" and event.get("facts", {}).get("seats") == [0]
        for event in events
    ):
        raise WorkerFailure("candidate_protocol_error: terminal pilot win receipt missing")


def parse_report(stdout: bytes) -> dict:
    for line in reversed(stdout.decode(errors="replace").splitlines()):
        try:
            value = json.loads(line)
        except json.JSONDecodeError:
            continue
        if isinstance(value, dict) and value.get("schema_version") == "cardbench.magic.card-pvp.check.v1":
            validate_report(value)
            return value
    raise WorkerFailure("candidate_protocol_error: no valid Spectral Searchlight PvP result")


def sandbox_command(workspace: Path, registry: Path, authority: Path, name: str, image: str, candidate_kind: str) -> list[str]:
    if authority.is_symlink() or not authority.is_file():
        raise WorkerFailure("trusted Spectral Searchlight authority source is missing")
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
        "cp /trusted-authority.rs /work/varieties/magic/session/src/bin/rav_spectral_searchlight_pvp.rs && "
        "cd /work && cargo run --offline --locked -j 2 --manifest-path varieties/magic/Cargo.toml "
        "-p cardbench-magic-session --bin rav-spectral-searchlight-pvp --" + candidate_flag,
    ]


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
        raise FileNotFoundError(f"trusted Spectral Searchlight authority source is missing: {authority}")
    if not shutil.which("docker"):
        raise FileNotFoundError("Docker is required for Magic candidate isolation")
    verify(workspace, manifest)
    scoring_workspace = make_scoring_snapshot(workspace, manifest_data["files"], output_root / "candidate-workspace")
    candidate_hash = hashlib.sha256((scoring_workspace / TARGET).read_bytes()).hexdigest()
    name = f"cardbench-magic-spectral-searchlight-{uuid.uuid4().hex[:16]}"
    code, stdout, stderr = run_bounded(
        sandbox_command(scoring_workspace, registry.resolve(), authority, name, image, candidate_kind),
        timeout=900,
        name=name,
    )
    (output_root / "worker.stdout").write_bytes(stdout)
    (output_root / "worker.stderr").write_bytes(stderr)
    if code != 0:
        report = {
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
            "pilot_reward": 0.0,
            "error": "candidate compile or Spectral Searchlight reference trace failed",
            "diagnostics": stderr.decode(errors="replace")[-12000:],
        }
    else:
        report = parse_report(stdout)
        report["candidate"] = {"kind": candidate_kind, "card_definition": "RAV-SPECTRAL-SEARCHLIGHT", "source_sha256": candidate_hash, "materialized_workspace_sha256": manifest_data["workspace_sha256"]}
        report["candidate_kind"] = candidate_kind
        report["starting_workspace_sha256"] = manifest_data["workspace_sha256"]
        report["sealed_authority"] = False
        report["worker_image"] = image
        report["sandbox"] = {"network": "none", "rootfs": "read-only", "pids_limit": 128}
        report["harbor_reward"] = 0.0
        report["pilot_reward"] = 0.0
        report["full_reward"] = False
    (output_root / "result.json").write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    (output_root / "reward.txt").write_text("0.00000000\n", encoding="utf-8")
    print(f"candidate_passed={report.get('pilot_card_passed', False)} cards={report.get('cards_passed', 0)}/291 coverage_complete=false")
    return 0 if report.get("pilot_card_passed") is True else 1


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--workspace", type=Path)
    parser.add_argument("--manifest", type=Path)
    parser.add_argument("--output", type=Path)
    parser.add_argument("--candidate-kind", choices=("reference", "agent"), default="agent")
    parser.add_argument("--validate-report", type=Path)
    args = parser.parse_args()
    if args.validate_report:
        report = json.loads(args.validate_report.read_text(encoding="utf-8"))
        validate_report(report)
        print("Spectral Searchlight report valid; all diagnostic credits and rewards are zero")
        return 0
    if not args.workspace or not args.manifest or not args.output:
        parser.error("--workspace, --manifest, and --output are required unless --validate-report is used")
    return score(args.workspace.resolve(), args.manifest.resolve(), args.output.resolve(), args.candidate_kind)


if __name__ == "__main__":
    raise SystemExit(main())
