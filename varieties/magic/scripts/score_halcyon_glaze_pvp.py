#!/usr/bin/env python3
"""Validate and optionally score an isolated Halcyon Glaze submission."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import shutil
import subprocess
import uuid
from pathlib import Path

from materialize_halcyon_glaze_task import verify

REPO = Path(__file__).resolve().parents[3]
TARGET = Path("varieties/magic/sets/ravnica_city_of_guilds/src/cards/halcyon_glaze.rs")
AUTHORITY = Path("varieties/magic/session/src/bin/rav_halcyon_glaze_pvp.rs")
BIN = "rav-halcyon-glaze-pvp"
CARD_ID = "RAV-HALCYON-GLAZE"
DEFAULT_IMAGE = "rust@sha256:d7482085ff5b415f84dba5647ae71606650bdef00db7aeb69f4b3d170c3e4082"


class WorkerFailure(RuntimeError):
    pass


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
            "collector_number": 54,
            "definition_id": CARD_ID,
            "name": "Halcyon Glaze",
        }
    ):
        raise WorkerFailure("candidate_protocol_error: incomplete Halcyon Glaze single-card report")
    traces = report.get("traces")
    if not isinstance(traces, list) or len(traces) != 1 or not isinstance(traces[0], dict):
        raise WorkerFailure("candidate_protocol_error: exactly one Halcyon Glaze full-game trace is required")
    trace = traces[0]
    if (
        trace.get("id") != "rav-halcyon-glaze-controller-creature-cast-animation-full-game"
        or trace.get("passed") is not True
        or not isinstance(trace.get("seed"), int)
        or trace["seed"] < 0
        or trace.get("terminal") != "Winner(PlayerId(0))"
        or trace.get("winner_seat") != 0
        or trace.get("rejected_moves") != 0
        or not isinstance(trace.get("turns"), int)
        or trace["turns"] <= 0
        or not isinstance(trace.get("accepted_moves"), int)
        or trace["accepted_moves"] <= 0
    ):
        raise WorkerFailure("candidate_protocol_error: Halcyon Glaze did not complete a clean PvP win")
    mechanics = trace.get("mechanics")
    if not isinstance(mechanics, dict):
        raise WorkerFailure("candidate_protocol_error: Halcyon Glaze mechanics are missing")
    payment = mechanics.get("paid_mana_colors")
    resolutions = mechanics.get("trigger_resolutions")
    damage = mechanics.get("damage_packets")
    cleanup = mechanics.get("cleanup_state")
    if (
        mechanics.get("definition_id") != CARD_ID
        or mechanics.get("mana_cost") != "{1}{U}{U}"
        or not isinstance(payment, list)
        or len(payment) != 3
        or payment.count("Blue") != 3
        or not isinstance(mechanics.get("controller_creature_spells_after_entry"), int)
        or mechanics["controller_creature_spells_after_entry"] < 5
        or not isinstance(mechanics.get("opponent_creature_spells_after_entry"), int)
        or mechanics["opponent_creature_spells_after_entry"] < 1
        or mechanics.get("trigger_stack_count") != mechanics.get("controller_creature_spells_after_entry")
        or mechanics.get("trigger_resolution_count") != mechanics.get("controller_creature_spells_after_entry")
        or not isinstance(resolutions, list)
        or len(resolutions) != mechanics.get("trigger_resolution_count")
        or not all(
            isinstance(item, dict)
            and item.get("stack_len") == 1
            and item.get("underlying_creature_spell") is not None
            and item.get("state") == {
                "colors": ["Blue"],
                "card_types": ["Creature", "Enchantment"],
                "creature_subtypes": ["Illusion"],
                "power": 4,
                "toughness": 4,
                "flying": True,
            }
            for item in resolutions
        )
        or not isinstance(cleanup, dict)
        or cleanup.get("colors") != ["Blue"]
        or cleanup.get("card_types") != ["Enchantment"]
        or cleanup.get("creature_subtypes") != []
        or cleanup.get("power") is not None
        or cleanup.get("toughness") is not None
        or cleanup.get("flying") is not False
        or mechanics.get("flying_glaze_attacks") != 5
        or damage != [4, 4, 4, 4, 4]
        or mechanics.get("terminal_pilot_win") is not True
    ):
        raise WorkerFailure("candidate_protocol_error: Glaze cost, cast scope, animation, cleanup, or combat differs")
    events = trace.get("events")
    if not isinstance(events, list) or not all(isinstance(event, dict) for event in events):
        raise WorkerFailure("candidate_protocol_error: projected event sequence is malformed")
    kinds = [event.get("kind") for event in events]
    if kinds.count("TriggeredAbilityStacked") < 5 or kinds.count("AbilityResolved") < 5 or kinds.count("GameEnded") != 1:
        raise WorkerFailure("candidate_protocol_error: trigger, resolution, or terminal event receipts are missing")
    if not isinstance(trace.get("ordered_event_sequence"), list) or trace["ordered_event_sequence"] != sorted(trace["ordered_event_sequence"]):
        raise WorkerFailure("candidate_protocol_error: ordered event receipts are invalid")
    guard = report.get("candidate_source_validation", {})
    if (
        guard.get("normalized_literal_source_and_ast_checked_before_candidate_calls") is not True
        or guard.get("compile_valid_injected_side_effect_candidate_rejected") is not True
        or guard.get("injected_candidate_calls_before_rejection") != 0
        or guard.get("side_effect_sentinel_absent_after_rejection") is not True
    ):
        raise WorkerFailure("candidate_protocol_error: source guard did not reject side effects before evaluation")
    mutations = report.get("mutation_sensitivity", {})
    for key in (
        "printed_cast_cost_mutation_detected_in_game",
        "missing_creature_spell_trigger_mutation_detected_in_game",
        "wrong_trigger_controller_scope_mutation_detected_in_game",
        "wrong_animation_power_mutation_detected_in_game",
        "wrong_animation_toughness_mutation_detected_in_game",
        "missing_flying_mutation_detected_in_game",
        "wrong_animation_color_mutation_detected_in_game",
        "wrong_illusion_subtype_mutation_detected_in_game",
        "persistent_creature_type_mutation_detected_in_game",
    ):
        if not isinstance(mutations, dict) or mutations.get(key) is not True:
            raise WorkerFailure(f"candidate_protocol_error: Halcyon Glaze mutation escaped: {key}")


def run_worker(workspace: Path, registry: Path, authority: Path, name: str, image: str, candidate_kind: str) -> subprocess.CompletedProcess[bytes]:
    flag = " --candidate" if candidate_kind == "agent" else ""
    command = [
        "docker", "run", "--rm", "--name", name, "--network", "none", "--read-only",
        "--cap-drop", "ALL", "--security-opt", "no-new-privileges", "--pids-limit", "128",
        "--memory", "3g", "--cpus", "2", "--user", "1000:1000",
        "--tmpfs", "/work:rw,exec,size=2147483648,uid=1000,gid=1000",
        "--tmpfs", "/tmp:rw,noexec,size=67108864,uid=1000,gid=1000",
        "--mount", f"type=bind,src={workspace},dst=/submission,readonly",
        "--mount", f"type=bind,src={registry},dst=/usr/local/cargo/registry,readonly",
        "--mount", f"type=bind,src={authority.resolve()},dst=/trusted-authority.rs,readonly",
        "--env", "HOME=/tmp/home", "--env", "CARGO_HOME=/usr/local/cargo", "--env", "RUSTUP_HOME=/usr/local/rustup",
        "--env", "CARGO_NET_OFFLINE=true", "--env", "CARGO_BUILD_JOBS=2", "--env", "RAYON_NUM_THREADS=2",
        image, "sh", "-c",
        "cp -R /submission/. /work/ && cp /trusted-authority.rs /work/varieties/magic/session/src/bin/rav_halcyon_glaze_pvp.rs && cd /work && cargo run --offline --locked -j 2 --manifest-path varieties/magic/Cargo.toml -p cardbench-magic-session --bin rav-halcyon-glaze-pvp --" + flag,
    ]
    return subprocess.run(command, capture_output=True, timeout=900, check=False)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--validate-report", type=Path)
    parser.add_argument("--workspace", type=Path)
    parser.add_argument("--manifest", type=Path)
    parser.add_argument("--output-root", type=Path)
    parser.add_argument("--candidate-kind", choices=["agent", "reference"], default="agent")
    args = parser.parse_args()
    if args.validate_report:
        validate_report(json.loads(args.validate_report.read_text(encoding="utf-8")))
        print("Halcyon Glaze report validates; rewards and credit remain zero")
        return 0
    if not args.workspace or not args.manifest or not args.output_root:
        parser.error("--workspace, --manifest, and --output-root are required unless --validate-report is used")
    output = args.output_root.resolve()
    output.mkdir(parents=True, exist_ok=True)
    manifest = json.loads(args.manifest.read_text(encoding="utf-8"))
    image = os.environ.get("CARDBENCH_MAGIC_WORKER_IMAGE", DEFAULT_IMAGE)
    if not (image.startswith("rust@sha256:") or image.startswith("sha256:")):
        raise ValueError("CARDBENCH_MAGIC_WORKER_IMAGE must be an immutable digest reference")
    registry = Path.home() / ".cargo" / "registry"
    authority = REPO / AUTHORITY
    if not registry.is_dir() or not authority.is_file() or authority.is_symlink() or not shutil.which("docker"):
        raise FileNotFoundError("offline Cargo registry, trusted authority, or Docker is unavailable")
    verify(args.workspace.resolve(), args.manifest.resolve())
    candidate_workspace = output / "candidate-workspace"
    if candidate_workspace.exists():
        shutil.rmtree(candidate_workspace)
    for relative in sorted(manifest["files"]):
        source = args.workspace.resolve() / Path(relative)
        destination = candidate_workspace / Path(relative)
        if source.is_symlink() or not source.is_file():
            raise WorkerFailure(f"candidate_workspace_source_missing: {relative}")
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(source, destination)
    source_hash = hashlib.sha256((candidate_workspace / TARGET).read_bytes()).hexdigest()
    name = f"cardbench-magic-halcyon-glaze-{uuid.uuid4().hex[:16]}"
    process = run_worker(candidate_workspace, registry.resolve(), authority, name, image, args.candidate_kind)
    (output / "worker.stdout").write_bytes(process.stdout)
    (output / "worker.stderr").write_bytes(process.stderr)
    if process.returncode != 0:
        raise WorkerFailure("candidate_compile_or_reference_failed: " + process.stderr.decode(errors="replace")[-12000:])
    report = None
    for line in reversed(process.stdout.decode(errors="replace").splitlines()):
        try:
            value = json.loads(line)
        except json.JSONDecodeError:
            continue
        if isinstance(value, dict) and value.get("schema_version") == "cardbench.magic.card-pvp.check.v1":
            report = value
            break
    if report is None:
        raise WorkerFailure("candidate_protocol_error: no Halcyon Glaze PvP report")
    validate_report(report)
    report["candidate"] = {"kind": args.candidate_kind, "card_definition": CARD_ID, "source_sha256": source_hash, "materialized_workspace_sha256": manifest["workspace_sha256"]}
    report["candidate_kind"] = args.candidate_kind
    report["sealed_authority"] = False
    report["worker_image"] = image
    report["sandbox"] = {"network": "none", "rootfs": "read-only", "pids_limit": 128}
    report["harbor_reward"] = 0.0
    report["pilot_reward"] = 0.0
    (output / "result.json").write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    (output / "reward.txt").write_text("0.00000000\n", encoding="utf-8")
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except Exception as error:
        raise SystemExit(f"Halcyon Glaze scoring failed: {error}")
