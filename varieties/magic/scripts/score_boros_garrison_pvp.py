#!/usr/bin/env python3
"""Validate the Boros Garrison diagnostic or score it in Harbor's isolated worker."""
from __future__ import annotations

import argparse
import hashlib
import json
import os
import shutil
import subprocess
import sys
import tempfile
import uuid
from pathlib import Path

REPO = Path(__file__).resolve().parents[3]
MATERIALIZER = Path(__file__).with_name("materialize_boros_garrison_task.py")
TARGET = Path("varieties/magic/sets/ravnica_city_of_guilds/src/cards/boros_garrison.rs")
AUTHORITY = Path("varieties/magic/session/src/bin/rav_boros_garrison_pvp.rs")
DEFAULT_WORKER_IMAGE = "rust@sha256:d7482085ff5b415f84dba5647ae71606650bdef00db7aeb69f4b3d170c3e4082"
EVENT_KINDS = [
    "CardMoved", "TriggeredAbilityStacked", "CardMoved", "AbilityResolved",
    "BoundManaAbilityFreeBundleActivated", "SpellManaPaid", "SpellCast",
    "AttackersDeclared", "DamageDealtToPlayer", "GameEnded",
]


class ScoringFailure(RuntimeError):
    pass


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def validate_report(report: dict) -> None:
    card = {"collector_number": 275, "definition_id": "RAV-BOROS-GARRISON", "name": "Boros Garrison"}
    if (
        report.get("schema_version") != "cardbench.magic.card-pvp.check.v1"
        or report.get("task_id") != "cardbench/magic/card"
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
        or report.get("card") != card
        or report.get("replay_deterministic") is not True
    ):
        raise ScoringFailure("candidate_protocol_error: incomplete Boros Garrison report or nonzero credit/reward")
    candidate = report.get("candidate")
    if not isinstance(candidate, dict) or candidate.get("card_definition") != card["definition_id"]:
        raise ScoringFailure("candidate_protocol_error: wrong Boros Garrison identity")
    traces = report.get("traces")
    if not isinstance(traces, list) or len(traces) != 1:
        raise ScoringFailure("candidate_protocol_error: exactly one full-game Boros Garrison trace is required")
    trace = traces[0]
    mechanics = trace.get("mechanics", {}) if isinstance(trace, dict) else {}
    if (
        trace.get("trace_id") != "rav-boros-garrison-tapped-bounce-bundle-pvp"
        or trace.get("terminal") != "Winner(PlayerId(0))"
        or trace.get("winner_seat") != 0
        or trace.get("rejected_moves") != 0
        or mechanics.get("enters_tapped") is not True
        or mechanics.get("target_controller") != 0
        or mechanics.get("target_owner") != 0
        or mechanics.get("target_definition") not in {"RAV-PLAINS", "RAV-MOUNTAIN"}
        or mechanics.get("target_returns_to_hand") is not True
        or mechanics.get("target_zone_when_returned") != "Some(Hand)"
        or mechanics.get("bundle") != ["Red", "White"]
        or mechanics.get("bundle_tapped_source") is not True
        or mechanics.get("mana_pool_at_activation") != [1, 1]
        or mechanics.get("skyknight_attacked") is not True
        or mechanics.get("skyknight_paid_red_white_and_generic") is not True
        or mechanics.get("terminal_pilot_win") is not True
    ):
        raise ScoringFailure("candidate_protocol_error: land entry, return trigger, exact bundle, payment, or terminal line is incomplete")
    if not isinstance(trace.get("turns"), int) or trace["turns"] < 1:
        raise ScoringFailure("candidate_protocol_error: missing terminal game turn count")
    land, target, skyknight = (mechanics.get(name) for name in ("land", "target", "skyknight"))
    if not all(isinstance(value, int) for value in (land, target, skyknight)) or len({land, target, skyknight}) != 3:
        raise ScoringFailure("candidate_protocol_error: Garrison, returned land, and Skyknight object ids differ")
    events = trace.get("events")
    if not isinstance(events, list) or len(events) != len(EVENT_KINDS):
        raise ScoringFailure("candidate_protocol_error: Boros Garrison ordered event sequence is incomplete")
    if any(not isinstance(event, dict) or not isinstance(event.get("sequence"), int) or not isinstance(event.get("kind"), str) or not isinstance(event.get("canonical"), str) for event in events):
        raise ScoringFailure("candidate_protocol_error: malformed projected Boros Garrison event")
    kinds = [event["kind"] for event in events]
    sequences = [event["sequence"] for event in events]
    if kinds != EVENT_KINDS or sequences != sorted(sequences) or trace.get("ordered_event_sequence") != sequences:
        raise ScoringFailure("candidate_protocol_error: Boros Garrison event order differs")
    canonical = [event["canonical"] for event in events]
    expected_fragments = [
        f"card: ObjectId({land}), to: Battlefield",
        f"source: ObjectId({land}), source_incarnation:",
        f"card: ObjectId({target}), to: Hand",
        f"source: ObjectId({land}), source_incarnation:",
        f'source: ObjectId({land}), ability: "boros-garrison-rw"',
        f"card: ObjectId({skyknight}), colors: [",
        f"card: ObjectId({skyknight})",
        f"ObjectId({skyknight})",
        f"source: ObjectId({skyknight}), player: PlayerId(1), amount: 2",
        "winner: Some(PlayerId(0))",
    ]
    if any(fragment not in event for fragment, event in zip(expected_fragments, canonical, strict=True)):
        raise ScoringFailure("candidate_protocol_error: Boros Garrison event object identity or semantics differ")
    payment = canonical[5]
    if "Red" not in payment or "White" not in payment:
        raise ScoringFailure("candidate_protocol_error: Skyknight payment omitted a required red or white mana")
    mutations = report.get("mutation_sensitivity", {})
    required = (
        "enters_tapped_removed_detected",
        "controlled_land_target_restriction_changed_detected",
        "return_trigger_removed_detected",
        "red_white_mana_bundle_changed_detected",
    )
    if any(mutations.get(key) is not True for key in required) or mutations.get("sealed_mutation_authority") is not False:
        raise ScoringFailure("candidate_protocol_error: Boros Garrison gameplay mutations are incomplete")


def verify_workspace(workspace: Path, manifest: Path) -> dict:
    subprocess.run(
        [sys.executable, str(MATERIALIZER), "--workspace", str(workspace), "--manifest", str(manifest), "--verify"],
        check=True, cwd=REPO, capture_output=True,
    )
    return json.loads(manifest.read_text(encoding="utf-8"))


def snapshot(workspace: Path, files: dict[str, str], destination: Path) -> Path:
    if destination.exists():
        shutil.rmtree(destination)
    destination.mkdir(parents=True)
    for relative in sorted(files):
        source = workspace / Path(relative)
        if source.is_symlink() or not source.is_file():
            raise ScoringFailure(f"candidate_snapshot_source_missing: {relative}")
        target = destination / Path(relative)
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(source, target)
    return destination


def run_worker(workspace: Path, authority: Path, image: str, candidate_kind: str, timeout: int) -> tuple[int, bytes, bytes]:
    registry = Path.home() / ".cargo" / "registry"
    if not registry.is_dir():
        raise FileNotFoundError(f"offline Cargo registry cache is missing: {registry}")
    if not shutil.which("docker"):
        raise FileNotFoundError("Docker is required for isolated Magic candidate scoring")
    if authority.is_symlink() or not authority.is_file():
        raise FileNotFoundError(f"trusted Boros Garrison authority is missing: {authority}")
    name = f"cardbench-magic-boros-garrison-{uuid.uuid4().hex[:12]}"
    flag = " --candidate" if candidate_kind == "agent" else ""
    command = [
        "docker", "run", "--rm", "--name", name, "--network", "none", "--read-only",
        "--cap-drop", "ALL", "--security-opt", "no-new-privileges", "--pids-limit", "128",
        "--memory", "3g", "--cpus", "2", "--user", "1000:1000",
        "--tmpfs", "/work:rw,exec,size=2147483648,uid=1000,gid=1000",
        "--tmpfs", "/tmp:rw,noexec,size=67108864,uid=1000,gid=1000",
        "--mount", f"type=bind,src={workspace.resolve()},dst=/submission,readonly",
        "--mount", f"type=bind,src={registry.resolve()},dst=/usr/local/cargo/registry,readonly",
        "--mount", f"type=bind,src={authority.resolve()},dst=/trusted-authority.rs,readonly",
        "--env", "HOME=/tmp/home", "--env", "CARGO_HOME=/usr/local/cargo", "--env", "RUSTUP_HOME=/usr/local/rustup",
        "--env", "CARGO_NET_OFFLINE=true", "--env", "CARGO_BUILD_JOBS=2", "--env", "RAYON_NUM_THREADS=2",
        image, "sh", "-c",
        "cp -R /submission/. /work/ && cp /trusted-authority.rs /work/varieties/magic/session/src/bin/rav_boros_garrison_pvp.rs && "
        "cd /work && cargo run --offline --locked -j 2 --manifest-path varieties/magic/Cargo.toml "
        "-p cardbench-magic-session --bin rav-boros-garrison-pvp --" + flag,
    ]
    completed = subprocess.run(command, stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=timeout, check=False)
    return completed.returncode, completed.stdout, completed.stderr


def parse_report(stdout: bytes) -> dict:
    for line in reversed(stdout.decode(errors="replace").splitlines()):
        try:
            report = json.loads(line)
        except json.JSONDecodeError:
            continue
        if report.get("schema_version") == "cardbench.magic.card-pvp.check.v1":
            validate_report(report)
            return report
    raise ScoringFailure("candidate_protocol_error: no Boros Garrison PvP JSON report")


def write_result(root: Path, result: dict) -> None:
    root.mkdir(parents=True, exist_ok=True)
    result["harbor_reward"] = 0.0
    result["pilot_reward"] = 0.0
    result["cards_credited"] = 0
    result["full_reward"] = False
    (root / "result.json").write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
    (root / "reward.txt").write_text("0.00000000\n", encoding="utf-8")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--workspace", type=Path)
    parser.add_argument("--manifest", type=Path)
    parser.add_argument("--output-root", type=Path)
    parser.add_argument("--candidate-kind", choices=("agent", "reference"), default="agent")
    parser.add_argument("--timeout", type=int, default=900)
    parser.add_argument("--report-file", type=Path, help="validate an existing local authority capture without Docker")
    args = parser.parse_args()
    if args.report_file:
        try:
            validate_report(json.loads(args.report_file.read_text(encoding="utf-8")))
            print("local_reference_report=valid cards=1/291 credited=0 pilot_reward=0.0 harbor_reward=0.0")
            return 0
        except Exception as error:
            print(f"Boros Garrison report validation failed: {error}", file=sys.stderr)
            return 1
    if not args.workspace or not args.manifest or not args.output_root:
        parser.error("--workspace, --manifest, and --output-root are required unless --report-file is used")
    output = args.output_root.resolve()
    try:
        workspace, manifest_path = args.workspace.resolve(), args.manifest.resolve()
        manifest = verify_workspace(workspace, manifest_path)
        image = os.environ.get("CARDBENCH_MAGIC_WORKER_IMAGE", DEFAULT_WORKER_IMAGE)
        if not (image.startswith("rust@sha256:") or image.startswith("sha256:")):
            raise ValueError("CARDBENCH_MAGIC_WORKER_IMAGE must be an immutable digest reference")
        with tempfile.TemporaryDirectory(prefix="boros-garrison-candidate-") as temporary:
            candidate = snapshot(workspace, manifest["files"], Path(temporary) / "workspace")
            code, stdout, stderr = run_worker(candidate, (REPO / AUTHORITY).resolve(), image, args.candidate_kind, args.timeout)
        output.mkdir(parents=True, exist_ok=True)
        (output / "worker.stdout").write_bytes(stdout)
        (output / "worker.stderr").write_bytes(stderr)
        if code != 0:
            raise ScoringFailure(stderr.decode(errors="replace")[-12000:] or f"worker exited {code}")
        report = parse_report(stdout)
        report["candidate"] = {
            "kind": args.candidate_kind,
            "card_definition": "RAV-BOROS-GARRISON",
            "source_sha256": sha256((candidate / TARGET).read_bytes()),
            "materialized_workspace_sha256": manifest["workspace_sha256"],
        }
        report["candidate_kind"] = args.candidate_kind
        report["starting_workspace_sha256"] = manifest["workspace_sha256"]
        report["sealed_authority"] = False
        report["worker_image"] = image
        report["sandbox"] = {"network": "none", "rootfs": "read-only", "pids_limit": 128}
        write_result(output, report)
        print(f"candidate_passed={report['pilot_card_passed']} cards={report['cards_passed']}/291 credit=0 reward=0.0")
        return 0
    except Exception as error:
        write_result(output, {
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
            "pilot_reward": 0.0,
            "sealed_authority": False,
            "error": f"{type(error).__name__}: {error}",
        })
        print(f"Boros Garrison candidate verifier failed: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
