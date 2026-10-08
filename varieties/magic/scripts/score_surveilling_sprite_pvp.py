#!/usr/bin/env python3
"""Run Surveilling Sprite in an offline, resource-limited candidate worker."""

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
MATERIALIZER = Path(__file__).with_name("materialize_surveilling_sprite_task.py")
TARGET = Path("varieties/magic/sets/ravnica_city_of_guilds/src/cards/surveilling_sprite.rs")
AUTHORITY_SOURCE = Path("varieties/magic/session/src/bin/rav_surveilling_sprite_pvp.rs")
DEFAULT_WORKER_IMAGE = "rust@sha256:d7482085ff5b415f84dba5647ae71606650bdef00db7aeb69f4b3d170c3e4082"
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
    proc = subprocess.Popen(command, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    assert proc.stdout is not None and proc.stderr is not None
    selector = selectors.DefaultSelector()
    selector.register(proc.stdout, selectors.EVENT_READ, "stdout")
    selector.register(proc.stderr, selectors.EVENT_READ, "stderr")
    output, diagnostics = bytearray(), bytearray()
    started = time.monotonic()
    try:
        while selector.get_map() or proc.poll() is None:
            if time.monotonic() - started > timeout:
                raise WorkerFailure("candidate_timeout: Docker worker exceeded its deadline")
            for key, _ in selector.select(0.2):
                chunk = os.read(key.fileobj.fileno(), 65536)
                if not chunk:
                    selector.unregister(key.fileobj)
                    continue
                target = output if key.data == "stdout" else diagnostics
                target.extend(chunk)
                if len(output) + len(diagnostics) > MAX_OUTPUT_BYTES:
                    raise WorkerFailure("candidate_output_limit: worker output exceeded 2 MiB")
        return proc.wait(), bytes(output), bytes(diagnostics)
    except BaseException:
        proc.kill()
        proc.wait()
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
        "cp /trusted-authority.rs /work/varieties/magic/session/src/bin/rav_surveilling_sprite_pvp.rs && "
        "cd /work && cargo run --offline --locked -j 2 --manifest-path varieties/magic/Cargo.toml "
        "-p cardbench-magic-session --bin rav-surveilling-sprite-pvp --" + candidate_flag,
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
    raise WorkerFailure("candidate_protocol_error: no valid Surveilling Sprite PvP result")


def validate_report(report: dict) -> None:
    if (
        report.get("task_id") != "cardbench/magic/card"
        or report.get("passed") is not True
        or report.get("pilot_card_passed") is not True
        or report.get("deterministic_replay") is not True
        or report.get("cards_passed") != 1
        or report.get("cards_credited") != 0
        or report.get("cards_total") != 291
        or report.get("printings_total") != 306
        or report.get("coverage_complete") is not False
        or report.get("full_reward") is not False
        or report.get("harbor_reward") != 0.0
        or report.get("pilot_reward") != 0.0
        or report.get("viewer_scope") != "Judge"
        or report.get("card") != {"collector_number": 67, "definition_id": "RAV-SURVEILLING-SPRITE", "name": "Surveilling Sprite"}
    ):
        raise WorkerFailure("candidate_protocol_error: incomplete Surveilling Sprite single-card report")
    candidate = report.get("candidate")
    if not isinstance(candidate, dict) or candidate.get("card_definition") != "RAV-SURVEILLING-SPRITE":
        raise WorkerFailure("candidate_protocol_error: wrong Surveilling Sprite candidate identity")
    rules = report.get("rules_authority")
    if not isinstance(rules, dict) or rules.get("comprehensive_rules") != "2026-08-07" or rules.get("authority_cutoff") != "2026-09-23":
        raise WorkerFailure("candidate_protocol_error: Surveilling Sprite rules authority is inconsistent")
    traces = report.get("traces")
    if not isinstance(traces, list) or len(traces) != 2:
        raise WorkerFailure("candidate_protocol_error: accepted and declined Surveilling Sprite traces are required")
    expected_ids = {
        "rav-surveilling-sprite-dies-draw-accepted-pvp",
        "rav-surveilling-sprite-dies-draw-declined-pvp",
    }
    if {trace.get("trace_id") for trace in traces if isinstance(trace, dict)} != expected_ids:
        raise WorkerFailure("candidate_protocol_error: accepted and declined traces have wrong identities")
    for trace in traces:
        if (
            not isinstance(trace, dict)
            or trace.get("passed") is not True
            or trace.get("terminal") != "Winner(PlayerId(0))"
            or trace.get("winner_seat") != 0
            or trace.get("rejected_moves") != 0
            or not isinstance(trace.get("shuffle_seed"), int)
        ):
            raise WorkerFailure("candidate_protocol_error: Surveilling Sprite did not reach a clean terminal win")
        sprite = trace.get("surveilling_sprite")
        blocker = trace.get("ground_blocker")
        char = trace.get("opponent_char")
        trigger = trace.get("dies_trigger")
        combat = trace.get("combat")
        if not all(isinstance(value, dict) for value in (sprite, blocker, char, trigger, combat)):
            raise WorkerFailure("candidate_protocol_error: Sprite, ground blocker, Char, dies trigger, or combat receipt is missing")
        sprite_id, blocker_id = sprite.get("object"), blocker.get("object")
        accepted = trace.get("trace_id") == "rav-surveilling-sprite-dies-draw-accepted-pvp"
        if (
            not isinstance(sprite_id, int)
            or not isinstance(blocker_id, int)
            or sprite.get("collector_number") != 67
            or sprite.get("mana_cost") != "{1}{U}"
            or sprite.get("mana_paid") != ["Blue", "Blue"]
            or (sprite.get("power"), sprite.get("toughness")) != (1, 1)
            or sprite.get("flying") is not True
            or sprite.get("zone_after_death") != "Graveyard"
            or blocker.get("definition") != "RAV-LURKING-INFORMANT"
            or blocker.get("live_and_can_block") is not True
            or blocker.get("blocked_flying_sprite") is not False
            or char.get("mana_cost") != "{2}{R}"
            or not isinstance(char.get("object"), int)
            or len(char.get("mana_paid", [])) != 3
            or "Red" not in char.get("mana_paid", [])
            or char.get("dealt_four_lethal_damage") is not True
            or trigger.get("ability") != "dies-draw-controller"
            or trigger.get("source") != sprite_id
            or trigger.get("condition") != "Dies"
            or trigger.get("source_in_graveyard_before_choice") is not True
            or trigger.get("choice") != ("accepted" if accepted else "declined")
            or trigger.get("paid") is not accepted
            or (trigger.get("drawn_card") is not None) is not accepted
            or combat.get("flying_evasion_observed") is not True
            or combat.get("opponent_life_at_terminal", 1) > 0
        ):
            raise WorkerFailure("candidate_protocol_error: identity, exact costs, live ground blocker, optional draw, or terminal combat state differs")
        events = trace.get("events")
        expected_kinds = [
            "SpellManaPaid", "SpellCast", "SpellResolved", "CardMoved", "AttackersDeclared",
            "BlockersDeclared", "SpellManaPaid", "SpellCast", "DamageDealtToPermanent",
            "SpellResolved", "StateBasedAction", "CardMoved", "TriggeredAbilityStacked",
        ]
        if accepted:
            expected_kinds.append("CardMoved")
        expected_kinds.extend(["AbilityResolved", "PolicyMoveSubmitted", "GameEnded"])
        if not isinstance(events, list) or any(not isinstance(event, dict) for event in events) or [event.get("kind") for event in events] != expected_kinds:
            raise WorkerFailure("candidate_protocol_error: ordered cast, Flying combat, Char damage, dies trigger, optional draw, and terminal events are incomplete")
        sequence = [event.get("sequence") for event in events]
        if any(not isinstance(value, int) for value in sequence) or sequence != sorted(sequence):
            raise WorkerFailure("candidate_protocol_error: projected event sequence is malformed")
        facts = [event.get("facts") or {} for event in events]
        if not (
            facts[0].get("objects") == [sprite_id] and facts[0].get("seats") == [0]
            and facts[0].get("amount") == 2 and facts[0].get("mana_colors") == ["Blue", "Blue"]
            and facts[1].get("objects") == [sprite_id] and facts[1].get("seats") == [0]
            and facts[2].get("objects") == [sprite_id]
            and facts[3].get("objects") == [sprite_id] and facts[3].get("zone") == "Battlefield"
            and sprite_id in facts[4].get("objects", []) and facts[4].get("seats") == [0]
            and facts[5].get("seats") == [1]
            and facts[6].get("objects") == [char.get("object")] and facts[6].get("seats") == [1]
            and "Red" in facts[6].get("mana_colors", []) and facts[6].get("amount") == 3
            and facts[7].get("objects") == [char.get("object")] and facts[7].get("seats") == [1]
            and facts[8].get("objects") == [char.get("object"), sprite_id] and facts[8].get("amount") == 4
            and facts[9].get("objects") == [char.get("object")]
            and facts[10].get("objects") == [sprite_id] and facts[10].get("reason") == "creature has lethal damage"
            and facts[11].get("objects") == [sprite_id] and facts[11].get("zone") == "Graveyard"
            and facts[12].get("objects") == [sprite_id] and facts[12].get("ability") == "dies-draw-controller"
            and (not accepted or (facts[13].get("objects") == [trigger.get("drawn_card")] and facts[13].get("zone") == "Hand"))
            and facts[-1].get("seats") == [0]
        ):
            raise WorkerFailure("candidate_protocol_error: event facts do not bind card identities, colored costs, combat state, and draw result")
    mutations = report.get("mutation_sensitivity")
    required = (
        "flying_removal_changes_live_blocking_outcome",
        "cost_mutation_detected_in_game",
        "power_mutation_detected_in_game",
        "dies_trigger_removal_detected_in_game",
        "draw_effect_removal_detected_in_game",
        "mandatory_trigger_mutation_detected_in_game",
    )
    if not isinstance(mutations, dict) or any(mutations.get(key) is not True for key in required):
        raise WorkerFailure("candidate_protocol_error: Flying, cost, power, dies-trigger, draw, and optionality mutations are required")

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
        raise FileNotFoundError(f"trusted Surveilling Sprite authority is missing: {authority}")
    if not shutil.which("docker"):
        raise FileNotFoundError("Docker is required for Magic candidate isolation")
    verify_workspace(workspace, manifest)
    scoring_workspace = make_scoring_snapshot(workspace, manifest_data["files"], output_root / "candidate-workspace")
    candidate_hash = sha256((scoring_workspace / TARGET).read_bytes())
    name = f"cardbench-magic-surveilling-sprite-{uuid.uuid4().hex[:16]}"
    command = sandbox_command(scoring_workspace, cargo_registry.resolve(), authority, name, worker_image, candidate_kind)
    code, stdout, stderr = run_bounded(command, timeout=900, name=name)
    (output_root / "worker.stdout").write_bytes(stdout)
    (output_root / "worker.stderr").write_bytes(stderr)
    if code != 0:
        result = {
            "schema_version": "cardbench.magic.card-pvp.check.v1",
            "task_id": "cardbench/magic/card",
            "candidate": {"kind": candidate_kind, "source_sha256": candidate_hash},
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
            "worker_image": worker_image,
            "sandbox": {"network": "none", "rootfs": "read-only", "pids_limit": 128},
            "error": "candidate compile or Surveilling Sprite reference trace failed",
            "diagnostics": stderr.decode(errors="replace")[-12000:],
        }
    else:
        result = parse_report(stdout)
        result["candidate"] = {
            "kind": candidate_kind,
            "card_definition": "RAV-SURVEILLING-SPRITE",
            "source_sha256": candidate_hash,
            "materialized_workspace_sha256": manifest_data["workspace_sha256"],
        }
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
        result = {
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
            "sealed_authority": False,
            "error": str(error),
        }
        (args.output_root / "result.json").write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
        (args.output_root / "reward.txt").write_text("0.0\n", encoding="utf-8")
        print(f"Magic candidate verifier failed: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
