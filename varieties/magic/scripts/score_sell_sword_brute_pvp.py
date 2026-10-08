#!/usr/bin/env python3
"""Score the isolated Sell-Sword Brute candidate with its trusted PvP trace."""

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
MATERIALIZER = Path(__file__).with_name("materialize_sell_sword_brute_task.py")
TARGET = Path("varieties/magic/sets/ravnica_city_of_guilds/src/cards/sell_sword_brute.rs")
AUTHORITY = Path("varieties/magic/session/src/bin/rav_sell_sword_brute_pvp.rs")
DEFAULT_WORKER_IMAGE = "rust@sha256:d7482085ff5b415f84dba5647ae71606650bdef00db7aeb69f4b3d170c3e4082"
MAX_OUTPUT_BYTES = 2 * 1024 * 1024


class WorkerFailure(RuntimeError):
    pass


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def verify_workspace(workspace: Path, manifest: Path) -> dict:
    subprocess.run(
        [sys.executable, str(MATERIALIZER), "--workspace", str(workspace), "--manifest", str(manifest), "--verify"],
        check=True,
        cwd=REPO,
    )
    return json.loads(manifest.read_text(encoding="utf-8"))


def make_snapshot(workspace: Path, files: dict[str, str], destination: Path) -> Path:
    if destination.is_symlink():
        raise WorkerFailure("candidate_snapshot_path_is_symlink")
    if destination.exists():
        shutil.rmtree(destination)
    destination.mkdir(parents=True)
    for relative in sorted(files):
        source = workspace / Path(relative)
        if source.is_symlink() or not source.is_file():
            raise WorkerFailure(f"candidate_snapshot_source_missing: {relative}")
        target = destination / Path(relative)
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(source, target)
    return destination


def sandbox_command(workspace: Path, registry: Path, name: str, image: str, candidate_kind: str) -> list[str]:
    authority = REPO / AUTHORITY
    if authority.is_symlink() or not authority.is_file():
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
        "--mount", f"type=bind,src={authority.resolve()},dst=/trusted-authority.rs,readonly",
        "--env", "HOME=/tmp/home", "--env", "CARGO_HOME=/usr/local/cargo",
        "--env", "RUSTUP_HOME=/usr/local/rustup", "--env", "CARGO_NET_OFFLINE=true",
        "--env", "CARGO_BUILD_JOBS=2", "--env", "RAYON_NUM_THREADS=2", image, "sh", "-c",
        "cp -R /submission/. /work/ && "
        "cp /trusted-authority.rs /work/varieties/magic/session/src/bin/rav_sell_sword_brute_pvp.rs && "
        "cd /work && cargo run --offline --locked -j 2 --manifest-path varieties/magic/Cargo.toml "
        "-p cardbench-magic-session --bin rav-sell-sword-brute-pvp --" + candidate_flag,
    ]


def validate_trace(trace: object) -> None:
    if not isinstance(trace, dict) or trace.get("id") != "rav-sell-sword-brute-dies-deals-two-to-controller" or trace.get("passed") is not True or trace.get("terminal") != "Winner(PlayerId(0))" or trace.get("winner_seat") != 0 or trace.get("rejected_moves") != 0:
        raise WorkerFailure("candidate_protocol_error: Sell-Sword Brute did not reach a clean terminal PvP win")
    candidate = trace.get("sell_sword_brute_object")
    gasp = trace.get("last_gasp_object")
    finisher = trace.get("win_condition_object")
    if not all(isinstance(value, int) for value in (candidate, gasp, finisher)):
        raise WorkerFailure("candidate_protocol_error: required object identities are missing")
    if trace.get("sell_sword_brute_zone") != "Some(Graveyard)":
        raise WorkerFailure("candidate_protocol_error: Sell-Sword Brute did not die")
    candidate_payment = trace.get("sell_sword_brute_cast_payment")
    gasp_payment = trace.get("last_gasp_cast_payment")
    if not isinstance(candidate_payment, dict) or candidate_payment.get("red_paid") is not True or candidate_payment.get("colors", []).count("Red") < 1 or len(candidate_payment.get("colors", [])) != 2:
        raise WorkerFailure("candidate_protocol_error: Sell-Sword Brute did not pay {1}{R}")
    if not isinstance(gasp_payment, dict) or gasp_payment.get("black_paid") is not True or gasp_payment.get("colors", []).count("Black") < 1 or len(gasp_payment.get("colors", [])) != 2:
        raise WorkerFailure("candidate_protocol_error: Last Gasp did not pay {1}{B}")
    trigger = trace.get("dies_trigger")
    if not isinstance(trigger, dict) or trigger.get("ability") != "dies-deal-two-to-controller" or trigger.get("source_zone_before_trigger") != "Graveyard" or not isinstance(trigger.get("source_incarnation"), int) or trigger.get("source_incarnation") != trigger.get("resolved_incarnation") or trigger.get("damage_to_controller") != 2:
        raise WorkerFailure("candidate_protocol_error: Sell-Sword Brute dies trigger facts differ")
    events = trace.get("events")
    if not isinstance(events, list) or not events:
        raise WorkerFailure("candidate_protocol_error: typed PvP events are missing")
    sequences = [event.get("sequence") for event in events if isinstance(event, dict)]
    if len(sequences) != len(events) or any(not isinstance(value, int) for value in sequences) or sequences != sorted(sequences):
        raise WorkerFailure("candidate_protocol_error: event sequence is malformed")

    def find(kind: str, *, obj: int | None = None, seat: int | None = None, amount: int | None = None, ability: str | None = None, zone: str | None = None, start: int = 0) -> int:
        for index in range(start, len(events)):
            event = events[index]
            facts = event.get("facts", {})
            if event.get("kind") != kind:
                continue
            if obj is not None and obj not in facts.get("objects", []):
                continue
            if seat is not None and seat not in facts.get("seats", []):
                continue
            if amount is not None and facts.get("amount") != amount:
                continue
            if ability is not None and ability not in event.get("canonical", ""):
                continue
            if zone is not None and facts.get("zone") != zone:
                continue
            return index
        raise WorkerFailure(f"candidate_protocol_error: missing {kind} receipt")

    positions: list[int] = []
    positions.append(find("SpellManaPaid", obj=candidate, seat=1))
    positions.append(find("SpellCast", obj=candidate, seat=1, start=positions[-1] + 1))
    positions.append(find("SpellResolved", obj=candidate, start=positions[-1] + 1))
    positions.append(find("CardMoved", obj=candidate, zone="Battlefield", start=positions[-1] + 1))
    positions.append(find("SpellManaPaid", obj=gasp, seat=0, start=positions[-1] + 1))
    positions.append(find("SpellCast", obj=gasp, seat=0, start=positions[-1] + 1))
    positions.append(find("SpellResolved", obj=gasp, start=positions[-1] + 1))
    positions.append(find("StateBasedAction", obj=candidate, start=positions[-1] + 1))
    positions.append(find("CardMoved", obj=candidate, zone="Graveyard", start=positions[-1] + 1))
    positions.append(find("TriggeredAbilityStacked", obj=candidate, seat=1, ability="dies-deal-two-to-controller", start=positions[-1] + 1))
    positions.append(find("DamageDealtToPlayer", obj=candidate, seat=1, amount=2, start=positions[-1] + 1))
    positions.append(find("AbilityResolved", obj=candidate, ability="dies-deal-two-to-controller", start=positions[-1] + 1))
    positions.append(find("AttackersDeclared", obj=finisher, seat=0, start=positions[-1] + 1))
    positions.append(find("DamageDealtToPlayer", obj=finisher, seat=1, amount=3, start=positions[-1] + 1))
    positions.append(find("GameEnded", seat=0, start=positions[-1] + 1))
    if positions != trace.get("ordered_event_sequence") or positions != sorted(positions):
        raise WorkerFailure("candidate_protocol_error: ordered Sell-Sword Brute, dies-trigger, and terminal events differ")
    if events[positions[7]].get("facts", {}).get("reason") != "creature has toughness zero or less":
        raise WorkerFailure("candidate_protocol_error: Brute death did not follow the toughness-zero state action")
    if events[positions[9]].get("facts", {}).get("incarnation") != trigger.get("source_incarnation") or events[positions[11]].get("facts", {}).get("incarnation") != trigger.get("resolved_incarnation"):
        raise WorkerFailure("candidate_protocol_error: trigger stack and resolution incarnation facts differ")
    candidate_mana = events[positions[0]].get("facts", {})
    gasp_mana = events[positions[4]].get("facts", {})
    if candidate_mana.get("objects") != [candidate] or candidate_mana.get("seats") != [1] or len(candidate_mana.get("mana_colors", [])) != 2 or "Red" not in candidate_mana.get("mana_colors", []):
        raise WorkerFailure("candidate_protocol_error: typed Sell-Sword Brute payment differs")
    if gasp_mana.get("objects") != [gasp] or gasp_mana.get("seats") != [0] or len(gasp_mana.get("mana_colors", [])) != 2 or "Black" not in gasp_mana.get("mana_colors", []):
        raise WorkerFailure("candidate_protocol_error: typed Last Gasp payment differs")
    stacked_facts = events[positions[9]].get("facts", {})
    damage_facts = events[positions[10]].get("facts", {})
    if stacked_facts.get("objects") != [candidate] or stacked_facts.get("ability") != "dies-deal-two-to-controller" or stacked_facts.get("seats") != [1]:
        raise WorkerFailure("candidate_protocol_error: dies trigger stack event is not bound to the Brute controller")
    if damage_facts.get("objects") != [candidate] or damage_facts.get("seats") != [1] or damage_facts.get("amount") != 2:
        raise WorkerFailure("candidate_protocol_error: typed controller damage receipt differs")
    life = trace.get("life_at_match_end")
    if not isinstance(life, list) or len(life) != 2 or life[0] <= 0 or life[1] > 0:
        raise WorkerFailure("candidate_protocol_error: terminal life totals do not identify seat zero as winner")


def validate_report(report: dict) -> None:
    if (
        report.get("schema_version") != "cardbench.magic.card-pvp.check.v1"
        or report.get("task_id") != "cardbench/magic/card"
        or report.get("passed") is not True
        or report.get("pilot_card_passed") is not True
        or report.get("replay_deterministic") is not True
        or report.get("cards_passed") != 1
        or report.get("cards_total") != 291
        or report.get("printings_total") != 306
        or report.get("cards_credited") != 0
        or report.get("coverage_complete") is not False
        or report.get("full_reward") is not False
        or report.get("harbor_reward") != 0.0
        or report.get("pilot_reward") != 0.0
        or report.get("viewer_scope") != "Judge"
        or report.get("card") != {"collector_number": 142, "definition_id": "RAV-SELL-SWORD-BRUTE", "name": "Sell-Sword Brute"}
        or report.get("rules_authority", {}).get("comprehensive_rules") != "2026-08-07"
    ):
        raise WorkerFailure("candidate_protocol_error: incomplete Sell-Sword Brute card result")
    candidate = report.get("candidate")
    if not isinstance(candidate, dict) or candidate.get("card_definition") != "RAV-SELL-SWORD-BRUTE":
        raise WorkerFailure("candidate_protocol_error: wrong candidate identity")
    traces = report.get("traces")
    if not isinstance(traces, list) or len(traces) != 1:
        raise WorkerFailure("candidate_protocol_error: one full-game Sell-Sword Brute trace is required")
    validate_trace(traces[0])
    mutation = report.get("mutation_sensitivity", {})
    if any(mutation.get(key) is not True for key in ("dies_trigger_removal_detected_in_game", "one_damage_mutation_detected_in_game")):
        raise WorkerFailure("candidate_protocol_error: Sell-Sword Brute semantic mutation controls are incomplete")


def parse_report(output: bytes) -> dict:
    for line in reversed(output.decode(errors="replace").splitlines()):
        try:
            report = json.loads(line)
        except json.JSONDecodeError:
            continue
        if report.get("schema_version") == "cardbench.magic.card-pvp.check.v1":
            validate_report(report)
            return report
    raise WorkerFailure("candidate_protocol_error: no valid Sell-Sword Brute PvP result")


def score(workspace: Path, manifest: Path, output_root: Path, candidate_kind: str) -> int:
    output_root.mkdir(parents=True, exist_ok=True)
    manifest_data = verify_workspace(workspace, manifest)
    worker_image = os.environ.get("CARDBENCH_MAGIC_WORKER_IMAGE", DEFAULT_WORKER_IMAGE)
    if not (worker_image.startswith("rust@sha256:") or worker_image.startswith("sha256:")):
        raise ValueError("CARDBENCH_MAGIC_WORKER_IMAGE must be an immutable digest reference")
    cargo_registry = Path.home() / ".cargo" / "registry"
    if not cargo_registry.is_dir():
        raise FileNotFoundError(f"offline Cargo registry cache is missing: {cargo_registry}")
    if not shutil.which("docker"):
        raise FileNotFoundError("Docker is required for isolated Magic candidate scoring")
    snapshot = make_snapshot(workspace, manifest_data["files"], output_root / "candidate-workspace")
    candidate_hash = sha256((snapshot / TARGET).read_bytes())
    name = f"cardbench-magic-sell-sword-brute-{uuid.uuid4().hex[:16]}"
    command = sandbox_command(snapshot.resolve(), cargo_registry.resolve(), name, worker_image, candidate_kind)
    try:
        completed = subprocess.run(command, capture_output=True, timeout=900, check=False)
    except subprocess.TimeoutExpired as error:
        subprocess.run(["docker", "rm", "-f", name], capture_output=True, check=False)
        raise WorkerFailure("candidate_timeout: worker exceeded its deadline") from error
    if len(completed.stdout) + len(completed.stderr) > MAX_OUTPUT_BYTES:
        subprocess.run(["docker", "rm", "-f", name], capture_output=True, check=False)
        raise WorkerFailure("candidate_output_limit: worker output exceeded 2 MiB")
    (output_root / "worker.stdout").write_bytes(completed.stdout)
    (output_root / "worker.stderr").write_bytes(completed.stderr)
    if completed.returncode != 0:
        result = {"schema_version": "cardbench.magic.card-pvp.check.v1", "task_id": "cardbench/magic/card", "candidate": {"kind": candidate_kind, "source_sha256": candidate_hash}, "passed": False, "pilot_card_passed": False, "cards_passed": 0, "cards_total": 291, "printings_total": 306, "cards_credited": 0, "coverage_complete": False, "full_reward": False, "harbor_reward": 0.0, "error": "Sell-Sword Brute reference trace or candidate compilation failed", "diagnostics": completed.stderr.decode(errors="replace")[-12000:]}
    else:
        result = parse_report(completed.stdout)
        result["candidate"] = {"kind": candidate_kind, "card_definition": "RAV-SELL-SWORD-BRUTE", "source_sha256": candidate_hash, "materialized_workspace_sha256": manifest_data["workspace_sha256"]}
        result["candidate_kind"] = candidate_kind
        result["starting_workspace_sha256"] = manifest_data["workspace_sha256"]
        result["sealed_authority"] = False
        result["worker_image"] = worker_image
        result["sandbox"] = {"network": "none", "rootfs": "read-only", "pids_limit": 128}
    (output_root / "result.json").write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
    (output_root / "reward.txt").write_text(f"{result.get('harbor_reward', 0.0):.8f}\n", encoding="utf-8")
    print(f"candidate_passed={result.get('pilot_card_passed', False)} cards={result.get('cards_passed', 0)}/291 coverage_complete=false")
    return 0 if result.get("passed") is True and result.get("pilot_card_passed") is True else 1


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
        (args.output_root / "error.json").write_text(json.dumps({"error": str(error)}, indent=2) + "\n", encoding="utf-8")
        print(f"candidate_failed={error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
