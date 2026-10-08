#!/usr/bin/env python3
"""Run Scatter the Seeds in a networkless worker and validate its PvP report."""

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
MATERIALIZER = Path(__file__).with_name("materialize_scatter_the_seeds_task.py")
TARGET = Path("varieties/magic/sets/ravnica_city_of_guilds/src/cards/scatter_the_seeds.rs")
AUTHORITY_SOURCE = Path("varieties/magic/session/src/bin/rav_scatter_the_seeds_pvp.rs")
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
                destination = stdout if key.data == "stdout" else stderr
                destination.extend(chunk)
                if len(stdout) + len(stderr) > MAX_OUTPUT_BYTES:
                    raise WorkerFailure("candidate_output_limit: worker output exceeded 2 MiB")
        return process.wait(), bytes(stdout), bytes(stderr)
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
        "cp /trusted-authority.rs /work/varieties/magic/session/src/bin/rav_scatter_the_seeds_pvp.rs && "
        "cd /work && cargo run --offline --locked -j 2 --manifest-path varieties/magic/Cargo.toml "
        "-p cardbench-magic-session --bin rav-scatter-the-seeds-pvp --" + candidate_flag,
    ]


def fail(message: str) -> None:
    raise WorkerFailure(f"candidate_protocol_error: {message}")


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
        or report.get("deterministic_replay") is not True
        or report.get("card") != {"collector_number": 181, "definition_id": "RAV-SCATTER-THE-SEEDS", "name": "Scatter the Seeds"}
    ):
        fail("incomplete Scatter the Seeds diagnostic or incorrect 291/306 RAV denominator")
    candidate = report.get("candidate")
    if not isinstance(candidate, dict) or candidate.get("card_definition") != "RAV-SCATTER-THE-SEEDS":
        fail("wrong Scatter the Seeds candidate identity")
    traces = report.get("traces")
    if not isinstance(traces, list) or len(traces) != 1:
        fail("one full-game Scatter the Seeds trace is required")
    trace = traces[0]
    if (
        not isinstance(trace, dict)
        or trace.get("trace_id") != "rav-scatter-the-seeds-convoke-three-saprolings-full-game"
        or trace.get("passed") is not True
        or trace.get("terminal") != "Winner(PlayerId(0))"
        or trace.get("winner_seat") != 0
        or trace.get("rejected_moves") != 0
        or trace.get("decks") != ["rav_scatter_elves_forest_60", "rav_island_passive_60"]
        or trace.get("policies") != ["rav.scatter-the-seeds-convoke-tokens.v1", "rav.scatter-the-seeds-passive-opponent.v1"]
        or not isinstance(trace.get("accepted_moves"), int) or trace["accepted_moves"] <= 0
        or not isinstance(trace.get("turns"), int) or trace["turns"] <= 0
    ):
        fail("Scatter the Seeds did not reach a clean terminal full-game result")
    card = trace.get("scatter_the_seeds")
    if not isinstance(card, dict) or (
        card.get("collector_number") != 181
        or card.get("mana_cost") != "{3}{G}{G}"
        or card.get("mana_paid") != ["Green"] * 3
        or card.get("generic_paid_by_convoke") != 2
        or card.get("convoke_contribution") != "Generic"
        or card.get("convoke_creatures_tapped") != [True, True]
        or card.get("resolved") is not True
        or card.get("zone_after_resolution") != "Some(Graveyard)"
        or len(card.get("convoke_creatures", [])) != 2
        or len(card.get("token_objects", [])) != 3
        or card.get("live_token_characteristics") != {"name": "Saproling", "colors": ["Green"], "card_types": ["Creature"], "creature_subtypes": ["Saproling"], "power": 1, "toughness": 1, "keywords": []}
        or not isinstance(card.get("attack_batches"), list) or not card["attack_batches"]
        or not isinstance(card.get("token_combat_damage"), list)
        or sum(item.get("amount", 0) for item in card["token_combat_damage"] if isinstance(item, dict)) < 20
        or not isinstance(card.get("opponent_life_after_combat"), int) or card["opponent_life_after_combat"] > 0
    ):
        fail("exact Convoke payment, live Saproling bodies, or terminal token combat differs")
    events = trace.get("events")
    if not isinstance(events, list) or len(events) < 12:
        fail("typed Scatter trace omitted payment, resolution, token, or combat events")
    kinds = [event.get("kind") for event in events]
    expected_prefix = ["ConvokeUsed", "ConvokeUsed", "SpellManaPaid", "SpellCast", "TokenCreated", "TokenCreated", "TokenCreated", "SpellResolved", "CardMoved", "AttackersDeclared"]
    if kinds[:10] != expected_prefix or kinds[-1] != "GameEnded" or kinds[10:-1].count("AttackersDeclared") < 1 or kinds[10:-1].count("DamageDealtToPlayer") < 20 or any(kind not in {"AttackersDeclared", "DamageDealtToPlayer"} for kind in kinds[10:-1]):
        fail("typed event stream is missing or reorders Convoke, payment, spell resolution, token entry, combat, and terminal events")
    sequences = [event.get("sequence") for event in events]
    if any(type(sequence) is not int for sequence in sequences) or sequences != sorted(set(sequences)):
        fail("typed event sequence is not strictly increasing")
    facts = [event.get("facts", {}) for event in events]
    spell = card.get("object")
    tokens = card.get("token_objects")
    if (
        type(spell) is not int
        or any(facts[i].get("seats") != [0] or facts[i].get("contribution") != "Generic" or len(facts[i].get("objects", [])) != 1 for i in (0, 1))
        or facts[2].get("seats") != [0] or facts[2].get("objects") != [spell] or facts[2].get("amount") != 3 or facts[2].get("mana_colors") != ["Green"] * 3
        or facts[3].get("seats") != [0] or facts[3].get("objects") != [spell]
        or facts[7].get("objects") != [spell]
        or facts[8].get("objects") != [spell] or facts[8].get("zone") != "Graveyard"
        or [facts[i].get("objects") for i in (4, 5, 6)] != [[token] for token in tokens]
        or any(facts[i].get("seats") != [0] for i in (4, 5, 6))
        or facts[9].get("seats") != [0] or set(facts[9].get("objects", [])) != set(tokens)
        or any(
            (event.get("kind") == "AttackersDeclared" and (fact.get("seats") != [0] or not set(fact.get("objects", [])).issubset(set(tokens))))
            or (event.get("kind") == "DamageDealtToPlayer" and (fact.get("seats") != [1] or fact.get("amount") != 1 or fact.get("objects", [None])[0] not in tokens))
            for event, fact in zip(events[10:-1], facts[10:-1], strict=True)
        )
        or facts[-1].get("seats") != [0]
    ):
        fail("typed event facts do not bind the exact spell, two generic contributors, three tokens, and token combat")
    mutation = report.get("mutation_sensitivity", {})
    required = ("convoke_removed_mutation_detected", "generic_cost_increased_mutation_detected", "two_token_mutation_detected", "wrong_saproling_body_mutation_detected")
    if not isinstance(mutation, dict) or any(mutation.get(key) is not True for key in required) or mutation.get("sealed_mutation_authority") is not False:
        fail("compiled Scatter the Seeds semantic mutation controls are incomplete")

def parse_report(stdout: bytes) -> dict:
    for line in reversed(stdout.decode(errors="replace").splitlines()):
        try:
            report = json.loads(line)
        except json.JSONDecodeError:
            continue
        if report.get("schema_version") == "cardbench.magic.card-pvp.check.v1":
            validate_report(report)
            return report
    fail("no valid Scatter the Seeds PvP result")


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
        raise FileNotFoundError(f"trusted Scatter the Seeds authority source is missing: {authority}")
    if not shutil.which("docker"):
        raise FileNotFoundError("Docker is required for Magic candidate isolation")
    verify_workspace(workspace, manifest)
    scoring_workspace = make_scoring_snapshot(workspace, manifest_data["files"], output_root / "candidate-workspace")
    candidate_hash = sha256((scoring_workspace / TARGET).read_bytes())
    name = f"cardbench-magic-scatter-the-seeds-{uuid.uuid4().hex[:16]}"
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
            "pilot_reward": 0.0,
            "sealed_authority": False,
            "worker_image": worker_image,
            "sandbox": {"network": "none", "rootfs": "read-only", "pids_limit": 128},
            "error": "candidate compile or Scatter the Seeds reference trace failed",
            "diagnostics": stderr.decode(errors="replace")[-12000:],
        }
    else:
        result = parse_report(stdout)
        result["candidate"] = {
            "kind": candidate_kind,
            "card_definition": "RAV-SCATTER-THE-SEEDS",
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
        print(f"Scatter the Seeds candidate verifier failed: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
