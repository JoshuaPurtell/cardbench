#!/usr/bin/env python3
"""Run Drake Familiar in a bounded offline worker and validate the PvP trace."""

from __future__ import annotations

import argparse
import hashlib
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
MATERIALIZER = Path(__file__).with_name("materialize_drake_familiar_task.py")
TARGET = Path("varieties/magic/sets/ravnica_city_of_guilds/src/cards/drake_familiar.rs")
AUTHORITY = Path("varieties/magic/session/src/bin/rav_drake_familiar_pvp.rs")
IMAGE = "rust@sha256:d7482085ff5b415f84dba5647ae71606650bdef00db7aeb69f4b3d170c3e4082"
MAX_OUTPUT = 2 * 1024 * 1024


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


def snapshot_workspace(workspace: Path, files: dict[str, str], snapshot: Path) -> Path:
    if snapshot.is_symlink():
        raise WorkerFailure("candidate_snapshot_path_is_symlink")
    if snapshot.exists():
        shutil.rmtree(snapshot)
    snapshot.mkdir(parents=True)
    for relative in sorted(files):
        source = workspace / relative
        if source.is_symlink() or not source.is_file():
            raise WorkerFailure(f"candidate_snapshot_source_missing: {relative}")
        destination = snapshot / relative
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(source, destination)
    return snapshot


def run_bounded(command: list[str], timeout: int, container: str) -> tuple[int, bytes, bytes]:
    process = subprocess.Popen(command, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    assert process.stdout is not None and process.stderr is not None
    selector = selectors.DefaultSelector()
    selector.register(process.stdout, selectors.EVENT_READ, "stdout")
    selector.register(process.stderr, selectors.EVENT_READ, "stderr")
    stdout, stderr = bytearray(), bytearray()
    started = time.monotonic()
    try:
        while selector.get_map() or process.poll() is None:
            if time.monotonic() - started > timeout:
                raise WorkerFailure("candidate_timeout: Docker worker exceeded its deadline")
            for key, _ in selector.select(0.2):
                chunk = os.read(key.fileobj.fileno(), 65536)
                if not chunk:
                    selector.unregister(key.fileobj)
                    continue
                (stdout if key.data == "stdout" else stderr).extend(chunk)
                if len(stdout) + len(stderr) > MAX_OUTPUT:
                    raise WorkerFailure("candidate_output_limit: worker output exceeded 2 MiB")
        return process.wait(), bytes(stdout), bytes(stderr)
    except BaseException:
        process.kill()
        process.wait()
        subprocess.run(["docker", "rm", "-f", container], capture_output=True, check=False)
        raise


def sandbox_command(workspace: Path, registry: Path, authority: Path, container: str, image: str, candidate_kind: str) -> list[str]:
    if authority.is_symlink() or not authority.is_file():
        raise WorkerFailure("trusted_authority_source_missing")
    candidate = " --candidate" if candidate_kind == "agent" else ""
    return [
        "docker", "run", "--rm", "--name", container, "--network", "none", "--read-only",
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
        "cp /trusted-authority.rs /work/varieties/magic/session/src/bin/rav_drake_familiar_pvp.rs && "
        "cd /work && cargo run --offline --locked -j 2 --manifest-path varieties/magic/Cargo.toml "
        "-p cardbench-magic-session --bin rav-drake-familiar-pvp --" + candidate,
    ]


def validate_trace(trace: object) -> None:
    if not isinstance(trace, dict) or trace.get("id") != "rav-drake-familiar-return-or-sacrifice-etb-pvp":
        raise WorkerFailure("candidate_protocol_error: Drake Familiar trace identity is missing")
    if trace.get("passed") is not True or trace.get("terminal") != "Winner(PlayerId(0))" or trace.get("winner_seat") != 0 or trace.get("rejected_moves") != 0:
        raise WorkerFailure("candidate_protocol_error: Drake Familiar did not reach a clean terminal seat-zero win")
    drake = trace.get("drake")
    etb = trace.get("etb")
    drake_id = trace.get("drake_object")
    enchantment_id = trace.get("enchantment_object")
    if not isinstance(drake, dict) or not isinstance(etb, dict):
        raise WorkerFailure("candidate_protocol_error: Drake Familiar gameplay facts are missing")
    paid = drake.get("paid_colors")
    if (
        not isinstance(drake_id, int) or not isinstance(enchantment_id, int)
        or drake.get("mana_cost") != "{1}{U}"
        or not isinstance(paid, list) or len(paid) != 2 or "Blue" not in paid
        or drake.get("power") != 2 or drake.get("toughness") != 1 or drake.get("flying") is not True
        or drake.get("attacked") is not True or not isinstance(drake.get("combat_damage"), int)
        or drake.get("combat_damage", 0) < 20
        or etb.get("ability") != "etb-return-any-enchantment-or-sacrifice-source"
        or not isinstance(etb.get("source_incarnation"), int)
        or etb.get("resolved_incarnation") != etb.get("source_incarnation")
        or etb.get("target_count") != 0 or etb.get("choice_opened") is not True
        or (etb.get("choice_min"), etb.get("choice_max")) != (0, 1)
        or etb.get("selected_object") != enchantment_id
        or etb.get("returned_to_owner_hand") is not True
        or etb.get("enchantment_owner_seat") != 1
        or etb.get("enchantment_controller_seat") != 1
        or etb.get("choice_actor_seat") != 0
        or not isinstance(etb.get("choice_candidate_controller_seats"), list)
        or 1 not in etb.get("choice_candidate_controller_seats", [])
        or etb.get("response_priority_passed") is not True
    ):
        raise WorkerFailure("candidate_protocol_error: Drake Familiar body, cost, target-free choice, return, or terminal combat facts differ")

    events = trace.get("events")
    if not isinstance(events, list) or len(events) < 20 or any(not isinstance(event, dict) for event in events):
        raise WorkerFailure("candidate_protocol_error: Drake Familiar projected event trace is incomplete")
    sequence = [event.get("sequence") for event in events]
    if any(not isinstance(value, int) for value in sequence) or sequence != sorted(sequence) or len(set(sequence)) != len(sequence):
        raise WorkerFailure("candidate_protocol_error: Drake Familiar event ordering is malformed")

    def first(kind: str, predicate) -> dict:
        event = next((event for event in events if event.get("kind") == kind and predicate(event)), None)
        if event is None:
            raise WorkerFailure(f"candidate_protocol_error: missing Drake Familiar {kind} receipt")
        return event

    paid_event = first("SpellManaPaid", lambda event: event.get("facts", {}).get("objects") == [drake_id] and event.get("facts", {}).get("seats") == [0])
    cast = first("SpellCast", lambda event: event.get("facts", {}).get("objects") == [drake_id])
    resolved_spell = first("SpellResolved", lambda event: event.get("facts", {}).get("objects") == [drake_id])
    entered = first("CardMoved", lambda event: event.get("facts", {}).get("objects") == [drake_id] and event.get("facts", {}).get("zone") == "Battlefield")
    ability_id = etb["ability"]
    stacked = first("TriggeredAbilityStacked", lambda event: event.get("facts", {}).get("objects") == [drake_id] and event.get("facts", {}).get("ability") == ability_id)
    response = first("PriorityPassed", lambda event: event.get("facts", {}).get("seats") == [1] and event["sequence"] > stacked["sequence"])
    opened = first("DecisionOpened", lambda event: "player: PlayerId(0)" in event.get("canonical", "") and "TriggeredEffectObject" in event.get("canonical", ""))
    completed = first("DecisionCompleted", lambda event: "player: PlayerId(0)" in event.get("canonical", "") and "TriggeredEffectObject" in event.get("canonical", ""))
    returned = first("CardMoved", lambda event: event.get("facts", {}).get("objects") == [enchantment_id] and event.get("facts", {}).get("zone") == "Hand")
    ability_resolved = first("AbilityResolved", lambda event: event.get("facts", {}).get("objects") == [drake_id] and event.get("facts", {}).get("ability") == ability_id)
    attack = first("AttackersDeclared", lambda event: event.get("facts", {}).get("seats") == [0] and drake_id in event.get("facts", {}).get("objects", []))
    damage = first("DamageDealtToPlayer", lambda event: event.get("facts", {}).get("seats") == [1] and drake_id in event.get("facts", {}).get("objects", []) and event.get("facts", {}).get("amount", 0) > 0)
    ended = first("GameEnded", lambda event: event.get("facts", {}).get("seats") == [0])
    if not (paid_event["sequence"] < cast["sequence"] < resolved_spell["sequence"] < entered["sequence"] < stacked["sequence"] < response["sequence"] < opened["sequence"] < completed["sequence"] < returned["sequence"] < ability_resolved["sequence"] < attack["sequence"] < damage["sequence"] < ended["sequence"]):
        raise WorkerFailure("candidate_protocol_error: Drake Familiar cast, priority, resolution choice, return, and combat receipts are out of order")


def parse_report(stdout: bytes) -> dict:
    for line in reversed(stdout.decode(errors="replace").splitlines()):
        try:
            report = json.loads(line)
        except json.JSONDecodeError:
            continue
        if report.get("schema_version") == "cardbench.magic.card-pvp.check.v1":
            validate_report(report)
            return report
    raise WorkerFailure("candidate_protocol_error: no valid Drake Familiar PvP result")


def validate_report(report: dict) -> None:
    card_id = "RAV-DRAKE-FAMILIAR"
    expected_card = {"collector_number": 44, "definition_id": card_id, "name": "Drake Familiar"}
    if (
        report.get("task_id") != "cardbench/magic/card" or report.get("passed") is not True
        or report.get("pilot_card_passed") is not True or report.get("cards_passed") != 1
        or report.get("cards_credited") != 0 or report.get("cards_total") != 291
        or report.get("printings_total") != 306 or report.get("coverage_complete") is not False
        or report.get("full_reward") is not False or report.get("harbor_reward") != 0.0
        or report.get("pilot_reward") != 0.0 or report.get("viewer_scope") != "Judge"
        or report.get("card") != expected_card
    ):
        raise WorkerFailure("candidate_protocol_error: incomplete Drake Familiar diagnostic report")
    candidate = report.get("candidate")
    if not isinstance(candidate, dict) or candidate.get("card_definition") != card_id:
        raise WorkerFailure("candidate_protocol_error: wrong Drake Familiar candidate identity")
    rules = report.get("rules_authority")
    source_references = rules.get("source_references", []) if isinstance(rules, dict) else []
    source_contract = rules.get("source_contract", "") if isinstance(rules, dict) else ""
    if (
        not isinstance(rules, dict)
        or rules.get("comprehensive_rules") != "2026-08-07"
        or "oracle_reconciliation" not in rules
        or not isinstance(source_references, list)
        or not all(isinstance(source, str) for source in source_references)
        or not isinstance(source_contract, str)
        or not any("rav-official-faq" in source for source in source_references)
        or not any("mtg.wtf/card/rav/44" in source for source in source_references)
        or not all(term in source_contract for term in ("any battlefield enchantment", "opponent's", "untargetable", "owner's hand"))
    ):
        raise WorkerFailure("candidate_protocol_error: Drake Familiar printed-text/Oracle authority note is missing")
    traces = report.get("traces")
    if not isinstance(traces, list) or len(traces) != 1:
        raise WorkerFailure("candidate_protocol_error: exactly one full-game Drake Familiar trace is required")
    validate_trace(traces[0])
    mutations = report.get("mutation_sensitivity")
    required = (
        "mana_cost_changed_detected_in_game", "body_changed_detected_in_game", "flying_removed_detected_in_game",
        "trigger_removed_detected_in_game", "trigger_condition_changed_detected_in_game",
        "targeted_instead_of_target_free_detected_in_game", "return_effect_changed_detected_in_game",
        "syntactically_parsed_candidate_side_effect_rejected_before_execution",
    )
    if not isinstance(mutations, dict) or any(mutations.get(key) is not True for key in required) or mutations.get("sealed_mutation_authority") is not False:
        raise WorkerFailure("candidate_protocol_error: Drake Familiar mutation controls are incomplete")


def score(workspace: Path, manifest: Path, output_root: Path, candidate_kind: str) -> int:
    output_root.mkdir(parents=True, exist_ok=True)
    manifest_data = json.loads(manifest.read_text(encoding="utf-8"))
    image = os.environ.get("CARDBENCH_MAGIC_WORKER_IMAGE", IMAGE)
    if not (image.startswith("rust@sha256:") or image.startswith("sha256:")):
        raise ValueError("CARDBENCH_MAGIC_WORKER_IMAGE must be an immutable digest reference")
    registry = Path.home() / ".cargo" / "registry"
    if not registry.is_dir():
        raise FileNotFoundError(f"offline Cargo registry cache is missing: {registry}")
    authority = REPO / AUTHORITY
    if authority.is_symlink() or not authority.is_file():
        raise FileNotFoundError(f"trusted Drake Familiar authority is missing: {authority}")
    if not shutil.which("docker"):
        raise FileNotFoundError("Docker is required for Magic candidate isolation")
    verify_workspace(workspace, manifest)
    snapshot = snapshot_workspace(workspace, manifest_data["files"], output_root / "candidate-workspace")
    digest = sha256((snapshot / TARGET).read_bytes())
    container = f"cardbench-magic-drake-familiar-{uuid.uuid4().hex[:16]}"
    code, stdout, stderr = run_bounded(
        sandbox_command(snapshot, registry.resolve(), authority, container, image, candidate_kind),
        900,
        container,
    )
    (output_root / "worker.stdout").write_bytes(stdout)
    (output_root / "worker.stderr").write_bytes(stderr)
    if code != 0:
        report = {
            "schema_version": "cardbench.magic.card-pvp.check.v1",
            "task_id": "cardbench/magic/card",
            "candidate": {"kind": candidate_kind, "source_sha256": digest},
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
            "sealed_authority": False,
            "worker_image": image,
            "sandbox": {"network": "none", "rootfs": "read-only", "pids_limit": 128},
            "error": "candidate compile or Drake Familiar reference trace failed",
            "diagnostics": stderr.decode(errors="replace")[-12000:],
        }
        (output_root / "result.json").write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
        print(json.dumps(report))
        return 1
    report = parse_report(stdout)
    report["candidate"].update({"kind": candidate_kind, "source_sha256": digest})
    report.update({"sealed_authority": False, "worker_image": image, "sandbox": {"network": "none", "rootfs": "read-only", "pids_limit": 128}})
    (output_root / "result.json").write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    print(json.dumps(report))
    return 0


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--workspace", required=True, type=Path)
    parser.add_argument("--manifest", required=True, type=Path)
    parser.add_argument("--output-root", required=True, type=Path)
    parser.add_argument("--candidate-kind", choices=("agent", "reference"), default="agent")
    args = parser.parse_args()
    return score(args.workspace.resolve(), args.manifest.resolve(), args.output_root.resolve(), args.candidate_kind)


if __name__ == "__main__":
    raise SystemExit(main())
