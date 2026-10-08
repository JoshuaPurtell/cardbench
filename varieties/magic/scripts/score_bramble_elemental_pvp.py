#!/usr/bin/env python3
"""Score the isolated RAV Bramble Elemental PvP task offline."""

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
MATERIALIZER = Path(__file__).with_name("materialize_bramble_elemental_task.py")
TARGET = Path("varieties/magic/sets/ravnica_city_of_guilds/src/cards/bramble_elemental.rs")
AUTHORITY_SOURCE = Path("varieties/magic/session/src/bin/rav_bramble_elemental_pvp.rs")
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
        "cp /trusted-authority.rs /work/varieties/magic/session/src/bin/rav_bramble_elemental_pvp.rs && "
        "cd /work && cargo run --offline --locked -j 2 --manifest-path varieties/magic/Cargo.toml "
        "-p cardbench-magic-session --bin rav-bramble-elemental-pvp --" + candidate_flag,
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
            "collector_number": 154,
            "definition_id": "RAV-BRAMBLE-ELEMENTAL",
            "name": "Bramble Elemental",
        }
        or report.get("deterministic_replay") is not True
    ):
        raise WorkerFailure("candidate_protocol_error: incomplete Bramble Elemental single-card report")
    mutations = report.get("mutation_sensitivity")
    mutation_keys = (
        "trigger_removed_mutation_detected_in_game",
        "controller_wide_aura_entry_instead_of_exact_attachment_detected_in_game",
        "wrong_saproling_count_mutation_detected_in_game",
        "bramble_etb_instead_of_aura_attachment_detected_in_game",
    )
    if not isinstance(mutations, dict) or any(mutations.get(key) is not True for key in mutation_keys):
        raise WorkerFailure("candidate_protocol_error: every Bramble semantic mutant must fail")
    traces = report.get("traces")
    if not isinstance(traces, list) or len(traces) != 1:
        raise WorkerFailure("candidate_protocol_error: exactly one Bramble full-game trace is required")
    trace = traces[0]
    if (
        not isinstance(trace, dict)
        or trace.get("id") != "rav-bramble-elemental-exact-aura-attachment-two-saprolings-full-game"
        or trace.get("passed") is not True
        or trace.get("terminal") != "Winner(PlayerId(0))"
        or trace.get("winner_seat") != 0
        or trace.get("turns", 0) <= 0
        or trace.get("rejected_moves") != 0
        or trace.get("exact_source_attachment_only") is not True
        or trace.get("saprolings_are_exact_green_1_1_creatures") != [True, True]
        or trace.get("both_saprolings_attacked") is not True
    ):
        raise WorkerFailure("candidate_protocol_error: Bramble attachment, token, or terminal-game trace is invalid")
    objects = (
        trace.get("bramble_object"), trace.get("bramble_aura_object"),
        trace.get("bramble_aura_target"), trace.get("other_aura_object"),
        trace.get("other_aura_target"), trace.get("trigger_incarnation"),
    )
    tokens = trace.get("saproling_objects")
    if (
        not all(isinstance(value, int) for value in objects)
        or not isinstance(tokens, list)
        or len(tokens) != 2
        or len(set(tokens)) != 2
        or not all(isinstance(token, int) for token in tokens)
        or trace.get("bramble_aura_target") != trace.get("bramble_object")
        or trace.get("other_aura_target") == trace.get("bramble_object")
        or not isinstance(trace.get("events_digest"), str)
    ):
        raise WorkerFailure("candidate_protocol_error: exact source, Aura, trigger-incarnation, and token objects are required")
    events = trace.get("events")
    required_kinds = {"SpellManaPaid", "SpellCast", "SpellResolved", "CardMoved", "AuraAttached", "TriggeredAbilityStacked", "TokenCreated", "AbilityResolved", "AttackersDeclared", "GameEnded"}
    if not isinstance(events, list) or not required_kinds.issubset({event.get("kind") for event in events if isinstance(event, dict)}):
        raise WorkerFailure("candidate_protocol_error: Bramble event evidence is incomplete")
    sequences = [event.get("sequence") for event in events]
    if any(not isinstance(sequence, int) for sequence in sequences) or sequences != sorted(sequences):
        raise WorkerFailure("candidate_protocol_error: event sequence is malformed")
    canonical = [event.get("canonical", "") for event in events]
    bramble_id = trace["bramble_object"]
    aura_id = trace["bramble_aura_object"]
    target_id = trace["bramble_aura_target"]
    other_aura_id = trace["other_aura_object"]
    other_target_id = trace["other_aura_target"]
    if not any(f"AuraAttached {{ aura: ObjectId({aura_id}), target: ObjectId({target_id}) }}" in value for value in canonical):
        raise WorkerFailure("candidate_protocol_error: exact Bramble Aura attachment is absent")
    if not any(f"AuraAttached {{ aura: ObjectId({other_aura_id}), target: ObjectId({other_target_id}) }}" in value for value in canonical):
        raise WorkerFailure("candidate_protocol_error: negative other-creature Aura attachment is absent")
    if not any(f'TriggeredAbilityStacked {{ controller: PlayerId(0), source: ObjectId({bramble_id})' in value for value in canonical):
        raise WorkerFailure("candidate_protocol_error: source-controlled Bramble trigger receipt is absent")
    if sum(value.startswith("TokenCreated ") for value in canonical) != 2:
        raise WorkerFailure("candidate_protocol_error: exact two Saproling creation receipts are required")
    if not any("GameEnded { winner: Some(PlayerId(0)) }" in value for value in canonical):
        raise WorkerFailure("candidate_protocol_error: terminal seat-zero win receipt is absent")


def parse_report(stdout: bytes) -> dict:
    for line in reversed(stdout.decode(errors="replace").splitlines()):
        try:
            value = json.loads(line)
        except json.JSONDecodeError:
            continue
        if isinstance(value, dict) and value.get("schema_version") == "cardbench.magic.card-pvp.check.v1":
            validate_report(value)
            return value
    raise WorkerFailure("candidate_protocol_error: no valid Bramble Elemental PvP result")


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
        raise FileNotFoundError(f"trusted Bramble Elemental authority is missing: {authority}")
    if not shutil.which("docker"):
        raise FileNotFoundError("Docker is required for Magic candidate isolation")
    verify_workspace(workspace, manifest)
    scoring_workspace = make_scoring_snapshot(workspace, manifest_data["files"], output_root / "candidate-workspace")
    candidate_hash = sha256((scoring_workspace / TARGET).read_bytes())
    name = f"cardbench-magic-bramble-elemental-{uuid.uuid4().hex[:16]}"
    command = sandbox_command(scoring_workspace, registry.resolve(), authority, name, image, candidate_kind)
    code, stdout, stderr = run_bounded(command, timeout=900, name=name)
    (output_root / "worker.stdout").write_bytes(stdout)
    (output_root / "worker.stderr").write_bytes(stderr)
    if code != 0:
        report = {"schema_version":"cardbench.magic.card-pvp.check.v1","task_id":"cardbench/magic/card","passed":False,"pilot_card_passed":False,"cards_passed":0,"cards_total":291,"printings_total":306,"cards_credited":0,"coverage_complete":False,"full_reward":False,"harbor_reward":0.0,"pilot_reward":0.0,"error":"candidate compile or Bramble reference trace failed","diagnostics":stderr.decode(errors="replace")[-12000:]}
    else:
        report = parse_report(stdout)
        report["candidate"] = {"kind":candidate_kind,"card_definition":"RAV-BRAMBLE-ELEMENTAL","source_sha256":candidate_hash,"materialized_workspace_sha256":manifest_data["workspace_sha256"]}
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
    parser.add_argument("--workspace", type=Path)
    parser.add_argument("--manifest", type=Path)
    parser.add_argument("--output-root", type=Path)
    parser.add_argument("--candidate-kind", choices=["reference", "agent"], default="agent")
    parser.add_argument("--validate-report", type=Path)
    args = parser.parse_args()
    if args.validate_report:
        validate_report(json.loads(args.validate_report.read_text(encoding="utf-8")))
        print("report_valid=true pilot_reward=0.0 harbor_reward=0.0 cards_credited=0")
        return 0
    if not args.workspace or not args.manifest or not args.output_root:
        parser.error("--workspace, --manifest, and --output-root are required unless --validate-report is used")
    return score(args.workspace.resolve(), args.manifest.resolve(), args.output_root.resolve(), args.candidate_kind)


if __name__ == "__main__":
    raise SystemExit(main())
