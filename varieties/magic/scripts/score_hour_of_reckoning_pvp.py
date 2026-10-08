#!/usr/bin/env python3
"""Run the Hour of Reckoning diagnostic in an offline isolated worker."""

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
MATERIALIZER = Path(__file__).with_name("materialize_hour_of_reckoning_task.py")
TARGET = Path("varieties/magic/sets/ravnica_city_of_guilds/src/cards/hour_of_reckoning.rs")
AUTHORITY = Path("varieties/magic/session/src/bin/rav_hour_of_reckoning_pvp.rs")
BIN = "rav-hour-of-reckoning-pvp"
DEFAULT_WORKER_IMAGE = "rust@sha256:d7482085ff5b415f84dba5647ae71606650bdef00db7aeb69f4b3d170c3e4082"
MAX_OUTPUT_BYTES = 2 * 1024 * 1024


class WorkerFailure(RuntimeError):
    pass


def digest(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def verify_workspace(workspace: Path, manifest: Path) -> dict:
    subprocess.run(
        [sys.executable, str(MATERIALIZER), "--workspace", str(workspace), "--manifest", str(manifest), "--verify"],
        check=True,
        cwd=REPO,
    )
    return json.loads(manifest.read_text(encoding="utf-8"))


def snapshot(workspace: Path, files: dict[str, str], destination: Path) -> Path:
    if destination.is_symlink():
        raise WorkerFailure("candidate_snapshot_path_is_symlink")
    if destination.exists():
        shutil.rmtree(destination)
    destination.mkdir(parents=True)
    for relative in sorted(files):
        source = workspace / relative
        if source.is_symlink() or not source.is_file():
            raise WorkerFailure(f"candidate_snapshot_source_missing: {relative}")
        target = destination / relative
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(source, target)
    return destination


def bounded(command: list[str], *, timeout: int, name: str) -> tuple[int, bytes, bytes]:
    proc = subprocess.Popen(command, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    assert proc.stdout is not None and proc.stderr is not None
    selector = selectors.DefaultSelector()
    selector.register(proc.stdout, selectors.EVENT_READ, "stdout")
    selector.register(proc.stderr, selectors.EVENT_READ, "stderr")
    output, errors, started = bytearray(), bytearray(), time.monotonic()
    try:
        while selector.get_map() or proc.poll() is None:
            if time.monotonic() - started > timeout:
                raise WorkerFailure("candidate_timeout")
            for key, _ in selector.select(0.2):
                chunk = os.read(key.fileobj.fileno(), 65536)
                if not chunk:
                    selector.unregister(key.fileobj)
                    continue
                (output if key.data == "stdout" else errors).extend(chunk)
                if len(output) + len(errors) > MAX_OUTPUT_BYTES:
                    raise WorkerFailure("candidate_output_limit")
        return proc.wait(), bytes(output), bytes(errors)
    except BaseException:
        proc.kill()
        proc.wait()
        subprocess.run(["docker", "rm", "-f", name], capture_output=True, check=False)
        raise


def validate_report(report: dict) -> None:
    expected_card = {
        "collector_number": 21,
        "printings": 1,
        "definition_id": "RAV-HOUR-OF-RECKONING",
        "name": "Hour of Reckoning",
    }
    if (
        report.get("schema_version") != "cardbench.magic.card-pvp.check.v1"
        or report.get("task_id") != "cardbench/magic/card"
        or report.get("passed") is not True
        or report.get("pilot_card_passed") is not True
        or report.get("cards_passed") != 1
        or report.get("cards_total") != 291
        or report.get("printings_total") != 306
        or report.get("cards_credited") != 0
        or report.get("coverage_complete") is not False
        or report.get("full_reward") is not False
        or report.get("harbor_reward") != 0.0
        or report.get("pilot_reward") != 0.0
        or report.get("viewer_scope") != "Judge"
        or report.get("card") != expected_card
    ):
        raise WorkerFailure("candidate_protocol_error: incomplete Hour of Reckoning report or nonzero reward")

    traces = report.get("traces")
    if not isinstance(traces, list) or len(traces) != 1:
        raise WorkerFailure("candidate_protocol_error: exactly one full-game trace is required")
    trace = traces[0]
    card = trace.get("hour_of_reckoning", {})
    if (
        trace.get("trace_id") != "rav-hour-of-reckoning-convoke-token-safe-global-destruction-full-game"
        or trace.get("terminal") != "Winner(PlayerId(0))"
        or trace.get("winner_seat") != 0
        or trace.get("rejected_moves") != 0
        or trace.get("passed") is not True
        or card.get("collector_number") != 21
        or card.get("mana_cost") != "{4}{W}{W}{W}"
        or card.get("spell_zone_after_resolution") != "Graveyard"
        or card.get("terminal_combat_win") is not True
        or len(card.get("destroyed_nontoken_creatures", [])) != 2
        or len(card.get("surviving_white_knight_tokens", [])) != 3
        or len(set(card.get("surviving_white_knight_tokens", []))) != 3
    ):
        raise WorkerFailure("candidate_protocol_error: Convoke, global destruction, token survival, or terminal duel differs")

    mana = [str(color).lower() for color in card.get("direct_mana_paid", [])]
    if mana.count("white") != 2 or mana.count("green") != 3 or len(mana) != 5:
        raise WorkerFailure("candidate_protocol_error: direct mana did not pay exactly two white and three generic green")

    spell = card.get("object")
    destroyed = card.get("destroyed_nontoken_creatures")
    generic_creature = card.get("convoke_generic_creature")
    white_creature = card.get("convoke_white_creature")
    tokens = card.get("surviving_white_knight_tokens")
    identities = [spell, *destroyed, generic_creature, white_creature, *tokens]
    if (
        any(not isinstance(value, int) for value in identities)
        or len(set([spell, *destroyed])) != 3
        or generic_creature == white_creature
        or spell in (generic_creature, white_creature)
        or set(destroyed).intersection((generic_creature, white_creature))
        or generic_creature not in tokens
        or white_creature not in tokens
        or spell in tokens
        or set(destroyed).intersection(tokens)
    ):
        raise WorkerFailure("candidate_protocol_error: spell, destroyed creatures, convokers, and tokens have inconsistent identities")

    events = trace.get("events")
    required = [
        "ConvokeUsed", "ConvokeUsed", "SpellManaPaid", "SpellCast", "CardDestroyed",
        "SpellResolved", "CardMoved", "AttackersDeclared", "DamageDealtToPlayer", "GameEnded",
    ]
    if not isinstance(events, list) or [event.get("kind") for event in events] != required:
        raise WorkerFailure("candidate_protocol_error: incomplete ordered Convoke/destruction/combat event chain")
    sequences = [event.get("sequence") for event in events]
    if any(not isinstance(value, int) for value in sequences) or sequences != sorted(sequences):
        raise WorkerFailure("candidate_protocol_error: projected events are not ordered")
    facts = [event.get("facts", {}) for event in events]
    canonical = [event.get("canonical", "") for event in events]
    if (
        facts[0].get("objects") != [generic_creature]
        or facts[0].get("seats") != [0]
        or facts[0].get("convoke_contribution") != "generic"
        or facts[1].get("objects") != [white_creature]
        or facts[1].get("seats") != [0]
        or facts[1].get("convoke_contribution") != "White"
        or facts[2].get("objects") != [spell]
        or facts[2].get("mana_colors", []).count("White") != 2
        or facts[2].get("mana_colors", []).count("Green") != 3
        or facts[3].get("objects") != [spell]
        or facts[4].get("objects", [None, None])[0] != spell
        or len(facts[4].get("objects", [])) != 2
        or facts[4].get("objects", [None, None])[1] not in destroyed
        or facts[5].get("objects") != [spell]
        or facts[6].get("objects") != [spell]
        or facts[6].get("zone") != "Graveyard"
        or not set(tokens).intersection(facts[7].get("objects", []))
        or facts[8].get("seats") != [1]
        or facts[8].get("amount", 0) <= 0
        or "winner: Some(PlayerId(0))" not in canonical[9]
    ):
        raise WorkerFailure("candidate_protocol_error: typed Convoke, destruction, response, or end-game evidence differs")

    ordered = trace.get("ordered_event_indices", [])
    if len(ordered) != len(required) or any(not isinstance(value, int) for value in ordered) or ordered != sequences:
        raise WorkerFailure("candidate_protocol_error: raw event ordering receipt is incomplete")
    mutations = report.get("mutation_sensitivity", {})
    expected_mutations = (
        "convoke_removed_detected_in_game",
        "mana_cost_changed_detected_in_game",
        "nontoken_destruction_removed_detected_in_game",
        "syntactic_filesystem_side_effect_rejected_before_candidate_call",
    )
    if any(mutations.get(key) is not True for key in expected_mutations) or mutations.get("sealed_mutation_authority") is not False:
        raise WorkerFailure("candidate_protocol_error: Hour of Reckoning semantic mutation controls are incomplete")


def sandbox_command(workspace: Path, registry: Path, authority: Path, name: str, image: str, kind: str) -> list[str]:
    if authority.is_symlink() or not authority.is_file():
        raise WorkerFailure("trusted_authority_source_missing")
    flag = " --candidate" if kind == "agent" else ""
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
        "cp -R /submission/. /work/ && cp /trusted-authority.rs /work/varieties/magic/session/src/bin/rav_hour_of_reckoning_pvp.rs && cd /work && cargo run --offline --locked -j 2 --manifest-path varieties/magic/Cargo.toml -p cardbench-magic-session --bin " + BIN + " --" + flag,
    ]


def write_failure(output: Path, error: str) -> None:
    output.mkdir(parents=True, exist_ok=True)
    report = {
        "schema_version": "cardbench.magic.card-pvp.check.v1", "task_id": "cardbench/magic/card",
        "passed": False, "pilot_card_passed": False, "cards_passed": 0, "cards_total": 291,
        "printings_total": 306, "cards_credited": 0, "coverage_complete": False,
        "full_reward": False, "harbor_reward": 0.0, "pilot_reward": 0.0, "error": error,
    }
    (output / "result.json").write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    (output / "reward.txt").write_text("0.0\n", encoding="utf-8")


def score(workspace: Path, manifest: Path, output: Path, kind: str) -> int:
    output.mkdir(parents=True, exist_ok=True)
    data = verify_workspace(workspace, manifest)
    image = os.environ.get("CARDBENCH_MAGIC_WORKER_IMAGE", DEFAULT_WORKER_IMAGE)
    if not (image.startswith("rust@sha256:") or image.startswith("sha256:")):
        raise ValueError("worker image must use an immutable digest")
    authority = REPO / AUTHORITY
    registry = Path.home() / ".cargo" / "registry"
    if not registry.is_dir():
        raise FileNotFoundError("offline Cargo registry cache is missing")
    scoring_workspace = snapshot(workspace, data["files"], output / "candidate-workspace")
    candidate_hash = digest((scoring_workspace / TARGET).read_bytes())
    name = f"cardbench-magic-hour-of-reckoning-{uuid.uuid4().hex[:16]}"
    code, stdout, stderr = bounded(
        sandbox_command(scoring_workspace, registry.resolve(), authority, name, image, kind),
        timeout=900,
        name=name,
    )
    (output / "worker.stdout").write_bytes(stdout)
    (output / "worker.stderr").write_bytes(stderr)
    report = None
    if code == 0:
        for line in reversed(stdout.decode(errors="replace").splitlines()):
            try:
                candidate = json.loads(line)
            except json.JSONDecodeError:
                continue
            if candidate.get("schema_version") == "cardbench.magic.card-pvp.check.v1":
                validate_report(candidate)
                report = candidate
                break
    if report is None:
        report = {
            "schema_version": "cardbench.magic.card-pvp.check.v1", "task_id": "cardbench/magic/card",
            "candidate": {"kind": kind, "source_sha256": candidate_hash}, "passed": False,
            "pilot_card_passed": False, "cards_passed": 0, "cards_total": 291, "printings_total": 306,
            "cards_credited": 0, "coverage_complete": False, "full_reward": False,
            "harbor_reward": 0.0, "pilot_reward": 0.0, "error": "candidate compile or Hour of Reckoning trace failed",
            "diagnostics": stderr.decode(errors="replace")[-12000:],
        }
    else:
        report["candidate"] = {
            "kind": kind, "card_definition": "RAV-HOUR-OF-RECKONING", "source_sha256": candidate_hash,
            "materialized_workspace_sha256": data["workspace_sha256"],
        }
        report["candidate_kind"] = kind
        report["starting_workspace_sha256"] = data["workspace_sha256"]
        report["sealed_authority"] = False
        report["worker_image"] = image
        report["sandbox"] = {"network": "none", "rootfs": "read-only", "pids_limit": 128}
        report["cards_credited"] = 0
        report["harbor_reward"] = 0.0
        report["pilot_reward"] = 0.0
    (output / "result.json").write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    (output / "reward.txt").write_text("0.0\n", encoding="utf-8")
    return 0 if report.get("passed") is True and report.get("pilot_card_passed") is True else 1


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--workspace", type=Path)
    parser.add_argument("--manifest", type=Path)
    parser.add_argument("--output-root", type=Path)
    parser.add_argument("--candidate-kind", choices=["agent", "reference"], default="agent")
    parser.add_argument("--validate-report", type=Path)
    args = parser.parse_args()
    if args.validate_report is not None:
        try:
            validate_report(json.loads(args.validate_report.read_text(encoding="utf-8")))
        except Exception as error:
            print(f"Hour of Reckoning report validation failed: {error}", file=sys.stderr)
            return 1
        print("Hour of Reckoning report validated; rewards and credits are zero.")
        return 0
    if args.workspace is None or args.manifest is None or args.output_root is None:
        parser.error("--workspace, --manifest, and --output-root are required unless --validate-report is used")
    try:
        return score(args.workspace.resolve(), args.manifest.resolve(), args.output_root.resolve(), args.candidate_kind)
    except Exception as error:
        write_failure(args.output_root.resolve(), str(error))
        print(f"Hour of Reckoning verifier failed: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
