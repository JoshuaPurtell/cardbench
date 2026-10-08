#!/usr/bin/env python3
"""Run Shambling Shell's isolated, offline single-card PvP authority."""

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
MATERIALIZER = Path(__file__).with_name("materialize_shambling_shell_task.py")
TARGET = Path("varieties/magic/sets/ravnica_city_of_guilds/src/cards/shambling_shell.rs")
AUTHORITY = Path("varieties/magic/session/src/bin/rav_shambling_shell_pvp.rs")
DEFAULT_IMAGE = "rust@sha256:d7482085ff5b415f84dba5647ae71606650bdef00db7aeb69f4b3d170c3e4082"


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
        "cp /trusted-authority.rs /work/varieties/magic/session/src/bin/rav_shambling_shell_pvp.rs && "
        "cd /work && cargo run --offline --locked -j 2 --manifest-path varieties/magic/Cargo.toml "
        "-p cardbench-magic-session --bin rav-shambling-shell-pvp --" + candidate_flag,
    ]


def validate_report(report: dict) -> None:
    card_id = "RAV-SHAMBLING-SHELL"
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
        or report.get("card") != {"collector_number": 230, "definition_id": card_id, "name": "Shambling Shell"}
    ):
        raise WorkerFailure("candidate_protocol_error: incomplete Shambling Shell single-card report")
    traces = report.get("traces")
    if not isinstance(traces, list) or len(traces) != 1:
        raise WorkerFailure("candidate_protocol_error: exactly one full-game trace is required")
    trace = traces[0]
    shell = trace.get("shell_object")
    target = trace.get("watchwolf_target_object")
    shell_payments = trace.get("shell_cast_payments")
    target_payments = trace.get("watchwolf_cast_payments")
    if (
        trace.get("trace_id") != "rav-shambling-shell-sacrifice-counter-dredge-full-game"
        or trace.get("terminal") != "Winner(PlayerId(0))"
        or trace.get("winner_seat") != 0
        or trace.get("rejected_moves") != 0
        or trace.get("dredge_count") != 3
        or trace.get("target_counter_count") != 1
        or (trace.get("target_power"), trace.get("target_toughness")) != (4, 4)
        or (trace.get("shell_power"), trace.get("shell_toughness")) != (3, 1)
        or trace.get("shell_zone") != "Some(Battlefield)"
        or not isinstance(trace.get("target_combat_damage"), int)
        or trace.get("target_combat_damage", 0) < 20
        or not isinstance(trace.get("shuffle_seed"), int)
        or not isinstance(shell, str)
        or not isinstance(target, str)
        or trace.get("deck_p0") != "4 Shambling Shell + 4 Watchwolf + 26 Forest + 13 Swamp + 13 Plains (60 shuffled cards)"
        or trace.get("deck_p1") != "30 Forest + 30 Plains (60 shuffled cards)"
        or not isinstance(trace.get("mill_object_ids"), list)
        or len(trace["mill_object_ids"]) != 3
        or len(set(trace["mill_object_ids"])) != 3
        or not isinstance(shell_payments, list)
        or len(shell_payments) < 2
        or any(len(payment) != 3 or payment.count("Black") < 1 or payment.count("Green") < 1 for payment in shell_payments[:2])
        or not isinstance(target_payments, list)
        or not any(len(payment) == 3 and payment.count("Green") >= 1 and payment.count("White") >= 1 for payment in target_payments)
        or not isinstance(trace.get("events"), list)
        or not isinstance(trace.get("event_count"), int)
        or trace.get("event_count", 0) < 35
        or not isinstance(trace.get("life_totals"), list)
        or len(trace["life_totals"]) != 2
        or trace["life_totals"][1] > 0
    ):
        raise WorkerFailure("candidate_protocol_error: Shambling Shell source sacrifice, counter, Dredge, payment, or terminal trace is incomplete")
    event_text = "\n".join(str(event) for event in trace["events"])
    for required in ("SpellCast", "SpellResolved", "SacrificedAsAbilityCost", "AbilityActivated", "AbilityResolved", "CounterPlaced", "Dredged", "DamageDealtToPlayer", "GameEnded"):
        if required not in event_text:
            raise WorkerFailure(f"candidate_protocol_error: missing {required} event receipt")
    if "targeted-creature-return-from-graveyard" in event_text:
        raise WorkerFailure("candidate_protocol_error: an unrelated graveyard return trigger appeared")
    def event_index(event: str) -> int:
        return int(event.split(":", 1)[0])

    cast_positions = [event_index(event) for event in trace["events"] if "SpellCast" in event and shell in event]
    resolved_positions = [event_index(event) for event in trace["events"] if "SpellResolved" in event and shell in event]
    dredge_position = next((event_index(event) for event in trace["events"] if "Dredged" in event and shell in event and "count: 3" in event), None)
    damage_position = next((event_index(event) for event in trace["events"] if "DamageDealtToPlayer" in event and target in event and "PlayerId(1)" in event), None)
    ended_position = next((event_index(event) for event in trace["events"] if "GameEnded" in event and "PlayerId(0)" in event), None)
    sacrifice = trace.get("sacrifice_as_cost_event_index")
    activated = trace.get("ability_activated_event_index")
    resolved = trace.get("ability_resolved_event_index")
    counter = trace.get("counter_placed_event_index")
    if (
        len(cast_positions) < 2
        or len(resolved_positions) < 2
        or dredge_position is None
        or damage_position is None
        or ended_position is None
        or not all(isinstance(index, int) for index in (sacrifice, activated, resolved, counter))
        or not (cast_positions[0] < sacrifice < activated < counter < resolved < dredge_position < cast_positions[1] < resolved_positions[1] < damage_position < ended_position)
    ):
        raise WorkerFailure("candidate_protocol_error: Shell sacrifice/counter/Dredge/same-object recast/combat event ordering is wrong")
    mutations = report.get("mutation_sensitivity")
    required_mutations = (
        "missing_dredge_mutation_detected",
        "wrong_dredge_count_mutation_detected",
        "wrong_mana_cost_mutation_detected",
        "wrong_creature_body_mutation_detected",
        "missing_sacrifice_counter_binding_mutation_detected",
        "missing_source_sacrifice_cost_mutation_detected",
        "wrong_sacrifice_counter_effect_mutation_detected",
        "compile_valid_side_effect_candidate_rejected_before_candidate_calls",
    )
    if not isinstance(mutations, dict) or any(mutations.get(key) is not True for key in required_mutations) or mutations.get("sealed_mutation_authority") is not False:
        raise WorkerFailure("candidate_protocol_error: Shambling Shell semantic mutation controls are incomplete")
    source_guard = report.get("source_guard")
    if not isinstance(source_guard, dict) or source_guard.get("compile_valid_side_effect_candidate_by_construction") is not True or source_guard.get("candidate_factories_called_for_injected_source") is not False:
        raise WorkerFailure("candidate_protocol_error: compile-valid side-effect source rejection attestation is incomplete")

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
        raise FileNotFoundError("Docker is required for Magic candidate isolation")
    verify_workspace(workspace, manifest_path)
    scoring_workspace = snapshot(workspace, manifest, output_root / "candidate-workspace")
    source_hash = sha256((scoring_workspace / TARGET).read_bytes())
    name = f"cardbench-magic-shambling-shell-{uuid.uuid4().hex[:16]}"
    result = subprocess.run(
        sandbox_command(scoring_workspace, registry.resolve(), name, image, candidate_kind),
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        timeout=900,
        check=False,
    )
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
        report = {
            "schema_version": "cardbench.magic.card-pvp.check.v1",
            "task_id": "cardbench/magic/card",
            "candidate": {"kind": candidate_kind, "source_sha256": source_hash},
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
            "error": "candidate compile or Shambling Shell reference trace failed",
            "diagnostics": result.stderr.decode(errors="replace")[-12000:],
        }
    else:
        validate_report(report)
        report["candidate"] = {
            "kind": candidate_kind,
            "card_definition": "RAV-SHAMBLING-SHELL",
            "source_sha256": source_hash,
            "materialized_workspace_sha256": manifest["workspace_sha256"],
        }
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
            "error": str(error),
        }
        (args.output_root / "result.json").write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
        (args.output_root / "reward.txt").write_text("0.00000000\n", encoding="utf-8")
        print(f"Magic candidate verifier failed: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
