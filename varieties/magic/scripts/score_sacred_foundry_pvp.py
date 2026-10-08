#!/usr/bin/env python3
"""Score a Sacred Foundry submission in an offline, isolated worker."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import shutil
import uuid
from pathlib import Path

from score_watchwolf_pvp import WorkerFailure, make_scoring_snapshot, run_bounded
from materialize_sacred_foundry_task import verify

REPO = Path(__file__).resolve().parents[3]
AUTHORITY_SOURCE = Path("varieties/magic/session/src/bin/rav_sacred_foundry_pvp.rs")
TARGET = Path("varieties/magic/sets/ravnica_city_of_guilds/src/cards/sacred_foundry.rs")
DEFAULT_WORKER_IMAGE = "rust@sha256:d7482085ff5b415f84dba5647ae71606650bdef00db7aeb69f4b3d170c3e4082"


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


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
        or report.get("card")
        != {"collector_number": 280, "definition_id": "RAV-SACRED-FOUNDRY", "name": "Sacred Foundry"}
    ):
        raise WorkerFailure("candidate_protocol_error: incomplete Sacred Foundry single-card report")
    traces = report.get("traces")
    if not isinstance(traces, list) or len(traces) != 1:
        raise WorkerFailure("candidate_protocol_error: one Sacred Foundry trace is required")
    trace = traces[0]
    choices = trace.get("land_entry_choices") if isinstance(trace, dict) else None
    if (
        not isinstance(trace, dict)
        or trace.get("id") != "rav-sacred-foundry-life-choice-and-mana"
        or trace.get("passed") is not True
        or trace.get("terminal") != "Winner(PlayerId(0))"
        or trace.get("winner_seat") != 0
        or trace.get("rejected_moves") != 0
        or trace.get("pilot_life_after_entry_payment") != 18
        or trace.get("white_and_red_mana_used") is not True
        or not isinstance(choices, list)
        or len(choices) != 2
    ):
        raise WorkerFailure("candidate_protocol_error: Sacred Foundry choices or terminal result differ")
    paid, declined = choices
    if (
        not isinstance(paid, dict)
        or not isinstance(declined, dict)
        or paid.get("card_object") == declined.get("card_object")
        or paid.get("choice") != "pay_two_life"
        or paid.get("life_paid") != 2
        or paid.get("controller_life_after_entry") != 18
        or paid.get("entered_tapped") is not False
        or declined.get("choice") != "decline"
        or declined.get("life_paid") != 0
        or declined.get("entered_tapped") is not True
    ):
        raise WorkerFailure("candidate_protocol_error: both exact Sacred Foundry entry branches are required")
    mana = trace.get("sacred_foundry_mana_receipts")
    required_receipts = {
        (paid["card_object"], "White", 1, True),
        (paid["card_object"], "Red", 1, True),
    }
    got_receipts = {
        (receipt.get("source"), receipt.get("color"), receipt.get("amount"), receipt.get("tapped"))
        for receipt in mana if isinstance(receipt, dict)
    } if isinstance(mana, list) else set()
    if not required_receipts.issubset(got_receipts):
        raise WorkerFailure("candidate_protocol_error: paid Sacred Foundry must produce one white and one red")

    hound = trace.get("gate_hound")
    recruit = trace.get("boros_recruit")
    if (
        not isinstance(hound, dict)
        or hound.get("name") != "Gate Hound"
        or hound.get("mana_cost") != "{2}{W}"
        or hound.get("cast_and_resolved") is not True
        or hound.get("power") != 1
        or hound.get("toughness") != 1
        or not isinstance(recruit, dict)
        or recruit.get("name") != "Boros Recruit"
        or recruit.get("mana_cost") != "{R/W}"
        or recruit.get("hybrid_choice") != "Red"
        or recruit.get("payment") != ["Red"]
        or recruit.get("cast_and_resolved") is not True
        or recruit.get("power") != 1
        or recruit.get("toughness") != 1
        or recruit.get("first_strike") is not True
    ):
        raise WorkerFailure("candidate_protocol_error: Foundry mana was not used to cast the support creatures")

    mana_order = trace.get("colored_mana_use_order")
    order_keys = (
        "paid_foundry_white_event_index", "gate_hound_payment_event_index",
        "paid_foundry_red_event_index", "boros_recruit_payment_event_index",
    )
    if not isinstance(mana_order, dict) or any(not isinstance(mana_order.get(key), int) for key in order_keys):
        raise WorkerFailure("candidate_protocol_error: Foundry mana use order is incomplete")
    if not (
        mana_order[order_keys[0]] < mana_order[order_keys[1]]
        and mana_order[order_keys[2]] < mana_order[order_keys[3]]
    ):
        raise WorkerFailure("candidate_protocol_error: each paid Foundry color must precede its spell payment")

    mutations = report.get("mutation_sensitivity")
    required_mutations = (
        "entry_life_amount_mutation_detected", "forced_tapped_entry_mutation_detected",
        "mana_color_choice_mutation_detected", "entry_choice_removed_mutation_detected",
        "mana_tap_cost_mutation_detected",
    )
    if not isinstance(mutations, dict) or any(mutations.get(key) is not True for key in required_mutations):
        raise WorkerFailure("candidate_protocol_error: Sacred Foundry semantic mutation controls are incomplete")

    events = trace.get("events")
    if not isinstance(events, list) or not all(isinstance(event, dict) for event in events):
        raise WorkerFailure("candidate_protocol_error: Sacred Foundry event trace is malformed")
    sequences = [event.get("sequence") for event in events]
    if any(not isinstance(value, int) for value in sequences) or sequences != sorted(sequences):
        raise WorkerFailure("candidate_protocol_error: Sacred Foundry event sequence is malformed")
    ids = [paid["card_object"], declined["card_object"], hound.get("object"), recruit.get("object")]
    if any(not isinstance(value, int) for value in ids) or len(set(ids)) != len(ids):
        raise WorkerFailure("candidate_protocol_error: Sacred Foundry trace object identities are invalid")

    def event_index(kind: str, object_id: int, *, amount: int | None = None) -> int | None:
        for index, event in enumerate(events):
            facts = event.get("facts", {})
            if event.get("kind") == kind and facts.get("objects") == [object_id]:
                if amount is None or facts.get("amount") == amount:
                    return index
        return None

    hound_payment = event_index("SpellManaPaid", hound["object"], amount=3)
    hound_cast = event_index("SpellCast", hound["object"])
    hound_resolved = event_index("SpellResolved", hound["object"])
    recruit_payment = event_index("SpellManaPaid", recruit["object"], amount=1)
    recruit_cast = event_index("SpellCast", recruit["object"])
    recruit_resolved = event_index("SpellResolved", recruit["object"])
    terminal = next((i for i, event in enumerate(events) if event.get("kind") == "GameEnded" and event.get("facts", {}).get("seats") == [0]), None)
    if any(value is None for value in (hound_payment, hound_cast, hound_resolved, recruit_payment, recruit_cast, recruit_resolved, terminal)):
        raise WorkerFailure("candidate_protocol_error: both creature casts, resolutions, and terminal result are required")
    hound_colors = events[hound_payment].get("facts", {}).get("mana_colors", [])
    recruit_colors = events[recruit_payment].get("facts", {}).get("mana_colors", [])
    if len(hound_colors) != 3 or "White" not in hound_colors:
        raise WorkerFailure("candidate_protocol_error: Gate Hound payment must include Foundry white and two generic")
    if recruit_colors != ["Red"]:
        raise WorkerFailure("candidate_protocol_error: Boros Recruit must spend Foundry red for its hybrid symbol")
    attack = next((i for i, event in enumerate(events) if event.get("kind") == "AttackersDeclared" and recruit["object"] in event.get("facts", {}).get("objects", [])), None)
    damage = next((i for i, event in enumerate(events) if event.get("kind") == "DamageDealtToPlayer" and event.get("facts", {}).get("objects") == [recruit["object"]] and event.get("facts", {}).get("amount") == 1), None)
    if attack is None or damage is None or not (hound_payment < hound_cast < hound_resolved < recruit_payment < recruit_cast < recruit_resolved < attack < damage < terminal):
        raise WorkerFailure("candidate_protocol_error: Sacred Foundry payment, cast, combat, and terminal order is invalid")


def parse_report(stdout: bytes) -> dict:
    for line in reversed(stdout.decode(errors="replace").splitlines()):
        try:
            value = json.loads(line)
        except json.JSONDecodeError:
            continue
        if isinstance(value, dict) and value.get("schema_version") == "cardbench.magic.card-pvp.check.v1":
            validate_report(value)
            return value
    raise WorkerFailure("candidate_protocol_error: no valid Sacred Foundry PvP result")


def sandbox_command(workspace: Path, registry: Path, authority: Path, name: str, image: str, candidate_kind: str) -> list[str]:
    if authority.is_symlink() or not authority.is_file():
        raise WorkerFailure("trusted Sacred Foundry authority source is missing")
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
        "cp /trusted-authority.rs /work/varieties/magic/session/src/bin/rav_sacred_foundry_pvp.rs && "
        "cd /work && cargo run --offline --locked -j 2 --manifest-path varieties/magic/Cargo.toml "
        "-p cardbench-magic-session --bin rav-sacred-foundry-pvp --" + candidate_flag,
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
        raise FileNotFoundError(f"trusted Sacred Foundry authority source is missing: {authority}")
    if not shutil.which("docker"):
        raise FileNotFoundError("Docker is required for Magic candidate isolation")
    verify(workspace, manifest)
    scoring_workspace = make_scoring_snapshot(workspace, manifest_data["files"], output_root / "candidate-workspace")
    candidate_hash = sha256((scoring_workspace / TARGET).read_bytes())
    name = f"cardbench-magic-sacred-foundry-{uuid.uuid4().hex[:16]}"
    command = sandbox_command(scoring_workspace, registry.resolve(), authority, name, image, candidate_kind)
    code, stdout, stderr = run_bounded(command, timeout=900, name=name)
    (output_root / "worker.stdout").write_bytes(stdout)
    (output_root / "worker.stderr").write_bytes(stderr)
    if code != 0:
        report = {"schema_version":"cardbench.magic.card-pvp.check.v1","task_id":"cardbench/magic/card","passed":False,"pilot_card_passed":False,"cards_passed":0,"cards_total":291,"printings_total":306,"cards_credited":0,"coverage_complete":False,"full_reward":False,"harbor_reward":0.0,"pilot_reward":0.0,"error":"candidate compile or Sacred Foundry reference trace failed","diagnostics":stderr.decode(errors="replace")[-12000:]}
    else:
        report = parse_report(stdout)
        report["candidate"] = {"kind":candidate_kind,"card_definition":"RAV-SACRED-FOUNDRY","source_sha256":candidate_hash,"materialized_workspace_sha256":manifest_data["workspace_sha256"]}
        report["candidate_kind"] = candidate_kind
        report["starting_workspace_sha256"] = manifest_data["workspace_sha256"]
        report["sealed_authority"] = False
        report["worker_image"] = image
        report["sandbox"] = {"network":"none","rootfs":"read-only","pids_limit":128}
        report["harbor_reward"] = 0.0
        report["pilot_reward"] = 0.0
        report["full_reward"] = False
    (output_root / "result.json").write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    (output_root / "reward.txt").write_text("0.00000000\n", encoding="utf-8")
    print(f"candidate_passed={report.get('pilot_card_passed', False)} cards={report.get('cards_passed',0)}/291 coverage_complete=false")
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
