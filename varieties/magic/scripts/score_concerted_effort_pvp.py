#!/usr/bin/env python3
"""Validate and optionally score an isolated Concerted Effort submission."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import shutil
import subprocess
import uuid
from pathlib import Path

from materialize_concerted_effort_task import verify

REPO = Path(__file__).resolve().parents[3]
TARGET = Path("varieties/magic/sets/ravnica_city_of_guilds/src/cards/concerted_effort.rs")
AUTHORITY = Path("varieties/magic/session/src/bin/rav_concerted_effort_pvp.rs")
BIN = "rav-concerted-effort-pvp"
DEFAULT_IMAGE = "rust@sha256:d7482085ff5b415f84dba5647ae71606650bdef00db7aeb69f4b3d170c3e4082"


class WorkerFailure(RuntimeError):
    pass


def validate_report(report: dict) -> None:
    if (
        report.get("schema_version") != "cardbench.magic.card-pvp.check.v1"
        or report.get("task_id") != "cardbench/magic/card"
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
        or report.get("card") != {"collector_number": 8, "definition_id": "RAV-CONCERTED-EFFORT", "name": "Concerted Effort"}
    ):
        raise WorkerFailure("candidate_protocol_error: incomplete Concerted Effort single-card report")
    traces = report.get("traces")
    if not isinstance(traces, list) or len(traces) != 1 or not isinstance(traces[0], dict):
        raise WorkerFailure("candidate_protocol_error: exactly one Concerted Effort trace is required")
    trace = traces[0]
    if trace.get("id") != "rav-concerted-effort-upkeep-keyword-sharing" or trace.get("passed") is not True or not isinstance(trace.get("seed"), int) or trace["seed"] < 0 or trace.get("winner_seat") != 0 or trace.get("terminal") != "Winner(PlayerId(0))" or trace.get("rejected_moves") != 0 or not isinstance(trace.get("turns"), int) or trace["turns"] <= 0 or not isinstance(trace.get("accepted_moves"), int) or trace["accepted_moves"] <= 0:
        raise WorkerFailure("candidate_protocol_error: Concerted Effort trace must be a clean terminal player-zero win")
    card, recipient = trace.get("concerted_effort"), trace.get("recipient")
    if not isinstance(card, dict) or card.get("mana_cost") != "{2}{W}{W}" or card.get("trigger_id") != "upkeep-share-controller-creature-keywords" or not isinstance(card.get("trigger_resolutions"), int) or card["trigger_resolutions"] < 1:
        raise WorkerFailure("candidate_protocol_error: Concerted Effort cast or upkeep trigger is missing")
    if not isinstance(recipient, dict) or recipient.get("definition_id") != "RAV-WATCHWOLF" or recipient.get("received_trample_from_other_creature") is not True:
        raise WorkerFailure("candidate_protocol_error: shared-keyword recipient or separate Trample provider is missing")
    keywords = recipient.get("shared_keywords")
    required = ("Flying", "FirstStrike", "DoubleStrike", "Landwalk(Island)", "Trample")
    if not isinstance(keywords, list) or any(not any(token in str(keyword) for keyword in keywords) for token in required):
        raise WorkerFailure("candidate_protocol_error: full-game Watchwolf snapshot lacks a required shared keyword")
    events = trace.get("events")
    kinds = [event.get("kind") for event in events if isinstance(event, dict)] if isinstance(events, list) else []
    required_order = ["SpellCast", "SpellResolved", "TriggeredAbilityStacked", "AbilityResolved", "AttackersDeclared", "DamageDealtToPlayer", "GameEnded"]
    cursor = iter(kinds)
    if not all(any(kind == expected for kind in cursor) for expected in required_order):
        raise WorkerFailure("candidate_protocol_error: cast, trigger, combat, and terminal receipts are out of order")
    mutation = report.get("mutation_sensitivity", {})
    for key in ("upkeep_trigger_removed_mutation_detected_in_game", "shared_flying_family_removed_mutation_detected_in_game", "shared_family_scope_mutation_detected_in_game"):
        if not isinstance(mutation, dict) or mutation.get(key) is not True:
            raise WorkerFailure("candidate_protocol_error: Concerted Effort trigger and family mutations must be detected")


def run_worker(workspace: Path, registry: Path, authority: Path, name: str, image: str, candidate_kind: str) -> subprocess.CompletedProcess[bytes]:
    flag = " --candidate" if candidate_kind == "agent" else ""
    command = [
        "docker", "run", "--rm", "--name", name, "--network", "none", "--read-only",
        "--cap-drop", "ALL", "--security-opt", "no-new-privileges", "--pids-limit", "128",
        "--memory", "3g", "--cpus", "2", "--user", "1000:1000",
        "--tmpfs", "/work:rw,exec,size=2147483648,uid=1000,gid=1000",
        "--tmpfs", "/tmp:rw,noexec,size=67108864,uid=1000,gid=1000",
        "--mount", f"type=bind,src={workspace},dst=/submission,readonly",
        "--mount", f"type=bind,src={registry},dst=/usr/local/cargo/registry,readonly",
        "--mount", f"type=bind,src={authority.resolve()},dst=/trusted-authority.rs,readonly",
        "--env", "HOME=/tmp/home", "--env", "CARGO_HOME=/usr/local/cargo", "--env", "RUSTUP_HOME=/usr/local/rustup",
        "--env", "CARGO_NET_OFFLINE=true", "--env", "CARGO_BUILD_JOBS=2", "--env", "RAYON_NUM_THREADS=2",
        image, "sh", "-c", "cp -R /submission/. /work/ && cp /trusted-authority.rs /work/varieties/magic/session/src/bin/rav_concerted_effort_pvp.rs && cd /work && cargo run --offline --locked -j 2 --manifest-path varieties/magic/Cargo.toml -p cardbench-magic-session --bin rav-concerted-effort-pvp --" + flag,
    ]
    return subprocess.run(command, capture_output=True, timeout=900, check=False)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--validate-report", type=Path)
    parser.add_argument("--workspace", type=Path)
    parser.add_argument("--manifest", type=Path)
    parser.add_argument("--output-root", type=Path)
    parser.add_argument("--candidate-kind", choices=["agent", "reference"], default="agent")
    args = parser.parse_args()
    if args.validate_report:
        validate_report(json.loads(args.validate_report.read_text(encoding="utf-8")))
        print("Concerted Effort report validates; rewards and credit remain zero")
        return 0
    if not args.workspace or not args.manifest or not args.output_root:
        parser.error("--workspace, --manifest, and --output-root are required unless --validate-report is used")
    output = args.output_root.resolve()
    output.mkdir(parents=True, exist_ok=True)
    manifest = json.loads(args.manifest.read_text(encoding="utf-8"))
    image = os.environ.get("CARDBENCH_MAGIC_WORKER_IMAGE", DEFAULT_IMAGE)
    if not (image.startswith("rust@sha256:") or image.startswith("sha256:")):
        raise ValueError("CARDBENCH_MAGIC_WORKER_IMAGE must be an immutable digest reference")
    registry = Path.home() / ".cargo" / "registry"
    authority = REPO / AUTHORITY
    if not registry.is_dir() or not authority.is_file() or authority.is_symlink() or not shutil.which("docker"):
        raise FileNotFoundError("offline registry, trusted authority, or Docker is unavailable")
    verify(args.workspace.resolve(), args.manifest.resolve())
    candidate_workspace = output / "candidate-workspace"
    if candidate_workspace.exists():
        shutil.rmtree(candidate_workspace)
    for relative in sorted(manifest["files"]):
        source = args.workspace.resolve() / Path(relative)
        destination = candidate_workspace / Path(relative)
        if source.is_symlink() or not source.is_file():
            raise WorkerFailure(f"candidate_workspace_source_missing: {relative}")
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(source, destination)
    source_hash = hashlib.sha256((candidate_workspace / TARGET).read_bytes()).hexdigest()
    name = f"cardbench-magic-concerted-effort-{uuid.uuid4().hex[:16]}"
    process = run_worker(candidate_workspace, registry.resolve(), authority, name, image, args.candidate_kind)
    (output / "worker.stdout").write_bytes(process.stdout)
    (output / "worker.stderr").write_bytes(process.stderr)
    if process.returncode != 0:
        raise WorkerFailure("candidate_compile_or_reference_failed: " + process.stderr.decode(errors="replace")[-12000:])
    report = None
    for line in reversed(process.stdout.decode(errors="replace").splitlines()):
        try:
            value = json.loads(line)
        except json.JSONDecodeError:
            continue
        if isinstance(value, dict) and value.get("schema_version") == "cardbench.magic.card-pvp.check.v1":
            report = value
            break
    if report is None:
        raise WorkerFailure("candidate_protocol_error: no Concerted Effort PvP report")
    validate_report(report)
    report["candidate"] = {"kind": args.candidate_kind, "card_definition": "RAV-CONCERTED-EFFORT", "source_sha256": source_hash, "materialized_workspace_sha256": manifest["workspace_sha256"]}
    report["candidate_kind"] = args.candidate_kind
    report["sealed_authority"] = False
    report["worker_image"] = image
    report["sandbox"] = {"network": "none", "rootfs": "read-only", "pids_limit": 128}
    report["harbor_reward"] = 0.0
    report["pilot_reward"] = 0.0
    (output / "result.json").write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    (output / "reward.txt").write_text("0.00000000\n", encoding="utf-8")
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except Exception as error:
        raise SystemExit(f"Concerted Effort scoring failed: {error}")
