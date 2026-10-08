#!/usr/bin/env python3
"""Validate and score the isolated Dimir Aqueduct PvP diagnostic."""
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
MATERIALIZER = Path(__file__).with_name("materialize_dimir_aqueduct_task.py")
TARGET = Path("varieties/magic/sets/ravnica_city_of_guilds/src/cards/dimir_aqueduct.rs")
AUTHORITY = Path("varieties/magic/session/src/bin/rav_dimir_aqueduct_pvp.rs")
DEFAULT_WORKER_IMAGE = "rust@sha256:d7482085ff5b415f84dba5647ae71606650bdef00db7aeb69f4b3d170c3e4082"


class ScoringFailure(RuntimeError):
    pass


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def validate_report(report: dict) -> None:
    card = {"collector_number": 276, "definition_id": "RAV-DIMIR-AQUEDUCT", "name": "Dimir Aqueduct"}
    if (
        report.get("schema_version") != "cardbench.magic.card-pvp.check.v1"
        or report.get("task_id") != "cardbench/magic/card"
        or report.get("passed") is not True
        or report.get("pilot_card_passed") is not True
        or report.get("replay_deterministic") is not True
        or report.get("cards_passed") != 1
        or report.get("cards_total") != 291
        or report.get("printings_total") != 306
        or report.get("cards_credited") != 0
        or report.get("coverage_complete") is not False
        or report.get("full_reward") is not False
        or report.get("harbor_reward") != 0.0
        or report.get("pilot_reward") != 0.0
        or report.get("viewer_scope") != "Judge"
        or report.get("card") != card
        or report.get("rules_authority", {}).get("comprehensive_rules") != "2026-08-07"
    ):
        raise ScoringFailure("candidate_protocol_error: incomplete Dimir Aqueduct diagnostic or nonzero reward/credit")
    candidate = report.get("candidate")
    if not isinstance(candidate, dict) or candidate.get("card_definition") != card["definition_id"]:
        raise ScoringFailure("candidate_protocol_error: wrong Dimir Aqueduct identity")
    traces = report.get("traces")
    if not isinstance(traces, list) or len(traces) != 1:
        raise ScoringFailure("candidate_protocol_error: exactly one full-game Dimir trace is required")
    trace = traces[0]
    mechanics = trace.get("mechanics", {}) if isinstance(trace, dict) else {}
    if (
        trace.get("trace_id") != "rav-dimir-aqueduct-tapped-bounce-bundle-pvp"
        or trace.get("terminal") != "Winner(PlayerId(0))"
        or trace.get("winner_seat") != 0
        or trace.get("rejected_moves") != 0
        or mechanics.get("enters_tapped") is not True
        or mechanics.get("target_controller") != 0
        or mechanics.get("target_returns_to_hand") is not True
        or mechanics.get("target_definition") not in {"RAV-ISLAND", "RAV-SWAMP"}
        or mechanics.get("bundle") != ["Blue", "Black"]
        or mechanics.get("bundle_tapped_source") is not True
        or mechanics.get("mana_pool_at_activation") != [1, 1]
        or not isinstance(mechanics.get("guildmage_paid_colors"), list)
        or len(mechanics.get("guildmage_paid_colors")) != 2
        or set(mechanics.get("guildmage_paid_colors")) != {"Blue", "Black"}
        or mechanics.get("guildmage_attacked") is not True
        or mechanics.get("terminal_pilot_win") is not True
    ):
        raise ScoringFailure("candidate_protocol_error: tapped entry, controlled-land return, exact mana bundle, or terminal line is incomplete")
    events = trace.get("events")
    if not isinstance(events, list) or len(events) < 9 or any(
        not isinstance(event, dict)
        or not isinstance(event.get("sequence"), int)
        or not isinstance(event.get("kind"), str)
        or not isinstance(event.get("canonical"), str)
        for event in events
    ):
        raise ScoringFailure("candidate_protocol_error: malformed Dimir event projection")
    canonical = [event["canonical"] for event in events]
    sequences = [event["sequence"] for event in events]
    if sequences != sorted(sequences):
        raise ScoringFailure("candidate_protocol_error: Dimir event sequence is unstable")
    aqueduct = f"ObjectId({mechanics.get('land')})"
    target = f"ObjectId({mechanics.get('target')})"
    guildmage = f"ObjectId({mechanics.get('guildmage')})"
    required = [
        ("CardMoved", f"card: {aqueduct}, to: Battlefield"),
        ("TriggeredAbilityStacked", f"source: {aqueduct}, source_incarnation:"),
        ("CardMoved", f"card: {target}, to: Hand"),
        ("AbilityResolved", f"source: {aqueduct}, source_incarnation:"),
        ("BoundManaAbilityFreeBundleActivated", f"source: {aqueduct}, ability: \"dimir-aqueduct-ub\""),
        ("SpellManaPaid", f"player: PlayerId(0), card: {guildmage}"),
        ("SpellCast", guildmage),
        ("AttackersDeclared", guildmage),
        ("DamageDealtToPlayer", f"source: {guildmage}, player: PlayerId(1), amount: 2"),
        ("GameEnded", "winner: Some(PlayerId(0))"),
    ]
    cursor = 0
    for kind, fragment in required:
        position = next((i for i in range(cursor, len(canonical)) if kind in canonical[i] and fragment in canonical[i]), None)
        if position is None:
            raise ScoringFailure(f"candidate_protocol_error: missing ordered Dimir {kind} receipt")
        cursor = position + 1
    mutations = report.get("mutation_sensitivity", {})
    required_mutations = (
        "enters_tapped_removed_detected",
        "controlled_land_target_restriction_changed_detected",
        "return_trigger_removed_detected",
        "blue_black_mana_bundle_changed_detected",
    )
    if any(mutations.get(key) is not True for key in required_mutations) or mutations.get("sealed_mutation_authority") is not False:
        raise ScoringFailure("candidate_protocol_error: Dimir gameplay mutations must be detected")


def parse_report(stdout: bytes) -> dict:
    for line in reversed(stdout.decode(errors="replace").splitlines()):
        try:
            report = json.loads(line)
        except json.JSONDecodeError:
            continue
        if report.get("schema_version") == "cardbench.magic.card-pvp.check.v1":
            validate_report(report)
            return report
    raise ScoringFailure("candidate_protocol_error: no Dimir Aqueduct PvP report")


def write_result(root: Path, result: dict) -> None:
    root.mkdir(parents=True, exist_ok=True)
    (root / "result.json").write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
    (root / "reward.txt").write_text("0.00000000\n", encoding="utf-8")


def snapshot(workspace: Path, files: dict[str, str], destination: Path) -> Path:
    destination.mkdir(parents=True, exist_ok=True)
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
    name = f"cardbench-magic-dimir-aqueduct-{uuid.uuid4().hex[:12]}"
    flag = " --candidate" if candidate_kind == "agent" else ""
    command = [
        "docker", "run", "--rm", "--name", name, "--network", "none", "--read-only",
        "--cap-drop", "ALL", "--security-opt", "no-new-privileges", "--pids-limit", "128",
        "--memory", "3g", "--cpus", "2", "--user", "1000:1000",
        "--tmpfs", "/work:rw,exec,size=2147483648,uid=1000,gid=1000",
        "--tmpfs", "/tmp:rw,noexec,size=67108864,uid=1000,gid=1000",
        "--mount", f"type=bind,src={workspace},dst=/submission,readonly",
        "--mount", f"type=bind,src={registry.resolve()},dst=/usr/local/cargo/registry,readonly",
        "--mount", f"type=bind,src={authority.resolve()},dst=/trusted-authority.rs,readonly",
        "--env", "HOME=/tmp/home", "--env", "CARGO_HOME=/usr/local/cargo", "--env", "RUSTUP_HOME=/usr/local/rustup",
        "--env", "CARGO_NET_OFFLINE=true", "--env", "CARGO_BUILD_JOBS=2", "--env", "RAYON_NUM_THREADS=2",
        image, "sh", "-c",
        "cp -R /submission/. /work/ && cp /trusted-authority.rs /work/varieties/magic/session/src/bin/rav_dimir_aqueduct_pvp.rs && "
        "cd /work && cargo run --offline --locked -j 2 --manifest-path varieties/magic/Cargo.toml "
        "-p cardbench-magic-session --bin rav-dimir-aqueduct-pvp --" + flag,
    ]
    completed = subprocess.run(command, stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=timeout, check=False)
    return completed.returncode, completed.stdout, completed.stderr


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--workspace", type=Path)
    parser.add_argument("--manifest", type=Path)
    parser.add_argument("--output-root", required=True, type=Path)
    parser.add_argument("--candidate-kind", choices=("reference", "agent"), default="agent")
    parser.add_argument("--timeout", type=int, default=900)
    parser.add_argument("--report-file", type=Path, help="validate an existing authority JSON-lines capture")
    args = parser.parse_args()
    try:
        if args.report_file:
            report = parse_report(args.report_file.read_bytes())
            report["candidate_kind"] = report.get("candidate", {}).get("kind", args.candidate_kind)
            report["sealed_authority"] = False
            report["harbor_reward"] = 0.0
            report["pilot_reward"] = 0.0
            write_result(args.output_root, report)
            return 0
        if args.workspace is None or args.manifest is None:
            raise ValueError("--workspace and --manifest are required when --report-file is not provided")
        workspace = args.workspace.resolve()
        manifest_path = args.manifest.resolve()
        subprocess.run([sys.executable, str(MATERIALIZER), "--workspace", str(workspace), "--manifest", str(manifest_path), "--verify"], cwd=REPO, check=True, capture_output=True)
        manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
        image = os.environ.get("CARDBENCH_MAGIC_WORKER_IMAGE", DEFAULT_WORKER_IMAGE)
        if not (image.startswith("rust@sha256:") or image.startswith("sha256:")):
            raise ValueError("CARDBENCH_MAGIC_WORKER_IMAGE must be an immutable digest reference")
        with tempfile.TemporaryDirectory(prefix="dimir-aqueduct-candidate-") as temporary:
            candidate = snapshot(workspace, manifest["files"], Path(temporary) / "workspace")
            code, stdout, stderr = run_worker(candidate, (REPO / AUTHORITY).resolve(), image, args.candidate_kind, args.timeout)
        args.output_root.mkdir(parents=True, exist_ok=True)
        (args.output_root / "worker.stdout").write_bytes(stdout)
        (args.output_root / "worker.stderr").write_bytes(stderr)
        if code != 0:
            raise ScoringFailure(stderr.decode(errors="replace")[-12000:] or f"worker exited {code}")
        report = parse_report(stdout)
        report["candidate"] = {"kind": args.candidate_kind, "card_definition": "RAV-DIMIR-AQUEDUCT", "source_sha256": sha256((workspace / TARGET).read_bytes()), "materialized_workspace_sha256": manifest["workspace_sha256"]}
        report["candidate_kind"] = args.candidate_kind
        report["starting_workspace_sha256"] = manifest["workspace_sha256"]
        report["sealed_authority"] = False
        report["worker_image"] = image
        report["sandbox"] = {"network": "none", "rootfs": "read-only", "pids_limit": 128}
        report["harbor_reward"] = 0.0
        report["pilot_reward"] = 0.0
        write_result(args.output_root, report)
        return 0
    except Exception as error:
        write_result(args.output_root, {"schema_version": "cardbench.magic.card-pvp.check.v1", "task_id": "cardbench/magic/card", "passed": False, "pilot_card_passed": False, "cards_passed": 0, "cards_total": 291, "printings_total": 306, "cards_credited": 0, "coverage_complete": False, "full_reward": False, "harbor_reward": 0.0, "pilot_reward": 0.0, "sealed_authority": False, "error": f"{type(error).__name__}: {error}"})
        print(str(error), file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
