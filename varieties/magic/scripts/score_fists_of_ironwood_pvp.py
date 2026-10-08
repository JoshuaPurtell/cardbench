#!/usr/bin/env python3
"""Run the Fists of Ironwood reference authority in a bounded offline worker."""

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
MATERIALIZER = Path(__file__).with_name("materialize_fists_of_ironwood_task.py")
TARGET = Path("varieties/magic/sets/ravnica_city_of_guilds/src/cards/fists_of_ironwood.rs")
AUTHORITY_SOURCE = Path("varieties/magic/session/src/bin/rav_fists_of_ironwood_pvp.rs")
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
                (output if key.data == "stdout" else diagnostics).extend(chunk)
                if len(output) + len(diagnostics) > MAX_OUTPUT_BYTES:
                    raise WorkerFailure("candidate_output_limit: worker output exceeded 2 MiB")
        return proc.wait(), bytes(output), bytes(diagnostics)
    except BaseException:
        proc.kill()
        proc.wait()
        subprocess.run(["docker", "rm", "-f", name], capture_output=True, check=False)
        raise


def sandbox_command(
    workspace: Path, registry: Path, authority: Path, name: str, image: str, candidate_kind: str
) -> list[str]:
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
        "cp /trusted-authority.rs /work/varieties/magic/session/src/bin/rav_fists_of_ironwood_pvp.rs && "
        "cd /work && cargo run --offline --locked -j 2 --manifest-path varieties/magic/Cargo.toml "
        "-p cardbench-magic-session --bin rav-fists-of-ironwood-pvp --" + candidate_flag,
    ]


def validate_report(report: dict) -> None:
    if (
        report.get("schema_version") != "cardbench.magic.card-pvp.check.v1"
        or report.get("task_id") != "cardbench/magic/card"
        or report.get("expansion") != "RAV"
        or report.get("passed") is not True
        or report.get("pilot_card_passed") is not True
        or report.get("cards_passed") != 1
        or report.get("cards_credited") != 0
        or report.get("cards_total") != 291
        or report.get("printings_total") != 306
        or report.get("coverage_complete") is not False
        or report.get("full_reward") is not False
        or report.get("harbor_reward") != 0.0
        or report.get("pilot_reward") != 0.0
        or report.get("viewer_scope") != "Judge"
        or report.get("card") != {
            "collector_number": 164,
            "definition_id": "RAV-FISTS-OF-IRONWOOD",
            "name": "Fists of Ironwood",
        }
        or report.get("deterministic_replay") is not True
    ):
        raise WorkerFailure("candidate_protocol_error: incomplete Fists of Ironwood single-card report")
    traces = report.get("traces")
    if not isinstance(traces, list) or len(traces) != 1:
        raise WorkerFailure("candidate_protocol_error: exactly one Fists full-game trace is required")
    trace = traces[0]
    if (
        not isinstance(trace, dict)
        or trace.get("id") != "rav-fists-of-ironwood-trample-and-etb-two-saprolings-full-game"
        or trace.get("passed") is not True
        or trace.get("terminal") != "Winner(PlayerId(0))"
        or trace.get("winner_seat") != 0
        or trace.get("turns", 0) <= 0
        or trace.get("rejected_moves") != 0
        or trace.get("aura_attached_to_exact_target") is not True
        or trace.get("target_has_trample_at_attack") is not True
        or trace.get("block_was_declared") is not True
        or trace.get("target_damage_to_blocker") != 1
        or trace.get("target_trample_damage_to_player", 0) < 2
        or trace.get("saprolings_are_exact_green_1_1_creatures") != [True, True]
    ):
        raise WorkerFailure("candidate_protocol_error: Fists attachment, trigger, combat, or game result is invalid")

    fists = trace.get("fists_object")
    target = trace.get("target_object")
    blocker = trace.get("blocker_object")
    incarnation = trace.get("fists_incarnation_at_trigger")
    tokens = trace.get("saproling_objects")
    if (
        not all(isinstance(value, int) for value in (fists, target, blocker, incarnation))
        or not isinstance(tokens, list)
        or len(tokens) != 2
        or not all(isinstance(token, int) for token in tokens)
        or len(set(tokens)) != 2
        or not isinstance(trace.get("events_digest"), str)
    ):
        raise WorkerFailure("candidate_protocol_error: exact Aura, target, trigger incarnation, blocker, and token objects are required")

    events = trace.get("events")
    expected_kinds = [
        "SpellManaPaid", "SpellCast", "SpellResolved", "CardMoved", "AuraAttached",
        "TriggeredAbilityStacked", "TokenCreated", "TokenCreated", "AbilityResolved",
        "AttackersDeclared", "BlockersDeclared", "DamageDealtToPermanent",
        "DamageDealtToPlayer", "GameEnded",
    ]
    if not isinstance(events, list) or [event.get("kind") for event in events] != expected_kinds:
        raise WorkerFailure("candidate_protocol_error: ordered Fists cast, attachment, token, combat, and terminal events differ")
    sequences = [event.get("sequence") for event in events]
    if any(not isinstance(sequence, int) for sequence in sequences) or sequences != sorted(sequences):
        raise WorkerFailure("candidate_protocol_error: event sequence is malformed")

    expected_canonical = [
        f"SpellManaPaid {{ player: PlayerId(0), card: ObjectId({fists}), colors: [Green, Green] }}",
        f"SpellCast {{ player: PlayerId(0), card: ObjectId({fists}) }}",
        f"SpellResolved {{ card: ObjectId({fists}) }}",
        f"CardMoved {{ card: ObjectId({fists}), to: Battlefield }}",
        f"AuraAttached {{ aura: ObjectId({fists}), target: ObjectId({target}) }}",
        f'TriggeredAbilityStacked {{ controller: PlayerId(0), source: ObjectId({fists}), source_incarnation: {incarnation}, ability: "etb-two-saprolings" }}',
        f"TokenCreated {{ player: PlayerId(0), token: ObjectId({tokens[0]}) }}",
        f"TokenCreated {{ player: PlayerId(0), token: ObjectId({tokens[1]}) }}",
        f'AbilityResolved {{ source: ObjectId({fists}), source_incarnation: {incarnation}, ability: "etb-two-saprolings" }}',
        f"AttackersDeclared {{ player: PlayerId(0), attackers: [ObjectId({target}), ObjectId({tokens[0]}), ObjectId({tokens[1]})] }}",
        f"BlockersDeclared {{ player: PlayerId(1), assignments: [(ObjectId({target}), ObjectId({blocker}))] }}",
        f"DamageDealtToPermanent {{ source: ObjectId({target}), permanent: ObjectId({blocker}), amount: 1 }}",
        f"DamageDealtToPlayer {{ source: ObjectId({target}), player: PlayerId(1), amount: 2 }}",
        "GameEnded { winner: Some(PlayerId(0)) }",
    ]
    if [event.get("canonical") for event in events] != expected_canonical:
        raise WorkerFailure("candidate_protocol_error: Fists trace object identities or exact combat receipts differ")
    token_events = events[6:8]
    if any(
        event.get("facts", {}).get("objects") != [token]
        or event.get("facts", {}).get("seats") != [0]
        for event, token in zip(token_events, tokens, strict=True)
    ):
        raise WorkerFailure("candidate_protocol_error: Saproling token receipts do not match the exact created objects")
    mutations = report.get("mutation_sensitivity", {})
    if not isinstance(mutations, dict) or any(
        mutations.get(key) is not True
        for key in (
            "etb_trigger_removed_mutation_detected_in_game",
            "trample_attachment_effect_removed_mutation_detected_in_game",
            "saproling_count_changed_mutation_detected_in_game",
        )
    ):
        raise WorkerFailure("candidate_protocol_error: every Fists semantic mutant must fail")


def parse_report(stdout: bytes) -> dict:
    for line in reversed(stdout.decode(errors="replace").splitlines()):
        try:
            value = json.loads(line)
        except json.JSONDecodeError:
            continue
        if value.get("schema_version") == "cardbench.magic.card-pvp.check.v1":
            validate_report(value)
            return value
    raise WorkerFailure("candidate_protocol_error: no valid Fists of Ironwood PvP result")


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
        raise FileNotFoundError(f"trusted Fists of Ironwood authority source is missing: {authority}")
    if not shutil.which("docker"):
        raise FileNotFoundError("Docker is required for Magic candidate isolation")
    verify_workspace(workspace, manifest)
    scoring_workspace = make_scoring_snapshot(workspace, manifest_data["files"], output_root / "candidate-workspace")
    candidate_hash = sha256((scoring_workspace / TARGET).read_bytes())
    name = f"cardbench-magic-fists-of-ironwood-{uuid.uuid4().hex[:16]}"
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
            "error": "candidate compile or Fists of Ironwood reference trace failed",
            "diagnostics": stderr.decode(errors="replace")[-12000:],
        }
    else:
        result = parse_report(stdout)
        result["candidate"] = {
            "kind": candidate_kind, "card_definition": "RAV-FISTS-OF-IRONWOOD", "source_sha256": candidate_hash,
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
