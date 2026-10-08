#!/usr/bin/env python3
"""Validate Suppression Field in an offline isolated candidate worker."""

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
MATERIALIZER = Path(__file__).with_name("materialize_suppression_field_task.py")
TARGET = Path("varieties/magic/sets/ravnica_city_of_guilds/src/cards/suppression_field.rs")
AUTHORITY_SOURCE = Path("varieties/magic/session/src/bin/rav_suppression_field_pvp.rs")
DEFAULT_WORKER_IMAGE = "rust@sha256:d7482085ff5b415f84dba5647ae71606650bdef00db7aeb69f4b3d1703c4082"
MAX_OUTPUT_BYTES = 2 * 1024 * 1024


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
    process = subprocess.Popen(command, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    assert process.stdout is not None and process.stderr is not None
    selector = selectors.DefaultSelector()
    selector.register(process.stdout, selectors.EVENT_READ, "stdout")
    selector.register(process.stderr, selectors.EVENT_READ, "stderr")
    output, diagnostics = bytearray(), bytearray()
    started = time.monotonic()
    try:
        while selector.get_map() or process.poll() is None:
            if time.monotonic() - started > timeout:
                raise WorkerFailure("candidate_timeout: isolated Docker worker exceeded its deadline")
            for key, _ in selector.select(0.2):
                chunk = os.read(key.fileobj.fileno(), 65536)
                if not chunk:
                    selector.unregister(key.fileobj)
                    continue
                target = output if key.data == "stdout" else diagnostics
                target.extend(chunk)
                if len(output) + len(diagnostics) > MAX_OUTPUT_BYTES:
                    raise WorkerFailure("candidate_output_limit: worker output exceeded 2 MiB")
        return process.wait(), bytes(output), bytes(diagnostics)
    except BaseException:
        process.kill()
        process.wait()
        subprocess.run(["docker", "rm", "-f", name], capture_output=True, check=False)
        raise


def sandbox_command(workspace: Path, registry: Path, authority: Path, name: str, image: str, candidate_kind: str) -> list[str]:
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
        "cp /trusted-authority.rs /work/varieties/magic/session/src/bin/rav_suppression_field_pvp.rs && "
        "cd /work && cargo run --offline --locked -j 2 --manifest-path varieties/magic/Cargo.toml "
        "-p cardbench-magic-session --bin rav-suppression-field-pvp --" + candidate_flag,
    ]


def parse_report(stdout: bytes) -> dict:
    for line in reversed(stdout.decode(errors="replace").splitlines()):
        try:
            value = json.loads(line)
        except json.JSONDecodeError:
            continue
        if value.get("schema_version") == "cardbench.magic.card-pvp.check.v1":
            validate_report(value)
            return value
    raise WorkerFailure("candidate_protocol_error: no valid Suppression Field PvP result")


def validate_report(report: dict) -> None:
    if (
        report.get("task_id") != "cardbench/magic/card"
        or report.get("passed") is not True
        or report.get("pilot_card_passed") is not True
        or report.get("cards_passed") != 1
        or report.get("cards_credited") != 0
        or report.get("cards_total") != 291
        or report.get("printings_total") != 306
        or report.get("coverage_complete") is not False
        or report.get("full_reward") is not False
        or report.get("harbor_reward") != 0.0
        or report.get("viewer_scope") != "Judge"
        or report.get("card") != {"collector_number": 31, "definition_id": "RAV-SUPPRESSION-FIELD", "name": "Suppression Field"}
    ):
        raise WorkerFailure("candidate_protocol_error: incomplete Suppression Field report")
    candidate = report.get("candidate")
    if not isinstance(candidate, dict) or candidate.get("card_definition") != "RAV-SUPPRESSION-FIELD":
        raise WorkerFailure("candidate_protocol_error: wrong Suppression Field candidate identity")
    traces = report.get("traces")
    if not isinstance(traces, list) or len(traces) != 1 or traces[0].get("id") != "rav-suppression-field-global-ability-tax-pvp":
        raise WorkerFailure("candidate_protocol_error: one full-game Suppression Field trace is required")
    trace = traces[0]
    mechanics = trace.get("mechanics")
    events = trace.get("events")
    sequence = mechanics.get("ordered_event_sequence") if isinstance(mechanics, dict) else None
    paid = mechanics.get("field_mana_paid") if isinstance(mechanics, dict) else None
    ability_paid = mechanics.get("ability_mana_paid") if isinstance(mechanics, dict) else None
    if (
        trace.get("terminal") != "Winner(PlayerId(0))"
        or trace.get("winner_seat") != 0
        or trace.get("rejected_moves") != 0
        or not isinstance(trace.get("seed"), int)
        or mechanics.get("field_mana_cost") != "{1}{W}"
        or not isinstance(mechanics.get("field_object"), int)
        or mechanics.get("field_controller_seat") != 0
        or not isinstance(paid, list) or len(paid) != 2 or paid.count("White") != 1
        or mechanics.get("affected_ability_source_object") is None
        or mechanics.get("affected_ability_definition") != "RAV-DIMIR-GUILDMAGE"
        or mechanics.get("affected_ability") != "target-player-draw"
        or mechanics.get("affected_ability_controller_seat") != 1
        or mechanics.get("base_ability_cost") != "{3}{U}"
        or mechanics.get("effective_ability_cost") != "{5}{U}"
        or mechanics.get("generic_increase") != 2
        or mechanics.get("cost_modifier_source_object") != mechanics.get("field_object")
        or ability_paid != {"generic": 5, "colors": ["Blue"]}
        or mechanics.get("ability_stacked") is not True
        or mechanics.get("ability_resolved") is not True
        or mechanics.get("draw_target_player") != 0
        or mechanics.get("draw_count") != 1
        or not isinstance(mechanics.get("drawn_card_object"), int)
        or mechanics.get("target_library_before") != mechanics.get("target_library_after", -1) + 1
        or mechanics.get("mana_ability_activated_after_field") is not True
        or mechanics.get("mana_ability_source_definition") != "RAV-ISLAND"
        or not isinstance(mechanics.get("mana_ability_source_object"), int)
        or not isinstance(sequence, list) or len(sequence) != 16
        or any(not isinstance(item, int) for item in sequence)
        or sequence != sorted(sequence)
        or not isinstance(events, list) or len(events) != 16
    ):
        raise WorkerFailure("candidate_protocol_error: cast, live cost increase, payment, draw, mana ability, or terminal PvP state is wrong")
    if [event.get("sequence") for event in events] != sequence:
        raise WorkerFailure("candidate_protocol_error: ordered event sequence differs from projected receipts")
    kinds = [event.get("kind") for event in events if isinstance(event, dict)]
    if len(kinds) != 16 or kinds[:8].count("SpellManaPaid") != 2 or kinds[:8].count("SpellCast") != 2 or kinds[:8].count("SpellResolved") != 2 or kinds[:8].count("CardMoved") != 2:
        raise WorkerFailure("candidate_protocol_error: both card cast/entry receipts are required")
    field = mechanics["field_object"]
    guildmage = mechanics["affected_ability_source_object"]
    if kinds[8:] != ["ActivatedAbilityCostCalculated", "AbilityManaPaid", "AbilityActivated", "CardMoved", "AbilityResolved", "ManaAbilityActivated", "AttackersDeclared", "GameEnded"]:
        raise WorkerFailure("candidate_protocol_error: ordered taxed activation, draw, mana, attack, and terminal events differ")
    field_group = events[:4] if events[0].get("facts", {}).get("objects") == [field] else events[4:8]
    guildmage_group = events[:4] if events[0].get("facts", {}).get("objects") == [guildmage] else events[4:8]
    for group, source, seat, expected_colors in ((field_group, field, 0, paid), (guildmage_group, guildmage, 1, None)):
        if [event.get("kind") for event in group] != ["SpellManaPaid", "SpellCast", "SpellResolved", "CardMoved"]:
            raise WorkerFailure("candidate_protocol_error: card cast/payment/resolve/entry sequence differs")
        facts = [event.get("facts", {}) for event in group]
        if facts[0].get("objects") != [source] or facts[0].get("seats") != [seat] or facts[1].get("objects") != [source] or facts[1].get("seats") != [seat] or facts[2].get("objects") != [source] or facts[3].get("objects") != [source] or facts[3].get("zone") != "Battlefield":
            raise WorkerFailure("candidate_protocol_error: cast source/player/battlefield facts differ")
        if expected_colors is not None and facts[0].get("mana_colors") != expected_colors:
            raise WorkerFailure("candidate_protocol_error: Suppression Field payment colors differ")
    cost_event = events[8]
    canonical = cost_event.get("canonical", "")
    required_fragments = (
        "acting_player: PlayerId(1)",
        f"source: ObjectId({guildmage})",
        'ability_id: "target-player-draw"',
        "kind: NonMana",
        "base_mana_cost: ManaCost { generic: 3, colored: [Blue] }",
        f"source: ObjectId({field})",
        "generic_amount: 2",
        "effective_mana_cost: ManaCost { generic: 5, colored: [Blue] }",
    )
    if not all(fragment in canonical for fragment in required_fragments):
        raise WorkerFailure("candidate_protocol_error: auditable activated-cost source/calculation receipt differs")
    ability_facts = [events[index].get("facts", {}) for index in range(9, 15)]
    if (
        ability_facts[0].get("objects") != [guildmage]
        or ability_facts[0].get("seats") != [1]
        or ability_facts[0].get("ability") != "target-player-draw"
        or ability_facts[0].get("mana_cost_generic") != 5
        or ability_facts[0].get("mana_cost_colors") != ["Blue"]
        or ability_facts[1].get("objects") != [guildmage]
        or ability_facts[1].get("seats") != [1]
        or ability_facts[1].get("ability") != "target-player-draw"
        or ability_facts[2].get("objects") != [mechanics["drawn_card_object"]]
        or ability_facts[2].get("zone") != "Hand"
        or ability_facts[3].get("objects") != [guildmage]
        or "player: PlayerId(1)" not in events[13].get("canonical", "")
        or f"land: ObjectId({mechanics['mana_ability_source_object']})" not in events[13].get("canonical", "")
        or "color: Blue" not in events[13].get("canonical", "")
        or events[14].get("facts", {}).get("seats") != [0]
        or events[15].get("facts", {}).get("seats") != [0]
    ):
        raise WorkerFailure("candidate_protocol_error: ability payment/stack/draw/mana/attack/winner facts differ")
    mutations = report.get("mutation_sensitivity")
    required = ("missing_live_source_detected_in_game", "incorrect_tax_amount_detected_in_game")
    if not isinstance(mutations, dict) or any(mutations.get(key) is not True for key in required):
        raise WorkerFailure("candidate_protocol_error: Suppression Field gameplay mutation controls are incomplete")
    rules = report.get("rules_authority")
    if not isinstance(rules, dict) or rules.get("comprehensive_rules") != "2026-08-07":
        raise WorkerFailure("candidate_protocol_error: pinned Comprehensive Rules snapshot is missing")


def score(workspace: Path, manifest: Path, output_root: Path, candidate_kind: str) -> int:
    output_root.mkdir(parents=True, exist_ok=True)
    manifest_data = json.loads(manifest.read_text(encoding="utf-8"))
    worker_image = os.environ.get("CARDBENCH_MAGIC_WORKER_IMAGE", DEFAULT_WORKER_IMAGE)
    if not (worker_image.startswith("rust@sha256:") or worker_image.startswith("sha256:")):
        raise ValueError("CARDBENCH_MAGIC_WORKER_IMAGE must be an immutable digest reference")
    cargo_registry = Path.home() / ".cargo" / "registry"
    if not cargo_registry.is_dir():
        raise FileNotFoundError(f"offline Cargo registry cache is missing: {cargo_registry}")
    authority = REPO / AUTHORITY_SOURCE
    if authority.is_symlink() or not authority.is_file():
        raise FileNotFoundError(f"trusted Suppression Field authority is missing: {authority}")
    if not shutil.which("docker"):
        raise FileNotFoundError("Docker is required for isolated Magic candidate scoring")
    verify_workspace(workspace, manifest)
    scoring_workspace = make_scoring_snapshot(workspace, manifest_data["files"], output_root / "candidate-workspace")
    candidate_hash = sha256((scoring_workspace / TARGET).read_bytes())
    name = f"cardbench-magic-suppression-field-{uuid.uuid4().hex[:16]}"
    command = sandbox_command(scoring_workspace, cargo_registry.resolve(), authority, name, worker_image, candidate_kind)
    code, stdout, stderr = run_bounded(command, timeout=900, name=name)
    (output_root / "worker.stdout").write_bytes(stdout)
    (output_root / "worker.stderr").write_bytes(stderr)
    if code != 0:
        result = {
            "schema_version": "cardbench.magic.card-pvp.check.v1", "task_id": "cardbench/magic/card",
            "candidate": {"kind": candidate_kind, "source_sha256": candidate_hash},
            "passed": False, "pilot_card_passed": False, "cards_passed": 0, "cards_total": 291,
            "printings_total": 306, "cards_credited": 0, "coverage_complete": False,
            "full_reward": False, "harbor_reward": 0.0, "sealed_authority": False,
            "worker_image": worker_image, "sandbox": {"network": "none", "rootfs": "read-only", "pids_limit": 128},
            "error": "candidate compilation or Suppression Field full-game trace failed",
            "diagnostics": stderr.decode(errors="replace")[-12000:],
        }
    else:
        result = parse_report(stdout)
        result["candidate"] = {"kind": candidate_kind, "card_definition": "RAV-SUPPRESSION-FIELD", "source_sha256": candidate_hash, "materialized_workspace_sha256": manifest_data["workspace_sha256"]}
        result["candidate_kind"] = candidate_kind
        result["starting_workspace_sha256"] = manifest_data["workspace_sha256"]
        result["sealed_authority"] = False
        result["worker_image"] = worker_image
        result["sandbox"] = {"network": "none", "rootfs": "read-only", "pids_limit": 128}
        result["harbor_reward"] = 0.0
        result["full_reward"] = False
    (output_root / "result.json").write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
    (output_root / "reward.txt").write_text(f"{result['harbor_reward']:.8f}\n", encoding="utf-8")
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
        result = {"schema_version": "cardbench.magic.card-pvp.check.v1", "task_id": "cardbench/magic/card", "passed": False, "pilot_card_passed": False, "cards_passed": 0, "cards_total": 291, "printings_total": 306, "cards_credited": 0, "coverage_complete": False, "full_reward": False, "harbor_reward": 0.0, "sealed_authority": False, "error": str(error)}
        (args.output_root / "result.json").write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
        (args.output_root / "reward.txt").write_text("0.0\n", encoding="utf-8")
        print(f"Magic candidate verifier failed: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
