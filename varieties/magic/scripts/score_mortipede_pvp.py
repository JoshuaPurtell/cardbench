#!/usr/bin/env python3
"""Run Mortipede in an offline, resource-limited candidate worker."""

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
MATERIALIZER = Path(__file__).with_name("materialize_mortipede_task.py")
TARGET = Path("varieties/magic/sets/ravnica_city_of_guilds/src/cards/mortipede.rs")
AUTHORITY_SOURCE = Path("varieties/magic/session/src/bin/rav_mortipede_pvp.rs")
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
        "cp /trusted-authority.rs /work/varieties/magic/session/src/bin/rav_mortipede_pvp.rs && "
        "cd /work && cargo run --offline --locked -j 2 --manifest-path varieties/magic/Cargo.toml "
        "-p cardbench-magic-session --bin rav-mortipede-pvp --" + candidate_flag,
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
    raise WorkerFailure("candidate_protocol_error: no valid Mortipede PvP result")


def validate_report(report: dict) -> None:
    card_id = "RAV-MORTIPEDE"
    expected_card = {"collector_number": 96, "definition_id": card_id, "name": "Mortipede"}
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
        or report.get("card") != expected_card
    ):
        raise WorkerFailure("candidate_protocol_error: incomplete Mortipede single-card report")
    candidate = report.get("candidate")
    if not isinstance(candidate, dict) or candidate.get("card_definition") != card_id:
        raise WorkerFailure("candidate_protocol_error: wrong Mortipede identity")
    rules = report.get("rules_authority")
    if not isinstance(rules, dict) or rules.get("comprehensive_rules") != "2026-08-07" or rules.get("authority_cutoff") != "2026-09-23" or not any("509.1" in section for section in rules.get("checked_sections", [])):
        raise WorkerFailure("candidate_protocol_error: Mortipede rules authority is inconsistent")
    traces = report.get("traces")
    if not isinstance(traces, list) or len(traces) != 1 or traces[0].get("trace_id") != "rav-mortipede-green-must-be-blocked-pvp":
        raise WorkerFailure("candidate_protocol_error: one Mortipede full-game trace is required")
    trace = traces[0]
    mortipede = trace.get("mortipede")
    activation = trace.get("activation")
    blocker = trace.get("blocker")
    combat = trace.get("combat")
    payment = mortipede.get("mana_paid") if isinstance(mortipede, dict) else None
    if (
        trace.get("terminal") != "Winner(PlayerId(0))"
        or trace.get("winner_seat") != 0
        or trace.get("rejected_moves") != 0
        or not isinstance(trace.get("shuffle_seed"), int)
        or not all(isinstance(value, dict) for value in (mortipede, activation, blocker, combat))
        or mortipede.get("collector_number") != 96
        or mortipede.get("mana_cost") != "{3}{B}"
        or (mortipede.get("base_power"), mortipede.get("base_toughness")) != (4, 1)
        or not isinstance(mortipede.get("object"), int)
        or not isinstance(payment, list) or len(payment) != 4 or "Black" not in payment
        or activation.get("ability") != "green-must-be-blocked"
        or activation.get("mana_cost") != "{2}{G}"
        or activation.get("green_mana_paid") is not True
        or activation.get("resolved_before_attack") is not True
        or blocker.get("definition") != "RAV-LURKING-INFORMANT"
        or blocker.get("must_block_assignment") is not True
        or combat.get("actual_required_block") is not True
        or combat.get("both_creatures_died") is not True
        or not isinstance(combat.get("support_damage_to_finish_game"), int) or combat["support_damage_to_finish_game"] < 20
    ):
        raise WorkerFailure("candidate_protocol_error: Mortipede identity, payment, required block, combat, or terminal result is wrong")
    events = trace.get("events")
    if not isinstance(events, list) or len(events) < 15 or any(not isinstance(event, dict) for event in events):
        raise WorkerFailure("candidate_protocol_error: projected Mortipede event trace is missing or incomplete")
    sequence = [event.get("sequence") for event in events]
    if any(not isinstance(value, int) for value in sequence) or sequence != sorted(sequence):
        raise WorkerFailure("candidate_protocol_error: projected event sequence is malformed")

    def event_index(kind: str, predicate) -> int:
        for index, event in enumerate(events):
            if event.get("kind") == kind and predicate(event.get("facts", {}), event.get("canonical", "")):
                return index
        raise WorkerFailure(f"candidate_protocol_error: missing {kind} receipt")

    object_id = mortipede["object"]
    blocker_id = blocker["object"]
    paid = event_index("SpellManaPaid", lambda facts, _: facts.get("objects") == [object_id] and facts.get("seats") == [0] and facts.get("amount") == 4 and "Black" in facts.get("mana_colors", []))
    cast = event_index("SpellCast", lambda facts, _: facts.get("objects") == [object_id] and facts.get("seats") == [0])
    resolved_spell = event_index("SpellResolved", lambda facts, _: facts.get("objects") == [object_id])
    entered = event_index("CardMoved", lambda facts, _: facts.get("objects") == [object_id] and facts.get("zone") == "Battlefield")
    mana_paid = event_index("AbilityManaPaid", lambda facts, _: facts.get("objects") == [object_id] and facts.get("seats") == [0] and facts.get("ability") == activation["ability"] and facts.get("mana_cost_generic") == 2 and facts.get("mana_cost_colors") == ["Green"])
    activated = event_index("AbilityActivated", lambda facts, _: facts.get("objects") == [object_id] and facts.get("seats") == [0] and facts.get("ability") == activation["ability"])
    ability_resolved = event_index("AbilityResolved", lambda facts, _: facts.get("objects") == [object_id] and facts.get("ability") == activation["ability"])
    attack = event_index("AttackersDeclared", lambda facts, _: facts.get("seats") == [0] and object_id in facts.get("objects", []))
    blocked = event_index("BlockersDeclared", lambda facts, _: facts.get("seats") == [1] and object_id in facts.get("objects", []) and blocker_id in facts.get("objects", []))
    combat_damage = event_index("DamageDealtToPermanent", lambda facts, _: facts.get("objects") == [object_id, blocker_id] and facts.get("amount") == 4)
    ended = event_index("GameEnded", lambda facts, _: facts.get("seats") == [0])
    if not (paid < cast < resolved_spell < entered < mana_paid < activated < ability_resolved < attack < blocked < combat_damage < ended):
        raise WorkerFailure("candidate_protocol_error: Mortipede cast, activation, blocking, combat, and terminal event order is wrong")
    mutations = report.get("mutation_sensitivity")
    required = ("must_be_blocked_effect_removal_detected_in_game", "activation_cost_mutation_detected_in_game", "cast_cost_mutation_detected_in_game", "base_power_mutation_detected_in_game", "base_toughness_mutation_detected_in_game")
    if not isinstance(mutations, dict) or any(mutations.get(key) is not True for key in required) or mutations.get("sealed_mutation_authority") is not False:
        raise WorkerFailure("candidate_protocol_error: Mortipede gameplay mutations are incomplete")

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
        raise FileNotFoundError(f"trusted Mortipede authority is missing: {authority}")
    if not shutil.which("docker"):
        raise FileNotFoundError("Docker is required for Magic candidate isolation")
    verify_workspace(workspace, manifest)
    scoring_workspace = make_scoring_snapshot(workspace, manifest_data["files"], output_root / "candidate-workspace")
    candidate_hash = sha256((scoring_workspace / TARGET).read_bytes())
    name = f"cardbench-magic-mortipede-{uuid.uuid4().hex[:16]}"
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
            "error": "candidate compile or Mortipede reference trace failed",
            "diagnostics": stderr.decode(errors="replace")[-12000:],
        }
    else:
        result = parse_report(stdout)
        result["candidate"] = {
            "kind": candidate_kind,
            "card_definition": "RAV-MORTIPEDE",
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
