#!/usr/bin/env python3
"""Run the Selesnya Signet pilot in a networkless bounded worker."""

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
MATERIALIZER = Path(__file__).with_name("materialize_selesnya_signet_task.py")
TARGET = Path("varieties/magic/sets/ravnica_city_of_guilds/src/cards/selesnya_signet.rs")
AUTHORITY_SOURCE = Path("varieties/magic/session/src/bin/rav_selesnya_signet_pvp.rs")
DEFAULT_WORKER_IMAGE = "rust@sha256:d7482085ff5b415f84dba5647ae71606650bdef00db7aeb69f4b3d170c3e4082"
MAX_OUTPUT_BYTES = 2 * 1024 * 1024


class WorkerFailure(RuntimeError):
    pass


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def verify_workspace(workspace: Path, manifest: Path) -> None:
    subprocess.run([sys.executable, str(MATERIALIZER), "--workspace", str(workspace), "--manifest", str(manifest), "--verify"], check=True, cwd=REPO)


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
                (output if key.data == "stdout" else diagnostics).extend(chunk)
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
        "cp /trusted-authority.rs /work/varieties/magic/session/src/bin/rav_selesnya_signet_pvp.rs && "
        "cd /work && cargo run --offline --locked -j 2 --manifest-path varieties/magic/Cargo.toml "
        "-p cardbench-magic-session --bin rav-selesnya-signet-pvp --" + candidate_flag,
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
    raise WorkerFailure("candidate_protocol_error: no valid Selesnya Signet PvP result")


def has_subsequence(actual: list[str], expected: list[str]) -> bool:
    cursor = iter(actual)
    return all(any(item == target for item in cursor) for target in expected)


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
        or report.get("card") != {"collector_number": 270, "definition_id": "RAV-SELESNYA-SIGNET", "name": "Selesnya Signet"}
        or report.get("rules_authority", {}).get("comprehensive_rules") != "2026-08-07"
    ):
        raise WorkerFailure("candidate_protocol_error: incomplete Selesnya Signet report")
    traces = report.get("traces")
    if not isinstance(traces, list) or len(traces) != 1:
        raise WorkerFailure("candidate_protocol_error: one Selesnya Signet trace is required")
    trace = traces[0]
    if (
        not isinstance(trace, dict)
        or trace.get("id") != "rav-selesnya-signet-casts-watchwolf"
        or trace.get("passed") is not True
        or trace.get("terminal") != "Winner(PlayerId(0))"
        or trace.get("winner_seat") != 0
        or not isinstance(trace.get("life_at_match_end"), list)
        or len(trace.get("life_at_match_end", [])) != 2
        or trace.get("life_at_match_end", [None, None])[1] > 0
        or trace.get("signet_activation") != {"ability": "selesnya-signet-gw", "generic_cost_paid": 1, "mana_output": {"green": 1, "white": 1}, "non_stack": True, "tapped": True}
        or trace.get("watchwolf_power_toughness") != {"power": 3, "toughness": 3}
        or trace.get("watchwolf_zone") != "Some(Battlefield)"
        or trace.get("watchwolf_keywords") != []
        or not trace.get("watchwolf_attack_turns")
        or trace.get("watchwolf_attack_turns")[0] <= trace.get("watchwolf_cast_turn")
    ):
        raise WorkerFailure("candidate_protocol_error: paid green-white bundle, live vanilla Watchwolf, later-turn combat, and terminal game are required")
    payments = trace.get("cast_payments", {})
    signet_payment = payments.get("selesnya_signet", {})
    watchwolf_payment = payments.get("watchwolf", {})
    spent = watchwolf_payment.get("spent", [])
    if (
        signet_payment != {"paid_total": 2, "required_generic_symbols": 2}
        or watchwolf_payment.get("required_generic_symbols") != 0
        or watchwolf_payment.get("green_symbol") is not True
        or watchwolf_payment.get("white_symbol") is not True
        or watchwolf_payment.get("total") != 2
        or not isinstance(spent, list)
        or "Green" not in spent
        or "White" not in spent
    ):
        raise WorkerFailure("candidate_protocol_error: real Signet and colored Watchwolf costs differ")
    events = trace.get("events")
    if not isinstance(events, list) or not all(isinstance(event, dict) for event in events):
        raise WorkerFailure("candidate_protocol_error: projected event list is malformed")
    kinds = [event.get("kind") for event in events]
    expected = [
        "SpellManaPaid", "SpellCast", "SpellResolved", "CardMoved",
        "BoundManaAbilityBundleActivated", "ManaAbilityManaPaid",
        "SpellManaPaid", "SpellCast", "SpellResolved", "CardMoved",
        "AttackersDeclared", "DamageDealtToPlayer", "GameEnded",
    ]
    if not has_subsequence(kinds, expected):
        raise WorkerFailure("candidate_protocol_error: ordered artifact cast, paid bundle, colored spell cast, combat and terminal events differ")
    if trace.get("decks") != ["rav_selesnya_signet_payment", "rav_lands_only"]:
        raise WorkerFailure("candidate_protocol_error: legal Selesnya Signet payment decks differ")
    bundle_event = next((event for event in events if event.get("kind") == "BoundManaAbilityBundleActivated"), None)
    if not isinstance(bundle_event, dict) or "selesnya-signet-gw" not in bundle_event.get("canonical", "") or "generic: 1" not in bundle_event.get("canonical", "") or "White: 1" not in bundle_event.get("canonical", "") or "Green: 1" not in bundle_event.get("canonical", "") or "tapped: true" not in bundle_event.get("canonical", ""):
        raise WorkerFailure("candidate_protocol_error: tap, one generic activation cost, and exact green-white bundle receipt differ")
    ability_cost_event = next((event for event in events if event.get("kind") == "ManaAbilityManaPaid"), None)
    if not isinstance(ability_cost_event, dict) or "generic: 1" not in ability_cost_event.get("canonical", ""):
        raise WorkerFailure("candidate_protocol_error: paid one generic ability-cost event is missing")
    watchwolf_reference = f"ObjectId({trace.get('watchwolf_object')})"
    if not any(
        event.get("kind") == "DamageDealtToPlayer"
        and watchwolf_reference in event.get("canonical", "")
        and "player: PlayerId(1)" in event.get("canonical", "")
        and "amount: 3" in event.get("canonical", "")
        for event in events
    ):
        raise WorkerFailure("candidate_protocol_error: the live Watchwolf must deal three combat damage")
    signet_reference = f"ObjectId({trace.get('selesnya_signet_object')})"
    if any(event.get("kind") == "AbilityActivated" and signet_reference in event.get("canonical", "") for event in events):
        raise WorkerFailure("candidate_protocol_error: mana ability was placed on the stack")
    sequences = [event.get("sequence") for event in events]
    if any(not isinstance(item, int) for item in sequences) or sequences != sorted(sequences):
        raise WorkerFailure("candidate_protocol_error: event sequence is malformed")
    positions = trace.get("ordered_event_sequence")
    if not isinstance(positions, list) or any(not isinstance(item, int) for item in positions) or positions != sorted(positions):
        raise WorkerFailure("candidate_protocol_error: required event positions are not ordered")
    mutation = report.get("mutation_sensitivity", {})
    if not isinstance(mutation, dict) or mutation.get("wrong_signet_bundle_mutation_detected") is not True:
        raise WorkerFailure("candidate_protocol_error: wrong-color Signet bundle mutation must fail")


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
        raise FileNotFoundError(f"trusted Selesnya Signet authority source is missing: {authority}")
    if not shutil.which("docker"):
        raise FileNotFoundError("Docker is required for Magic candidate isolation")
    verify_workspace(workspace, manifest)
    scoring_workspace = make_scoring_snapshot(workspace, manifest_data["files"], output_root / "candidate-workspace")
    candidate_hash = sha256((scoring_workspace / TARGET).read_bytes())
    name = f"cardbench-magic-selesnya-signet-{uuid.uuid4().hex[:16]}"
    command = sandbox_command(scoring_workspace, cargo_registry.resolve(), authority, name, worker_image, candidate_kind)
    code, stdout, stderr = run_bounded(command, timeout=900, name=name)
    (output_root / "worker.stdout").write_bytes(stdout)
    (output_root / "worker.stderr").write_bytes(stderr)
    if code != 0:
        result = {
            "schema_version": "cardbench.magic.card-pvp.check.v1", "task_id": "cardbench/magic/card",
            "candidate": {"kind": candidate_kind, "source_sha256": candidate_hash}, "passed": False,
            "pilot_card_passed": False, "cards_passed": 0, "cards_total": 291, "printings_total": 306,
            "cards_credited": 0, "coverage_complete": False, "full_reward": False,
            "harbor_reward": 0.0, "pilot_reward": 0.0, "sealed_authority": False,
            "worker_image": worker_image, "sandbox": {"network": "none", "rootfs": "read-only", "pids_limit": 128},
            "error": "candidate compile or Selesnya Signet reference trace failed",
            "diagnostics": stderr.decode(errors="replace")[-12000:],
        }
    else:
        result = parse_report(stdout)
        result["candidate"] = {"kind": candidate_kind, "card_definition": "RAV-SELESNYA-SIGNET", "source_sha256": candidate_hash, "materialized_workspace_sha256": manifest_data["workspace_sha256"]}
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
            "schema_version": "cardbench.magic.card-pvp.check.v1", "task_id": "cardbench/magic/card",
            "passed": False, "pilot_card_passed": False, "cards_passed": 0, "cards_total": 291,
            "printings_total": 306, "cards_credited": 0, "coverage_complete": False,
            "full_reward": False, "harbor_reward": 0.0, "sealed_authority": False, "error": str(error),
        }
        (args.output_root / "result.json").write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
        (args.output_root / "reward.txt").write_text("0.0\n", encoding="utf-8")
        print(f"Magic candidate verifier failed: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
