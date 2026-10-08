#!/usr/bin/env python3
"""Validate a Peregrine Mask reference report or run the isolated candidate worker."""

from __future__ import annotations

import argparse
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
MATERIALIZER = Path(__file__).with_name("materialize_peregrine_mask_task.py")
AUTHORITY_SOURCE = Path("varieties/magic/session/src/bin/rav_peregrine_mask_pvp.rs")
DEFAULT_WORKER_IMAGE = "rust@sha256:d7482085ff5b415f84dba5647ae71606650bdef00db7aeb69f4b3d170c3e4082"
MAX_OUTPUT_BYTES = 2 * 1024 * 1024


class WorkerFailure(RuntimeError):
    pass


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


def sandbox_command(workspace: Path, registry: Path, authority: Path, name: str, image: str) -> list[str]:
    if authority.is_symlink() or not authority.is_file():
        raise WorkerFailure("trusted_authority_source_missing")
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
        "cp /trusted-authority.rs /work/varieties/magic/session/src/bin/rav_peregrine_mask_pvp.rs && "
        "cd /work && cargo run --offline --locked -j 2 --manifest-path varieties/magic/Cargo.toml "
        "-p cardbench-magic-session --bin rav-peregrine-mask-pvp -- --candidate",
    ]


def validate_report(report: dict) -> None:
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
        or report.get("card") != {"collector_number": 268, "definition_id": "RAV-PEREGRINE-MASK", "name": "Peregrine Mask"}
        or report.get("deterministic_replay") is not True
    ):
        raise WorkerFailure("candidate_protocol_error: incomplete Peregrine Mask single-card report or nonzero reward/credit")
    mutation = report.get("mutation_sensitivity")
    if not isinstance(mutation, dict) or any(
        mutation.get(key) is not True
        for key in (
            "defender_removed_mutation_detected_in_game",
            "flying_removed_mutation_detected_in_game",
            "first_strike_removed_mutation_detected_in_game",
            "equip_cost_changed_mutation_detected_in_game",
        )
    ):
        raise WorkerFailure("candidate_protocol_error: each Peregrine Mask gameplay mutation must be detected")
    traces = report.get("traces")
    if not isinstance(traces, list) or len(traces) != 1:
        raise WorkerFailure("candidate_protocol_error: exactly one full-game Peregrine Mask trace is required")
    trace = traces[0]
    if (
        not isinstance(trace, dict)
        or trace.get("id") != "rav-peregrine-mask-defender-flying-first-strike-full-game"
        or trace.get("passed") is not True
        or trace.get("terminal") != "Winner(PlayerId(0))"
        or trace.get("winner_seat") != 0
        or not isinstance(trace.get("turns"), int)
        or trace.get("turns", 0) <= 0
        or trace.get("rejected_moves") != 1
        or trace.get("mask_definition") != "RAV-PEREGRINE-MASK"
        or trace.get("mask_zone") != "Some(Battlefield)"
        or trace.get("target_definition") != "RAV-WATCHWOLF"
        or trace.get("target_zone") != "Some(Battlefield)"
        or trace.get("target_controlled_by_player0") is not True
        or trace.get("equipment_attached_to_exact_target") is not True
        or trace.get("target_can_attack_at_declaration") is not False
        or trace.get("defender_illegal_attack_probe_rejected_without_state_change") is not True
        or not {"Defender", "Flying", "FirstStrike"}.issubset(set(trace.get("target_keywords_at_equip", [])))
        or trace.get("skyknight_definition") != "RAV-SKYKNIGHT-LEGIONNAIRE"
        or trace.get("target_blocked_skyknight") is not True
        or trace.get("skyknight_destroyed_by_first_strike") is not True
        or trace.get("target_damage_to_skyknight") != 3
        or trace.get("skyknight_damage_to_target") != 0
        or trace.get("equip_mana_paid") != {"generic": 2, "colors": []}
        or not isinstance(trace.get("events_digest"), str)
    ):
        raise WorkerFailure("candidate_protocol_error: attachment, keywords, combat, payment, or terminal trace differs")
    object_ids = [trace.get("mask_object"), trace.get("target_object"), trace.get("skyknight_object")]
    if not all(isinstance(item, int) and item >= 0 for item in object_ids) or len(set(object_ids)) != 3:
        raise WorkerFailure("candidate_protocol_error: exact Mask, Watchwolf, and Skyknight objects are required")
    events = trace.get("events")
    expected_kinds = [
        "SpellManaPaid", "SpellCast", "SpellResolved", "CardMoved",
        "AbilityManaPaid", "AbilityActivated", "EquipmentAttached", "AbilityResolved",
        "AttackersDeclared", "BlockersDeclared", "DamageDealtToPermanent", "CardMoved", "GameEnded",
    ]
    if not isinstance(events, list) or [event.get("kind") for event in events] != expected_kinds:
        raise WorkerFailure("candidate_protocol_error: ordered Peregrine Mask/equip/combat event list differs")
    sequences = [event.get("sequence") for event in events]
    if any(not isinstance(item, int) for item in sequences) or sequences != sorted(sequences):
        raise WorkerFailure("candidate_protocol_error: event sequence is malformed")
    mask, target, skyknight = object_ids
    expected_canonical = [
        f"SpellManaPaid {{ player: PlayerId(0), card: ObjectId({mask}), colors: [Green] }}",
        f"SpellCast {{ player: PlayerId(0), card: ObjectId({mask}) }}",
        f"SpellResolved {{ card: ObjectId({mask}) }}",
        f"CardMoved {{ card: ObjectId({mask}), to: Battlefield }}",
        f"AbilityManaPaid {{ player: PlayerId(0), source: ObjectId({mask}), ability: \"equip-defender-flying-first-strike\", mana_cost: ManaCost {{ generic: 2, colored: [] }} }}",
        f"AbilityActivated {{ player: PlayerId(0), source: ObjectId({mask}), source_incarnation: {events[5].get('facts', {}).get('incarnation')}, definition: \"RAV-PEREGRINE-MASK\", ability: \"equip-defender-flying-first-strike\" }}",
        f"EquipmentAttached {{ equipment: ObjectId({mask}), target: ObjectId({target}), previous: None }}",
        f"AbilityResolved {{ source: ObjectId({mask}), source_incarnation: {events[7].get('facts', {}).get('incarnation')}, ability: \"equip-defender-flying-first-strike\" }}",
        f"AttackersDeclared {{ player: PlayerId(1), attackers: [ObjectId({skyknight})] }}",
        f"BlockersDeclared {{ player: PlayerId(0), assignments: [(ObjectId({skyknight}), ObjectId({target}))] }}",
        f"DamageDealtToPermanent {{ source: ObjectId({target}), permanent: ObjectId({skyknight}), amount: 3 }}",
        f"CardMoved {{ card: ObjectId({skyknight}), to: Graveyard }}",
        "GameEnded { winner: Some(PlayerId(0)) }",
    ]
    if [event.get("canonical") for event in events] != expected_canonical:
        raise WorkerFailure("candidate_protocol_error: exact equipment attachment and first-strike receipt order differs")


def parse_report(stdout: bytes) -> dict:
    for line in reversed(stdout.decode(errors="replace").splitlines()):
        try:
            value = json.loads(line)
        except json.JSONDecodeError:
            continue
        if value.get("schema_version") == "cardbench.magic.card-pvp.check.v1":
            validate_report(value)
            return value
    raise WorkerFailure("candidate_protocol_error: no valid Peregrine Mask PvP result")


def score(workspace: Path, manifest: Path, output_root: Path, candidate_kind: str) -> dict:
    output_root.mkdir(parents=True, exist_ok=True)
    manifest_data = json.loads(manifest.read_text(encoding="utf-8"))
    worker_image = os.environ.get("CARDBENCH_MAGIC_WORKER_IMAGE", DEFAULT_WORKER_IMAGE)
    if not (worker_image.startswith("rust@sha256:") or worker_image.startswith("sha256:")):
        raise ValueError("CARDBENCH_MAGIC_WORKER_IMAGE must be an immutable digest reference")
    cargo_registry = Path.home() / ".cargo" / "registry"
    if not cargo_registry.is_dir():
        raise FileNotFoundError(f"offline Cargo registry cache is missing: {cargo_registry}")
    authority = REPO / AUTHORITY_SOURCE
    verify_workspace(workspace, manifest)
    scoring_workspace = make_scoring_snapshot(workspace, manifest_data["files"], output_root / "candidate-workspace")
    run_name = f"peregrine-mask-{uuid.uuid4().hex[:12]}"
    command = sandbox_command(scoring_workspace, cargo_registry, authority, run_name, worker_image)
    exit_code, stdout, stderr = run_bounded(command, timeout=600, name=run_name)
    report = parse_report(stdout)
    if exit_code != 0:
        raise WorkerFailure("candidate_protocol_error: authority returned a nonzero exit after its passing report")
    score_data = {"schema_version": "cardbench.magic.card-score.v1", "task_id": "cardbench/magic/card", "candidate_kind": candidate_kind, "passed": True, "reward": 0.0, "cards_credited": 0, "pilot_reward": 0.0, "harbor_reward": 0.0, "report": report}
    (output_root / "score.json").write_text(json.dumps(score_data, indent=2) + "\n", encoding="utf-8")
    (output_root / "worker.stderr.txt").write_bytes(stderr)
    return score_data


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--workspace", type=Path)
    parser.add_argument("--manifest", type=Path)
    parser.add_argument("--output-root", type=Path)
    parser.add_argument("--candidate-kind", choices=["agent", "reference"], default="agent")
    parser.add_argument("--validate-report", type=Path, help="validate an existing authority report without Docker")
    args = parser.parse_args()
    if args.validate_report:
        report = json.loads(args.validate_report.read_text(encoding="utf-8"))
        validate_report(report)
        print(json.dumps({"passed": True, "pilot_reward": 0.0, "harbor_reward": 0.0, "cards_credited": 0}))
        return 0
    if args.workspace is None or args.manifest is None or args.output_root is None:
        parser.error("--workspace, --manifest, and --output-root are required unless --validate-report is used")
    result = score(args.workspace.resolve(), args.manifest.resolve(), args.output_root.resolve(), args.candidate_kind)
    print(json.dumps(result))
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except WorkerFailure as error:
        print(str(error), file=sys.stderr)
        raise SystemExit(2)
