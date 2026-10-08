#!/usr/bin/env python3
"""Run and validate Elvish Skysweeper's isolated offline PvP authority."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import shutil
import subprocess
import sys
import uuid
from pathlib import Path

REPO = Path(__file__).resolve().parents[3]
MATERIALIZER = Path(__file__).with_name("materialize_elvish_skysweeper_task.py")
TARGET = Path("varieties/magic/sets/ravnica_city_of_guilds/src/cards/elvish_skysweeper.rs")
AUTHORITY = Path("varieties/magic/session/src/bin/rav_elvish_skysweeper_pvp.rs")
DEFAULT_IMAGE = "rust@sha256:d7482085ff5b415f84dba5647ae71606650bdef00db7aeb69f4b3d170c3e4082"


class WorkerFailure(RuntimeError):
    pass


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def verify_workspace(workspace: Path, manifest_path: Path) -> None:
    subprocess.run(
        [sys.executable, str(MATERIALIZER), "--workspace", str(workspace), "--manifest", str(manifest_path), "--verify"],
        check=True,
        cwd=REPO,
    )


def snapshot(workspace: Path, manifest: dict, destination: Path) -> Path:
    if destination.is_symlink():
        raise WorkerFailure("candidate_snapshot_path_is_symlink")
    if destination.exists():
        shutil.rmtree(destination)
    destination.mkdir(parents=True)
    for relative in sorted(manifest["files"]):
        source = workspace / Path(relative)
        if source.is_symlink() or not source.is_file():
            raise WorkerFailure(f"candidate_snapshot_source_missing: {relative}")
        target = destination / Path(relative)
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(source, target)
    return destination


def sandbox_command(workspace: Path, registry: Path, name: str, image: str, candidate_kind: str) -> list[str]:
    trusted_authority = REPO / AUTHORITY
    if trusted_authority.is_symlink() or not trusted_authority.is_file():
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
        "--mount", f"type=bind,src={trusted_authority.resolve()},dst=/trusted-authority.rs,readonly",
        "--env", "HOME=/tmp/home", "--env", "CARGO_HOME=/usr/local/cargo",
        "--env", "RUSTUP_HOME=/usr/local/rustup", "--env", "CARGO_NET_OFFLINE=true",
        "--env", "CARGO_BUILD_JOBS=2", image, "sh", "-c",
        "cp -R /submission/. /work/ && "
        "cp /trusted-authority.rs /work/varieties/magic/session/src/bin/rav_elvish_skysweeper_pvp.rs && "
        "cd /work && cargo run --offline --locked -j 2 --manifest-path varieties/magic/Cargo.toml "
        "-p cardbench-magic-session --bin rav-elvish-skysweeper-pvp --" + candidate_flag,
    ]


def validate_report(report: dict) -> None:
    card_id = "RAV-ELVISH-SKYSWEEPER"
    source_validation = report.get("source_validation")
    if (
        not isinstance(source_validation, dict)
        or source_validation.get("normalized_ast_schema_checked_before_candidate_calls") is not True
        or source_validation.get("side_effect_mutation_parse_valid") is not True
        or source_validation.get("side_effect_mutation_compile_valid_by_construction") is not True
        or source_validation.get("side_effect_mutation_rejected") is not True
        or source_validation.get("candidate_calls_before_guard") != 0
        or source_validation.get("rejection_stage")
        != "source-shape-and-declarative-AST-before-candidate-function-invocation"
    ):
        raise WorkerFailure("candidate_protocol_error: candidate source was not guarded before execution")
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
        or report.get("card") != {"collector_number": 162, "printings": 1, "definition_id": card_id, "name": "Elvish Skysweeper"}
    ):
        raise WorkerFailure("candidate_protocol_error: incomplete Elvish Skysweeper single-card report")
    traces = report.get("traces")
    if not isinstance(traces, list) or len(traces) != 1:
        raise WorkerFailure("candidate_protocol_error: exactly one Skysweeper full-game trace is required")
    trace = traces[0]
    source, sacrifice, target, combat = (trace.get("skysweeper"), trace.get("sacrifice"), trace.get("target"), trace.get("combat"))
    if (
        trace.get("trace_id") != "rav-elvish-skysweeper-sacrifice-destroy-flyer-full-game"
        or trace.get("terminal") != "Winner(PlayerId(0))" or trace.get("winner_seat") != 0
        or trace.get("rejected_moves") != 0 or not isinstance(trace.get("shuffle_seed"), int)
        or not isinstance(source, dict) or not isinstance(sacrifice, dict) or not isinstance(target, dict) or not isinstance(combat, dict)
        or source.get("mana_cost") != "{G}" or source.get("mana_paid") != ["Green"]
        or source.get("ability_id") != "sacrifice-creature-destroy-flying"
        or source.get("ability_mana_cost") != "{4}{G}" or source.get("sacrifice_cost") != 1
        or not isinstance(source.get("object"), int)
        or not isinstance(sacrifice.get("object"), int) or sacrifice.get("object") == source.get("object")
        or sacrifice.get("definition") != "RAV-BIRDS-OF-PARADISE" or sacrifice.get("zone_after_cost") != "Some(Graveyard)"
        or not isinstance(target.get("object"), int) or target.get("object") != target.get("destroyed_by")
        or target.get("definition") != "RAV-BIRDS-OF-PARADISE" or target.get("owner") != 1
        or target.get("zone_after_resolution") != "Some(Graveyard)"
        or not isinstance(combat.get("attack_turns"), list) or len(combat.get("attack_turns", [])) < 3
        or not isinstance(combat.get("damage_to_opponent"), int) or combat.get("damage_to_opponent", 0) < 20
        or not isinstance(combat.get("life_at_terminal"), list) or len(combat["life_at_terminal"]) != 2
        or combat["life_at_terminal"][1] > 0
    ):
        raise WorkerFailure("candidate_protocol_error: Skysweeper cost, sacrifice, exact flyer destruction, combat, or terminal result is wrong")
    events, ordered = trace.get("events"), trace.get("ordered_event_indices")
    if not isinstance(events, list) or len(events) < 40 or any(not isinstance(event, dict) for event in events):
        raise WorkerFailure("candidate_protocol_error: projected Skysweeper event trace is missing")
    sequences = [event.get("sequence") for event in events]
    if any(not isinstance(value, int) for value in sequences) or sequences != sorted(sequences):
        raise WorkerFailure("candidate_protocol_error: projected event sequence is malformed")
    if not isinstance(ordered, list) or len(ordered) != 12 or any(not isinstance(value, int) for value in ordered) or ordered != sorted(ordered) or ordered[-1] >= len(events):
        raise WorkerFailure("candidate_protocol_error: ordered Skysweeper event indices are malformed")
    source_id, sacrifice_id, target_id = source["object"], sacrifice["object"], target["object"]
    expected_kinds = ["SpellManaPaid", "SpellCast", "SpellResolved", "CardMoved", "AbilityManaPaid", "SacrificedAsAbilityCost", "CardMoved", "AbilityActivated", "CardDestroyed", "CardMoved", "AbilityResolved", "GameEnded"]
    selected = [events[index] for index in ordered]
    if [event.get("kind") for event in selected] != expected_kinds:
        raise WorkerFailure("candidate_protocol_error: event receipts are missing or out of order")
    canonical = [event.get("canonical", "") for event in selected]
    required = [
        f"card: ObjectId({source_id})", f"card: ObjectId({source_id})", f"card: ObjectId({source_id})",
        f"card: ObjectId({source_id}), to: Battlefield", f"source: ObjectId({source_id})",
        f"source: ObjectId({source_id}), permanent: ObjectId({sacrifice_id})",
        f"card: ObjectId({sacrifice_id}), to: Graveyard", f"source: ObjectId({source_id})",
        f"source: ObjectId({source_id}), card: ObjectId({target_id})",
        f"card: ObjectId({target_id}), to: Graveyard", f"source: ObjectId({source_id})",
        "winner: Some(PlayerId(0))",
    ]
    for index, (event, text) in enumerate(zip(selected, canonical, strict=True)):
        if required[index] not in text:
            raise WorkerFailure(f"candidate_protocol_error: wrong object provenance at ordered event {expected_kinds[index]}")
    mutations = report.get("mutation_sensitivity")
    required_mutations = (
        "wrong_activation_cost_mutation_detected", "missing_sacrifice_cost_mutation_detected",
        "broad_creature_target_mutation_detected", "wrong_destroy_effect_mutation_detected",
        "wrong_creature_body_mutation_detected",
    )
    if not isinstance(mutations, dict) or any(mutations.get(key) is not True for key in required_mutations) or mutations.get("sealed_mutation_authority") is not False:
        raise WorkerFailure("candidate_protocol_error: Skysweeper semantic mutation controls are incomplete")


def score(workspace: Path, manifest_path: Path, output_root: Path, candidate_kind: str) -> int:
    output_root.mkdir(parents=True, exist_ok=True)
    manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    image = os.environ.get("CARDBENCH_MAGIC_WORKER_IMAGE", DEFAULT_IMAGE)
    if not (image.startswith("rust@sha256:") or image.startswith("sha256:")):
        raise ValueError("CARDBENCH_MAGIC_WORKER_IMAGE must be an immutable digest reference")
    registry = Path.home() / ".cargo" / "registry"
    if not registry.is_dir():
        raise FileNotFoundError(f"offline Cargo registry cache is missing: {registry}")
    if not shutil.which("docker"):
        raise FileNotFoundError("Docker is required for isolated Magic candidate evaluation")
    verify_workspace(workspace, manifest_path)
    candidate_workspace = snapshot(workspace, manifest, output_root / "candidate-workspace")
    source_hash = sha256((candidate_workspace / TARGET).read_bytes())
    name = f"cardbench-magic-elvish-skysweeper-{uuid.uuid4().hex[:16]}"
    result = subprocess.run(sandbox_command(candidate_workspace, registry.resolve(), name, image, candidate_kind), stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=900, check=False)
    (output_root / "worker.stdout").write_bytes(result.stdout)
    (output_root / "worker.stderr").write_bytes(result.stderr)
    report = None
    if result.returncode == 0:
        for line in reversed(result.stdout.decode(errors="replace").splitlines()):
            try:
                candidate_report = json.loads(line)
            except json.JSONDecodeError:
                continue
            if candidate_report.get("schema_version") == "cardbench.magic.card-pvp.check.v1":
                report = candidate_report
                break
    if report is None:
        report = {"schema_version": "cardbench.magic.card-pvp.check.v1", "task_id": "cardbench/magic/card", "candidate": {"kind": candidate_kind, "source_sha256": source_hash}, "passed": False, "pilot_card_passed": False, "cards_passed": 0, "cards_total": 291, "printings_total": 306, "cards_credited": 0, "coverage_complete": False, "full_reward": False, "harbor_reward": 0.0, "sealed_authority": False, "error": "candidate compile or Elvish Skysweeper reference trace failed", "diagnostics": result.stderr.decode(errors="replace")[-12000:]}
    else:
        validate_report(report)
        report["candidate"] = {"kind": candidate_kind, "card_definition": "RAV-ELVISH-SKYSWEEPER", "source_sha256": source_hash, "materialized_workspace_sha256": manifest["workspace_sha256"]}
        report["candidate_kind"] = candidate_kind
        report["starting_workspace_sha256"] = manifest["workspace_sha256"]
        report["sealed_authority"] = False
        report["worker_image"] = image
        report["sandbox"] = {"network": "none", "rootfs": "read-only", "pids_limit": 128}
        report["harbor_reward"] = 0.0
        report["full_reward"] = False
    (output_root / "result.json").write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    (output_root / "reward.txt").write_text("0.00000000\n", encoding="utf-8")
    print(f"candidate_passed={report.get('pilot_card_passed', False)} cards={report.get('cards_passed', 0)}/291 credit=0 reward=0")
    return 0 if report.get("passed") is True and report.get("pilot_card_passed") is True else 1


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
        report = {"schema_version": "cardbench.magic.card-pvp.check.v1", "task_id": "cardbench/magic/card", "passed": False, "pilot_card_passed": False, "cards_passed": 0, "cards_total": 291, "printings_total": 306, "cards_credited": 0, "coverage_complete": False, "full_reward": False, "harbor_reward": 0.0, "sealed_authority": False, "error": f"{type(error).__name__}: {error}"}
        (args.output_root / "result.json").write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
        (args.output_root / "reward.txt").write_text("0.00000000\n", encoding="utf-8")
        print(report["error"], file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
