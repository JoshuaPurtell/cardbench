#!/usr/bin/env python3
"""Validate and score the isolated Elves of Deep Shadow PvP diagnostic."""

from __future__ import annotations

import argparse
import json
import os
import subprocess
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parents[3]
MATERIALIZER = Path(__file__).with_name("materialize_elves_of_deep_shadow_task.py")
TARGET = Path("varieties/magic/sets/ravnica_city_of_guilds/src/cards/elves_of_deep_shadow.rs")
AUTHORITY = Path("varieties/magic/session/src/bin/rav_elves_of_deep_shadow_pvp.rs")
DEFAULT_IMAGE = "rust@sha256:d7482085ff5b415f84dba5647ae71606650bdef00db7aeb69f4b3d170c3e4082"


class ScoreFailure(RuntimeError):
    pass


def validate_report(report: dict) -> None:
    if (
        report.get("schema_version") != "cardbench.magic.card-pvp.check.v1"
        or report.get("task_id") != "cardbench/magic/card"
        or report.get("passed") is not True
        or report.get("pilot_card_passed") is not True
        or report.get("card") != {"collector_number": 161, "definition_id": "RAV-ELVES-OF-DEEP-SHADOW", "name": "Elves of Deep Shadow"}
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
        raise ScoreFailure("candidate_protocol_error: incomplete Elves of Deep Shadow 1/291 zero-reward report")

    trace_list = report.get("traces")
    if not isinstance(trace_list, list) or len(trace_list) != 1:
        raise ScoreFailure("candidate_protocol_error: one full-game Elves of Deep Shadow trace is required")
    trace = trace_list[0]
    if (
        not isinstance(trace, dict)
        or trace.get("trace_id") != "rav-elves-of-deep-shadow-black-mana-drawback-full-game"
        or trace.get("passed") is not True
        or trace.get("terminal") != "Winner(PlayerId(0))"
        or trace.get("winner_seat") != 0
        or trace.get("rejected_moves") != 0
        or not isinstance(trace.get("shuffle_seed"), int)
    ):
        raise ScoreFailure("candidate_protocol_error: Elves of Deep Shadow trace did not reach a clean terminal win")

    elf = trace.get("elves")
    mana = trace.get("mana_ability")
    gasp = trace.get("last_gasp")
    combat = trace.get("combat")
    if (
        not isinstance(elf, dict)
        or elf.get("collector_number") != 161
        or elf.get("mana_cost") != "{G}"
        or elf.get("mana_paid") != ["Green"]
        or elf.get("colors") != ["Green"]
        or elf.get("card_types") != ["Creature"]
        or elf.get("power") != 1
        or elf.get("toughness") != 1
        or elf.get("zone") != "Battlefield"
        or not isinstance(mana, dict)
        or mana.get("id") != "produce-black-and-damage-controller"
        or mana.get("source") != elf.get("object")
        or mana.get("mana_added") != ["Black"]
        or mana.get("tap_cost_paid") is not True
        or mana.get("controller_damage") != 1
        or not isinstance(gasp, dict)
        or gasp.get("mana_cost") != "{1}{B}"
        or gasp.get("target_zone_after_resolution") != "Graveyard"
        or gasp.get("spell_zone_after_resolution") != "Graveyard"
        or gasp.get("state_action") != "creature has toughness zero or less"
        or not isinstance(combat, dict)
        or combat.get("own_watchwolf_power") != 3
        or combat.get("own_watchwolf_toughness") != 3
        or not isinstance(combat.get("total_damage_to_opponent"), int)
        or combat.get("total_damage_to_opponent", 0) < 20
        or not isinstance(combat.get("terminal_life"), list)
        or combat["terminal_life"][0] <= 0
        or combat["terminal_life"][1] > 0
    ):
        raise ScoreFailure("candidate_protocol_error: printed Elves characteristics, black mana drawback, Last Gasp, or terminal combat differ")

    events = trace.get("events")
    if not isinstance(events, list) or not all(isinstance(item, dict) for item in events):
        raise ScoreFailure("candidate_protocol_error: typed Elves event projection is missing")
    seqs = [item.get("sequence") for item in events]
    if any(not isinstance(seq, int) for seq in seqs) or seqs != sorted(set(seqs)):
        raise ScoreFailure("candidate_protocol_error: Elves event sequence is not strictly increasing")
    kinds = {item.get("kind") for item in events}
    required = {"SpellManaPaid", "SpellCast", "SpellResolved", "CardMoved", "BoundManaAbilityActivated", "ManaAdded", "DamageDealtToPlayer", "AttackersDeclared", "StateBasedAction", "GameEnded"}
    if not required.issubset(kinds):
        raise ScoreFailure("candidate_protocol_error: cast, bound-mana, drawback, removal, combat, or terminal event is omitted")

    def index(kind: str, object_id: int | None = None, fragment: str | None = None, *, after: int = -1) -> int:
        for position, item in enumerate(events):
            if position <= after or item.get("kind") != kind:
                continue
            canonical = item.get("canonical", "")
            objects = item.get("facts", {}).get("objects", [])
            object_matches = object_id is None or f"ObjectId({object_id})" in canonical or object_id in objects or any(isinstance(obj, dict) and obj.get("id") == object_id for obj in objects)
            if object_matches and (fragment is None or fragment in canonical):
                return position
        raise ScoreFailure(f"candidate_protocol_error: missing {kind} for {object_id or fragment}")

    elf_id = elf["object"]
    gasp_id = gasp["object"]
    enemy_wolf = gasp.get("target")
    own_wolf = combat.get("own_watchwolf")
    paid_elf = index("SpellManaPaid", elf_id)
    cast_elf = index("SpellCast", elf_id)
    resolved_elf = index("SpellResolved", elf_id)
    entered_elf = index("CardMoved", elf_id, "to: Battlefield")
    opponent_elf_id = trace.get("opponent_elf_object")
    opponent_wolf_id = combat.get("enemy_watchwolf")
    own_wolf_id = combat.get("own_watchwolf")
    elf_payment = events[paid_elf].get("facts", {}).get("mana_colors")
    if elf_payment != ["Green"]:
        raise ScoreFailure("candidate_protocol_error: Elves of Deep Shadow must be paid with exactly {G}")
    opponent_elf_payment = index("SpellManaPaid", opponent_elf_id)
    opponent_elf_cast = index("SpellCast", opponent_elf_id)
    opponent_elf_entry = index("CardMoved", opponent_elf_id, "to: Battlefield")
    if events[opponent_elf_payment].get("facts", {}).get("mana_colors") != ["Green"] or not (opponent_elf_payment < opponent_elf_cast < opponent_elf_entry):
        raise ScoreFailure("candidate_protocol_error: opponent Elves must pay exact green and resolve onto the battlefield")
    opponent_wolf_payment = index("SpellManaPaid", opponent_wolf_id)
    opponent_wolf_cast = index("SpellCast", opponent_wolf_id)
    opponent_wolf_entry = index("CardMoved", opponent_wolf_id, "to: Battlefield")
    opponent_attack = index("AttackersDeclared", opponent_wolf_id)
    opponent_wolf_colors = events[opponent_wolf_payment].get("facts", {}).get("mana_colors", [])
    if len(opponent_wolf_colors) != 2 or "Green" not in opponent_wolf_colors or "White" not in opponent_wolf_colors or not (opponent_wolf_payment < opponent_wolf_cast < opponent_wolf_entry < opponent_attack):
        raise ScoreFailure("candidate_protocol_error: opponent Watchwolf must pay exact green/white and attack")
    activation = index("BoundManaAbilityActivated", elf_id, "produce-black-and-damage-controller")
    mana_added = index("ManaAdded", fragment="Black", after=activation)
    drawback = index("DamageDealtToPlayer", elf_id, "player: PlayerId(0)", after=mana_added)
    if events[activation].get("canonical", "").find("color: Black") < 0 or events[activation].get("canonical", "").find("tapped: true") < 0 or events[mana_added].get("facts", {}).get("amount") != 1 or events[mana_added].get("facts", {}).get("zone") != "Black" or events[drawback].get("facts", {}).get("amount") != 1:
        raise ScoreFailure("candidate_protocol_error: bound Elves ability must tap for one black and deal one controller damage")
    gasp_payment = index("SpellManaPaid", gasp_id, after=drawback)
    gasp_colors = events[gasp_payment].get("facts", {}).get("mana_colors", [])
    if len(gasp_colors) != 2 or "Black" not in gasp_colors or not any(color != "Black" for color in gasp_colors):
        raise ScoreFailure("candidate_protocol_error: Last Gasp did not spend Elves' black mana plus one generic mana")
    gasp_cast = index("SpellCast", gasp_id, after=gasp_payment)
    resolved_gasp = index("SpellResolved", gasp_id, after=gasp_cast)
    zero_toughness = index("StateBasedAction", enemy_wolf, "toughness zero or less", after=resolved_gasp)
    dead_wolf = index("CardMoved", enemy_wolf, "to: Graveyard", after=zero_toughness)
    own_paid = index("SpellManaPaid", own_wolf_id)
    own_cast = index("SpellCast", own_wolf_id)
    own_resolved = index("SpellResolved", own_wolf_id)
    own_entered = index("CardMoved", own_wolf_id, "to: Battlefield")
    own_attack = index("AttackersDeclared", own_wolf_id)
    own_damage = index("DamageDealtToPlayer", own_wolf_id, "player: PlayerId(1)")
    own_colors = events[own_paid].get("facts", {}).get("mana_colors", [])
    if len(own_colors) != 2 or "Green" not in own_colors or "White" not in own_colors:
        raise ScoreFailure("candidate_protocol_error: Watchwolf finisher must pay exact green and white symbols")
    terminal = index("GameEnded", fragment="winner: Some(PlayerId(0))")
    own_elf_chain = paid_elf < cast_elf < resolved_elf < entered_elf
    opponent_elf_chain = opponent_elf_payment < opponent_elf_cast < opponent_elf_entry
    enemy_wolf_chain = opponent_wolf_payment < opponent_wolf_cast < opponent_wolf_entry < opponent_attack
    removal_chain = opponent_attack < activation < mana_added < drawback < gasp_payment < gasp_cast < resolved_gasp < zero_toughness < dead_wolf
    finisher_chain = dead_wolf < own_paid < own_cast < own_resolved < own_entered < own_attack < own_damage < terminal
    if not (own_elf_chain and opponent_elf_chain and enemy_wolf_chain and entered_elf < opponent_attack and removal_chain and finisher_chain):
        raise ScoreFailure("candidate_protocol_error: Elves payment, mana output/drawback, Last Gasp, and terminal combat event order differs")

    mutations = report.get("mutation_sensitivity", {})
    expected = ("wrong_green_cast_cost_detected", "wrong_live_power_detected", "wrong_mana_color_detected", "controller_damage_removed_detected", "tap_cost_removed_detected")
    if not isinstance(mutations, dict) or any(mutations.get(key) is not True for key in expected):
        raise ScoreFailure("candidate_protocol_error: all five compiled Elves semantic mutants must be rejected")


def run_candidate(workspace: Path, image: str) -> dict:
    if not (image.startswith("rust@sha256:") or image.startswith("sha256:")):
        raise ValueError("CARDBENCH_MAGIC_WORKER_IMAGE must be an immutable digest")
    cargo_registry = Path.home() / ".cargo" / "registry"
    if not cargo_registry.is_dir():
        raise FileNotFoundError(f"offline Cargo registry cache is missing: {cargo_registry}")
    authority = REPO / AUTHORITY
    if authority.is_symlink() or not authority.is_file():
        raise FileNotFoundError(f"trusted Elves of Deep Shadow authority is missing: {authority}")
    name = "cardbench-elves-deep-shadow-" + os.urandom(6).hex()
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
        "cp -R /submission/. /work/ && cp /trusted-authority.rs /work/varieties/magic/session/src/bin/rav_elves_of_deep_shadow_pvp.rs && cd /work && cargo run --offline --locked -j 2 --manifest-path varieties/magic/Cargo.toml -p cardbench-magic-session --bin rav-elves-of-deep-shadow-pvp -- --candidate",
    ]
    completed = subprocess.run(command, stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=300, check=False)
    if completed.returncode != 0:
        raise ScoreFailure("candidate_failed: isolated Elves of Deep Shadow authority returned nonzero: " + completed.stderr.decode(errors="replace")[-5000:])
    for line in reversed(completed.stdout.decode(errors="replace").splitlines()):
        try:
            report = json.loads(line)
        except json.JSONDecodeError:
            continue
        if report.get("schema_version") == "cardbench.magic.card-pvp.check.v1":
            validate_report(report)
            return report
    raise ScoreFailure("candidate_protocol_error: no Elves of Deep Shadow PvP report was returned")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--workspace", required=True, type=Path)
    parser.add_argument("--manifest", required=True, type=Path)
    parser.add_argument("--output-root", required=True, type=Path)
    parser.add_argument("--candidate-kind", choices=("agent", "reference"), default="agent")
    args = parser.parse_args()
    subprocess.run([sys.executable, str(MATERIALIZER), "--workspace", str(args.workspace.resolve()), "--manifest", str(args.manifest.resolve()), "--verify"], cwd=REPO, check=True)
    report = run_candidate(args.workspace.resolve(), os.environ.get("CARDBENCH_MAGIC_WORKER_IMAGE", DEFAULT_IMAGE))
    report["candidate_kind"] = args.candidate_kind
    args.output_root.mkdir(parents=True, exist_ok=True)
    (args.output_root / "elves-of-deep-shadow-reference-report.json").write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    print(json.dumps({"schema_version": "cardbench.magic.card-task-score.v1", "passed": True, "cards_credited": 0, "cards_total": 291, "printings_total": 306, "reward": 0.0, "qualification_status": "diagnostic_only", "candidate_kind": args.candidate_kind}))
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (ScoreFailure, FileNotFoundError, subprocess.CalledProcessError, subprocess.TimeoutExpired, ValueError) as error:
        print(str(error), file=sys.stderr)
        raise SystemExit(1)
