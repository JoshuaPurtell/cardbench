#!/usr/bin/env python3
"""Run the Caregiver PvP authority in a bounded, networkless worker."""

from __future__ import annotations

import argparse
import json
import os
import shutil
import subprocess
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parents[3]
MATERIALIZER = Path(__file__).with_name("materialize_caregiver_task.py")
TARGET = Path("varieties/magic/sets/ravnica_city_of_guilds/src/cards/caregiver.rs")
AUTHORITY = Path("varieties/magic/session/src/bin/rav_caregiver_pvp.rs")
DEFAULT_IMAGE = "rust@sha256:d7482085ff5b415f84dba5647ae71606650bdef00db7aeb69f4b3d170c3e4082"


class ScoreFailure(RuntimeError):
    pass


def validate_report(report: dict) -> None:
    if (
        report.get("schema_version") != "cardbench.magic.card-pvp.check.v1"
        or report.get("task_id") != "cardbench/magic/card"
        or report.get("passed") is not True
        or report.get("pilot_card_passed") is not True
        or report.get("card") != {"collector_number": 6, "definition_id": "RAV-CAREGIVER", "name": "Caregiver"}
        or report.get("cards_passed") != 1
        or report.get("cards_total") != 291
        or report.get("printings_total") != 306
        or report.get("cards_credited") != 0
        or report.get("coverage_complete") is not False
        or report.get("full_reward") is not False
        or report.get("harbor_reward") != 0.0
        or report.get("pilot_reward") != 0.0
        or report.get("viewer_scope") != "Judge"
        or report.get("deterministic_replay") is not True
    ):
        raise ScoreFailure("candidate_protocol_error: incomplete Caregiver 1/291 zero-reward report")
    traces = report.get("traces")
    if not isinstance(traces, list) or len(traces) != 1:
        raise ScoreFailure("candidate_protocol_error: one full-game Caregiver trace is required")
    trace = traces[0]
    if (
        not isinstance(trace, dict)
        or trace.get("trace_id") != "rav-caregiver-sacrifice-targeted-prevention-full-game"
        or trace.get("passed") is not True
        or trace.get("terminal") != "Winner(PlayerId(0))"
        or trace.get("winner_seat") != 0
        or trace.get("rejected_moves") != 0
        or not isinstance(trace.get("shuffle_seed"), int)
    ):
        raise ScoreFailure("candidate_protocol_error: Caregiver trace did not reach a clean terminal win")
    caregiver = trace.get("caregiver")
    activation = trace.get("activation")
    combat = trace.get("combat")
    wolf = combat.get("watchwolf") if isinstance(combat, dict) else None
    if (
        not isinstance(caregiver, dict)
        or caregiver.get("collector_number") != 6
        or caregiver.get("mana_cost") != "{W}"
        or caregiver.get("mana_paid") != ["White"]
        or caregiver.get("colors") != ["White"]
        or caregiver.get("card_types") != ["Creature"]
        or caregiver.get("power") != 1
        or caregiver.get("toughness") != 1
        or caregiver.get("characteristics_checked_on_battlefield_entry") is not True
        or caregiver.get("zone_after_cost") != "Graveyard"
        or not isinstance(activation, dict)
        or activation.get("ability") != "prevent-one-damage"
        or activation.get("mana_cost") != "{W}"
        or activation.get("mana_paid") != "White"
        or activation.get("sacrificed_source") != caregiver.get("object")
        or activation.get("shield_target") != "PlayerId(0)"
        or activation.get("shield_amount") != 1
        or activation.get("prevention_resolved_before_combat_damage") is not True
        or not isinstance(combat, dict)
        or combat.get("damage_prevented") != 1
        or combat.get("prevented_damage_was_not_dealt") is not True
        or not isinstance(wolf, dict)
        or wolf.get("power") != 3
        or wolf.get("toughness") != 3
        or not isinstance(wolf.get("attacks"), int)
        or wolf.get("attacks", 0) < 1
        or not isinstance(wolf.get("total_damage_to_opponent"), int)
        or wolf["total_damage_to_opponent"] < 20
        or not isinstance(combat.get("terminal_life"), list)
        or combat["terminal_life"][0] <= 0
        or combat["terminal_life"][1] > 0
    ):
        raise ScoreFailure("candidate_protocol_error: exact white payments, source sacrifice, one-point shield, prevention, or lethal combat differ")

    events = trace.get("events")
    if not isinstance(events, list) or not all(isinstance(event, dict) for event in events):
        raise ScoreFailure("candidate_protocol_error: typed event projection is missing")
    kinds = [event.get("kind") for event in events]
    required = {
        "SpellManaPaid", "SpellCast", "SpellResolved", "CardMoved", "AbilityManaPaid",
        "SacrificedAsAbilityCost", "AbilityActivated", "DamageShieldCreated", "AbilityResolved",
        "AttackersDeclared", "BlockersDeclared", "DamagePrevented", "DamageDealtToPlayer", "GameEnded",
    }
    if not required.issubset(kinds):
        raise ScoreFailure("candidate_protocol_error: cast, cost, prevention, combat, or terminal event kind omitted")
    seqs = [event.get("sequence") for event in events]
    if any(not isinstance(seq, int) for seq in seqs) or seqs != sorted(set(seqs)):
        raise ScoreFailure("candidate_protocol_error: event sequences are not strictly increasing")

    caregiver_id = caregiver.get("object")
    elf_id = combat.get("attacking_elf")
    wolf_id = wolf.get("object")
    def event_index(kind: str, *, obj: int | None = None, text: str | None = None) -> int:
        for index, event in enumerate(events):
            if event.get("kind") != kind:
                continue
            facts = event.get("facts", {})
            canonical = event.get("canonical", "")
            if obj is not None and obj not in facts.get("objects", []) and f"ObjectId({obj})" not in canonical:
                continue
            if text is not None and text not in canonical:
                continue
            return index
        raise ScoreFailure(f"candidate_protocol_error: required {kind} event for {obj or text} is missing")

    paid = event_index("SpellManaPaid", obj=caregiver_id)
    cast = event_index("SpellCast", obj=caregiver_id)
    resolved = event_index("SpellResolved", obj=caregiver_id)
    entered = event_index("CardMoved", obj=caregiver_id, text="to: Battlefield")
    ability_paid = event_index("AbilityManaPaid", obj=caregiver_id)
    sacrificed = event_index("SacrificedAsAbilityCost", obj=caregiver_id)
    moved = event_index("CardMoved", obj=caregiver_id, text="to: Graveyard")
    activated = event_index("AbilityActivated", obj=caregiver_id)
    shield = event_index("DamageShieldCreated", obj=caregiver_id)
    ability_resolved = event_index("AbilityResolved", obj=caregiver_id)
    prevented = event_index("DamagePrevented", obj=elf_id)
    wolf_paid = event_index("SpellManaPaid", obj=wolf_id)
    wolf_cast = event_index("SpellCast", obj=wolf_id)
    wolf_resolved = event_index("SpellResolved", obj=wolf_id)
    wolf_entered = event_index("CardMoved", obj=wolf_id, text="to: Battlefield")
    wolf_attack = event_index("AttackersDeclared", obj=wolf_id)
    wolf_damage = event_index("DamageDealtToPlayer", obj=wolf_id)
    terminal = event_index("GameEnded", text="winner: Some(PlayerId(0))")
    if not (paid < cast < resolved < entered < ability_paid < sacrificed < moved < activated < shield < ability_resolved < prevented < wolf_paid < wolf_cast < wolf_resolved < wolf_entered < wolf_attack < wolf_damage < terminal):
        raise ScoreFailure("candidate_protocol_error: Caregiver's exact-cost event order differs")
    payment_facts = events[paid].get("facts", {})
    activation_facts = events[ability_paid].get("facts", {})
    shield_canonical = events[shield].get("canonical", "")
    prevention_canonical = events[prevented].get("canonical", "")
    if payment_facts.get("mana_colors") != ["White"] or activation_facts.get("mana_cost_colors") != ["White"] or activation_facts.get("mana_cost_generic") != 0:
        raise ScoreFailure("candidate_protocol_error: Caregiver must pay exact white symbols for casting and activation")
    if "permanent: ObjectId(" + str(caregiver_id) + ")" not in events[sacrificed].get("canonical", "") or events[moved].get("facts", {}).get("zone") != "Graveyard":
        raise ScoreFailure("candidate_protocol_error: Caregiver must pay source sacrifice and move to its graveyard")
    if "Player(PlayerId(0))" not in shield_canonical or "amount: 1" not in shield_canonical or "Player(PlayerId(0))" not in prevention_canonical:
        raise ScoreFailure("candidate_protocol_error: one shield must target the controller and prevent the Elf's damage")
    mutation = report.get("mutation_sensitivity", {})
    expected_mutants = ("wrong_white_activation_cost_detected_in_game", "missing_source_sacrifice_detected_in_game", "missing_one_damage_shield_detected_in_game", "wrong_white_cast_cost_detected_in_game")
    if not isinstance(mutation, dict) or any(mutation.get(name) is not True for name in expected_mutants):
        raise ScoreFailure("candidate_protocol_error: all compiled Caregiver semantic mutants must be rejected")


def run_candidate(workspace: Path, image: str) -> dict:
    if not (image.startswith("rust@sha256:") or image.startswith("sha256:")):
        raise ValueError("CARDBENCH_MAGIC_WORKER_IMAGE must be an immutable digest")
    cargo_registry = Path.home() / ".cargo" / "registry"
    if not cargo_registry.is_dir():
        raise FileNotFoundError(f"offline Cargo registry cache is missing: {cargo_registry}")
    authority = REPO / AUTHORITY
    if authority.is_symlink() or not authority.is_file():
        raise FileNotFoundError(f"trusted Caregiver authority is missing: {authority}")
    name = "cardbench-caregiver-" + os.urandom(6).hex()
    command = [
        "docker", "run", "--rm", "--name", name, "--network", "none", "--read-only",
        "--cap-drop", "ALL", "--security-opt", "no-new-privileges", "--pids-limit", "128",
        "--memory", "3g", "--cpus", "2", "--user", "1000:1000",
        "--tmpfs", "/work:rw,exec,size=2210483648,uid=1000,gid=1000",
        "--tmpfs", "/tmp:rw,noexec,size=67108864,uid=1000,gid=1000",
        "--mount", f"type=bind,src={workspace},dst=/submission,readonly",
        "--mount", f"type=bind,src={cargo_registry.resolve()},dst=/usr/local/cargo/registry,readonly",
        "--mount", f"type=bind,src={authority.resolve()},dst=/trusted-authority.rs,readonly",
        "--env", "HOME=/tmp/home", "--env", "CARGO_HOME=/usr/local/cargo", "--env", "RUSTUP_HOME=/usr/local/rustup",
        "--env", "CARGO_NET_OFFLINE=true", "--env", "CARGO_BUILD_JOBS=2", image, "sh", "-c",
        "cp -R /submission/. /work/ && cp /trusted-authority.rs /work/varieties/magic/session/src/bin/rav_caregiver_pvp.rs && cd /work && cargo run --offline --locked -j 2 --manifest-path varieties/magic/Cargo.toml -p cardbench-magic-session --bin rav-caregiver-pvp -- --candidate",
    ]
    completed = subprocess.run(command, stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=300, check=False)
    if completed.returncode != 0:
        raise ScoreFailure("candidate_failed: isolated Caregiver authority returned nonzero: " + completed.stderr.decode(errors="replace")[-5000:])
    for line in reversed(completed.stdout.decode(errors="replace").splitlines()):
        try:
            report = json.loads(line)
        except json.JSONDecodeError:
            continue
        if report.get("schema_version") == "cardbench.magic.card-pvp.check.v1":
            validate_report(report)
            return report
    raise ScoreFailure("candidate_protocol_error: no Caregiver PvP report was returned")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--workspace", required=True, type=Path)
    parser.add_argument("--manifest", required=True, type=Path)
    parser.add_argument("--output-root", required=True, type=Path)
    parser.add_argument("--candidate-kind", choices=("agent", "reference"), default="agent")
    args = parser.parse_args()
    workspace = args.workspace.resolve()
    manifest = args.manifest.resolve()
    subprocess.run([sys.executable, str(MATERIALIZER), "--workspace", str(workspace), "--manifest", str(manifest), "--verify"], cwd=REPO, check=True)
    report = run_candidate(workspace, os.environ.get("CARDBENCH_MAGIC_WORKER_IMAGE", DEFAULT_IMAGE))
    report["candidate_kind"] = args.candidate_kind
    args.output_root.mkdir(parents=True, exist_ok=True)
    (args.output_root / "caregiver-reference-report.json").write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    print(json.dumps({"schema_version": "cardbench.magic.card-task-score.v1", "passed": True, "cards_credited": 0, "cards_total": 291, "printings_total": 306, "reward": 0.0, "qualification_status": "diagnostic_only", "candidate_kind": args.candidate_kind}))
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (ScoreFailure, FileNotFoundError, subprocess.CalledProcessError, subprocess.TimeoutExpired, ValueError) as error:
        print(str(error), file=sys.stderr)
        raise SystemExit(1)
