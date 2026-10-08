#!/usr/bin/env python3
"""Score an isolated Light of Sanction submission against its trusted PvP trace."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import shutil
import uuid
from pathlib import Path

from score_watchwolf_pvp import WorkerFailure, make_scoring_snapshot, run_bounded
from materialize_light_of_sanction_task import verify

REPO = Path(__file__).resolve().parents[3]
AUTHORITY_SOURCE = Path("varieties/magic/session/src/bin/rav_light_of_sanction_pvp.rs")
TARGET = Path("varieties/magic/sets/ravnica_city_of_guilds/src/cards/light_of_sanction.rs")
DEFAULT_WORKER_IMAGE = "rust@sha256:d7482085ff5b415f84dba5647ae71606650bdef00db7aeb69f4b3d170c3e4082"


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def _int(value: object) -> bool:
    return type(value) is int


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
        or report.get("card") != {"collector_number": 24, "definition_id": "RAV-LIGHT-OF-SANCTION", "name": "Light of Sanction"}
    ):
        raise WorkerFailure("candidate_protocol_error: incomplete Light of Sanction single-card report")
    candidate = report.get("candidate")
    if not isinstance(candidate, dict) or candidate.get("card_definition") != "RAV-LIGHT-OF-SANCTION":
        raise WorkerFailure("candidate_protocol_error: wrong Light of Sanction candidate identity")
    traces = report.get("traces")
    if not isinstance(traces, list) or len(traces) != 1:
        raise WorkerFailure("candidate_protocol_error: exactly one Light of Sanction trace is required")
    trace = traces[0]
    if (
        not isinstance(trace, dict)
        or trace.get("id") != "rav-light-of-sanction-friendly-source-prevention"
        or trace.get("passed") is not True
        or trace.get("seed") != 0
        or trace.get("winner_seat") != 0
        or trace.get("terminal") != "Winner(PlayerId(0))"
        or trace.get("rejected_moves") != 0
        or not _int(trace.get("turns")) or trace["turns"] <= 0
        or not _int(trace.get("accepted_moves")) or trace["accepted_moves"] <= 0
        or trace.get("policies") != ["rav.light-of-sanction-source-damage.v1", "rav.light-of-sanction-opponent-char.v1"]
        or trace.get("decks") != ["rav_light_of_sanction_three_color_pilot", "rav_char_opponent"]
    ):
        raise WorkerFailure("candidate_protocol_error: Light of Sanction full-game terminal trace differs")

    mechanics = trace.get("mechanics")
    if not isinstance(mechanics, dict):
        raise WorkerFailure("candidate_protocol_error: Light of Sanction mechanics are missing")
    light, friendly, opponent, wolf, finisher = (mechanics.get(key) for key in ("light_of_sanction", "friendly_char", "opponent_char", "watchwolf", "finisher"))
    if not all(isinstance(item, dict) for item in (light, friendly, opponent, wolf, finisher)):
        raise WorkerFailure("candidate_protocol_error: source, damage packets, or finisher mechanics are missing")
    if (
        not _int(light.get("object")) or light["object"] <= 0
        or not _int(light.get("source_incarnation")) or light["source_incarnation"] <= 0
        or light.get("mana_cost") != "{1}{W}{W}"
        or not isinstance(light.get("payment"), list) or len(light["payment"]) != 3 or light["payment"].count("White") < 2
        or not _int(friendly.get("object")) or friendly["object"] <= 0
        or friendly.get("mana_cost") != "{2}{R}"
        or not isinstance(friendly.get("payment"), list) or len(friendly["payment"]) != 3 or "Red" not in friendly["payment"]
        or not _int(friendly.get("target")) or friendly["target"] != wolf.get("object")
        or friendly.get("target_damage") != 4 or friendly.get("prevented") is not True
        or friendly.get("target_survived_with_damage") != 0 or friendly.get("controller_self_damage") != 2
        or not _int(opponent.get("object")) or opponent["object"] <= 0
        or opponent.get("mana_cost") != "{2}{R}"
        or not isinstance(opponent.get("payment"), list) or len(opponent["payment"]) != 3 or "Red" not in opponent["payment"]
        or opponent.get("target") != wolf.get("object") or opponent.get("target_damage") != 4
        or opponent.get("damage_committed") is not True or opponent.get("target_moved_to_graveyard") is not True
        or opponent.get("light_still_on_battlefield") is not True
        or wolf.get("power") != 3 or wolf.get("toughness") != 3
        or wolf.get("survived_friendly_char") is not True or wolf.get("died_to_opponent_char") is not True
        or not _int(finisher.get("object")) or finisher["object"] <= 0
        or finisher.get("mana_cost") != "{1}{R}{W}"
        or not isinstance(finisher.get("payment"), list) or len(finisher["payment"]) != 3
        or "Red" not in finisher["payment"] or "White" not in finisher["payment"]
        or finisher.get("flying") is not True or finisher.get("combat_damage") != 2
        or mechanics.get("controller_relative_prevention_consistent") is not True
    ):
        raise WorkerFailure("candidate_protocol_error: Light, controlled-source prevention, opponent damage, or terminal finisher differs")

    events = trace.get("events")
    expected_kinds = [
        "SpellManaPaid", "SpellCast", "SpellResolved", "CardMoved",
        "SpellCast", "CardMoved",
        "SpellManaPaid", "SpellCast", "DamagePrevented", "DamageDealtToPlayer", "SpellResolved",
        "SpellManaPaid", "SpellCast", "DamageDealtToPermanent", "SpellResolved", "CardMoved",
        "SpellManaPaid", "SpellCast", "SpellResolved", "CardMoved", "AttackersDeclared", "DamageDealtToPlayer", "GameEnded",
    ]
    if not isinstance(events, list) or [event.get("kind") for event in events if isinstance(event, dict)] != expected_kinds or len(events) != len(expected_kinds):
        raise WorkerFailure("candidate_protocol_error: ordered Light/prevention/opposing-damage/terminal event trace differs")
    sequences = [event.get("sequence") for event in events]
    if any(not _int(sequence) or sequence < 0 for sequence in sequences) or sequences != sorted(sequences) or len(set(sequences)) != len(sequences):
        raise WorkerFailure("candidate_protocol_error: Light of Sanction event sequences are malformed")
    light_paid, light_cast, light_resolved, light_entered = events[:4]
    wolf_cast, wolf_entered = events[4:6]
    friendly_paid, friendly_cast, prevented, self_damage, friendly_resolved = events[6:11]
    opposing_paid, opposing_cast, enemy_damage, opposing_resolved, wolf_died = events[11:16]
    sky_paid, sky_cast, sky_resolved, sky_entered, attack, combat_damage, terminal = events[16:]

    def has_obj(event: dict, obj: int, zone: str | None = None) -> bool:
        facts = event.get("facts")
        return isinstance(facts, dict) and obj in facts.get("objects", []) and (zone is None or facts.get("zone") == zone)

    if not (
        light_paid.get("facts", {}).get("seats") == [0] and has_obj(light_paid, light["object"]) and light_paid.get("facts", {}).get("amount") == 3
        and has_obj(light_cast, light["object"]) and has_obj(light_resolved, light["object"]) and has_obj(light_entered, light["object"], "Battlefield")
        and light_paid["sequence"] < light_cast["sequence"] < light_resolved["sequence"] < light_entered["sequence"]
        and has_obj(wolf_cast, wolf["object"]) and has_obj(wolf_entered, wolf["object"], "Battlefield")
        and has_obj(friendly_paid, friendly["object"]) and friendly_paid.get("facts", {}).get("seats") == [0]
        and has_obj(friendly_cast, friendly["object"]) and has_obj(prevented, friendly["object"]) and has_obj(prevented, wolf["object"])
        and prevented.get("facts", {}).get("amount") == 4
        and has_obj(self_damage, friendly["object"]) and self_damage.get("facts", {}).get("seats") == [0] and self_damage.get("facts", {}).get("amount") == 2
        and has_obj(friendly_resolved, friendly["object"])
        and friendly_cast["sequence"] < prevented["sequence"] < self_damage["sequence"] < friendly_resolved["sequence"]
        and has_obj(opposing_paid, opponent["object"]) and opposing_paid.get("facts", {}).get("seats") == [1]
        and has_obj(opposing_cast, opponent["object"]) and has_obj(enemy_damage, opponent["object"]) and has_obj(enemy_damage, wolf["object"])
        and enemy_damage.get("facts", {}).get("amount") == 4 and has_obj(opposing_resolved, opponent["object"])
        and has_obj(wolf_died, wolf["object"], "Graveyard")
        and opposing_cast["sequence"] < enemy_damage["sequence"] < opposing_resolved["sequence"] < wolf_died["sequence"]
        and has_obj(sky_paid, finisher["object"]) and sky_paid.get("facts", {}).get("seats") == [0]
        and has_obj(sky_cast, finisher["object"]) and has_obj(sky_resolved, finisher["object"])
        and has_obj(sky_entered, finisher["object"], "Battlefield")
        and attack.get("facts", {}).get("seats") == [0] and has_obj(attack, finisher["object"])
        and has_obj(combat_damage, finisher["object"]) and combat_damage.get("facts", {}).get("seats") == [1]
        and combat_damage.get("facts", {}).get("amount") == 2
        and terminal.get("facts", {}).get("seats") == [0]
        and terminal.get("sequence") == trace.get("terminal_sequence")
        and prevented.get("sequence") == trace.get("prevention_sequence")
        and enemy_damage.get("sequence") == trace.get("opponent_damage_sequence")
    ):
        raise WorkerFailure("candidate_protocol_error: Light payment/cast/resolution, prevention, lethal opponent damage, or terminal order differs")
    mutations = report.get("mutation_sensitivity")
    required = ("static_binding_removal_detected", "wrong_source_scope_detected", "prevention_keyword_mutation_detected")
    if not isinstance(mutations, dict) or any(mutations.get(key) is not True for key in required):
        raise WorkerFailure("candidate_protocol_error: static binding, source-scope, and keyword mutations must be detected")


def parse_report(stdout: bytes) -> dict:
    for line in reversed(stdout.decode(errors="replace").splitlines()):
        try:
            report = json.loads(line)
        except json.JSONDecodeError:
            continue
        if isinstance(report, dict) and report.get("schema_version") == "cardbench.magic.card-pvp.check.v1":
            validate_report(report)
            return report
    raise WorkerFailure("candidate_protocol_error: no valid Light of Sanction PvP result")


def sandbox_command(workspace: Path, registry: Path, authority: Path, name: str, image: str, candidate_kind: str) -> list[str]:
    if authority.is_symlink() or not authority.is_file():
        raise WorkerFailure("trusted Light of Sanction authority source is missing")
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
        "--env", "CARGO_BUILD_JOBS=2", "--env", "RAYON_NUM_THREADS=2",
        "--env", "CARDBENCH_LIGHT_OF_SANCTION_SEED=0", image, "sh", "-c",
        "cp -R /submission/. /work/ && "
        "cp /trusted-authority.rs /work/varieties/magic/session/src/bin/rav_light_of_sanction_pvp.rs && "
        "cd /work && cargo run --offline --locked -j 2 --manifest-path varieties/magic/Cargo.toml "
        "-p cardbench-magic-session --bin rav-light-of-sanction-pvp --" + candidate_flag,
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
        raise FileNotFoundError(f"trusted Light of Sanction authority is missing: {authority}")
    if not shutil.which("docker"):
        raise FileNotFoundError("Docker is required for Magic candidate isolation")
    verify(workspace, manifest)
    scoring_workspace = make_scoring_snapshot(workspace, manifest_data["files"], output_root / "candidate-workspace")
    candidate_hash = sha256((scoring_workspace / TARGET).read_bytes())
    name = f"cardbench-magic-light-of-sanction-{uuid.uuid4().hex[:16]}"
    command = sandbox_command(scoring_workspace, registry.resolve(), authority, name, image, candidate_kind)
    code, stdout, stderr = run_bounded(command, timeout=900, name=name)
    (output_root / "worker.stdout").write_bytes(stdout)
    (output_root / "worker.stderr").write_bytes(stderr)
    if code != 0:
        report = {
            "schema_version": "cardbench.magic.card-pvp.check.v1", "task_id": "cardbench/magic/card",
            "candidate": {"kind": candidate_kind, "source_sha256": candidate_hash},
            "passed": False, "pilot_card_passed": False, "cards_passed": 0,
            "cards_total": 291, "printings_total": 306, "cards_credited": 0,
            "coverage_complete": False, "full_reward": False, "harbor_reward": 0.0, "pilot_reward": 0.0,
            "error": "candidate compile or Light of Sanction reference trace failed",
            "diagnostics": stderr.decode(errors="replace")[-12000:],
        }
    else:
        report = parse_report(stdout)
        report["candidate"] = {"kind": candidate_kind, "card_definition": "RAV-LIGHT-OF-SANCTION", "source_sha256": candidate_hash, "materialized_workspace_sha256": manifest_data["workspace_sha256"]}
        report["candidate_kind"] = candidate_kind
        report["starting_workspace_sha256"] = manifest_data["workspace_sha256"]
        report["sealed_authority"] = False
        report["worker_image"] = image
        report["sandbox"] = {"network": "none", "rootfs": "read-only", "pids_limit": 128}
        report["harbor_reward"] = 0.0
        report["pilot_reward"] = 0.0
        report["full_reward"] = False
        report["cards_credited"] = 0
        report["coverage_complete"] = False
    (output_root / "result.json").write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    (output_root / "reward.txt").write_text("0.00000000\n", encoding="utf-8")
    print(f"candidate_passed={report.get('pilot_card_passed', False)} cards={report.get('cards_passed', 0)}/291 coverage_complete=false")
    return 0 if report.get("passed") is True and report.get("pilot_card_passed") is True else 1


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--workspace", required=True, type=Path)
    parser.add_argument("--manifest", required=True, type=Path)
    parser.add_argument("--output-root", required=True, type=Path)
    parser.add_argument("--candidate-kind", choices=["reference", "agent"], default="agent")
    args = parser.parse_args()
    return score(args.workspace.resolve(), args.manifest.resolve(), args.output_root.resolve(), args.candidate_kind)


if __name__ == "__main__":
    raise SystemExit(main())
