#!/usr/bin/env python3
"""Validate and score the isolated Disembowel PvP diagnostic."""
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
MATERIALIZER = Path(__file__).with_name("materialize_disembowel_task.py")
TARGET = Path("varieties/magic/sets/ravnica_city_of_guilds/src/cards/disembowel.rs")
AUTHORITY = Path("varieties/magic/session/src/bin/rav_disembowel_pvp.rs")
DEFAULT_WORKER_IMAGE = "rust@sha256:d7482085ff5b415f84dba5647ae71606650bdef00db7aeb69f4b3d170c3e4082"


class ScoringFailure(RuntimeError):
    pass


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def validate_report(report: dict) -> None:
    expected = {"collector_number": 85, "definition_id": "RAV-DISEMBOWEL", "name": "Disembowel"}
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
        or report.get("card") != expected
        or report.get("rules_authority", {}).get("comprehensive_rules") != "2026-08-07"
    ):
        raise ScoringFailure("candidate_protocol_error: incomplete Disembowel diagnostic report")
    candidate = report.get("candidate")
    if not isinstance(candidate, dict) or candidate.get("card_definition") != expected["definition_id"]:
        raise ScoringFailure("candidate_protocol_error: wrong Disembowel identity")
    traces = report.get("traces")
    if not isinstance(traces, list) or len(traces) != 1:
        raise ScoringFailure("candidate_protocol_error: one Disembowel full-game trace is required")
    trace = traces[0]
    mechanics = trace.get("mechanics", {}) if isinstance(trace, dict) else {}
    if (
        trace.get("trace_id") != "rav-disembowel-exact-chosen-x-mana-value-pvp"
        or trace.get("terminal") != "Winner(PlayerId(0))"
        or trace.get("rejected_moves") != 0
        or mechanics.get("mana_cost") != "{X}{B}"
        or mechanics.get("chosen_x") != 1
        or mechanics.get("target_mana_value") != 1
        or mechanics.get("paid_colors") != ["Black", "Black"]
        or mechanics.get("target_definition") != "RAV-WAR-TORCH-GOBLIN"
        or mechanics.get("exact_mana_value_match") is not True
        or mechanics.get("target_destroyed") is not True
        or mechanics.get("target_final_zone") != "Graveyard"
        or mechanics.get("spell_final_zone") != "Graveyard"
        or mechanics.get("controller_won") is not True
    ):
        raise ScoringFailure("candidate_protocol_error: exact-X target, payment, destruction, or terminal trace is incomplete")
    events = trace.get("events")
    if not isinstance(events, list) or len(events) < 8 or any(
        not isinstance(item, dict)
        or not isinstance(item.get("sequence"), int)
        or not isinstance(item.get("kind"), str)
        or not isinstance(item.get("canonical"), str)
        for item in events
    ):
        raise ScoringFailure("candidate_protocol_error: malformed Disembowel event projection")
    canonical = [item["canonical"] for item in events]
    if [item["sequence"] for item in events] != sorted(item["sequence"] for item in events):
        raise ScoringFailure("candidate_protocol_error: projected event order is unstable")
    spell = f"ObjectId({mechanics.get('spell')})"
    target = f"ObjectId({mechanics.get('target')})"
    required = [
        ("SpellManaPaid", spell),
        ("SpellCast", spell),
        ("CardDestroyed", f"{spell}, card: {target}"),
        ("CardMoved", f"{target}, to: Graveyard"),
        ("SpellResolved", spell),
        ("GameEnded", "winner: Some(PlayerId(0))"),
    ]
    indices = []
    for kind, fragment in required:
        index = next((i for i, item in enumerate(canonical) if kind in item and fragment in item), None)
        if index is None:
            raise ScoringFailure(f"candidate_protocol_error: missing {kind} receipt for Disembowel")
        indices.append(index)
    if indices != sorted(indices):
        raise ScoringFailure("candidate_protocol_error: Disembowel lifecycle event order is invalid")
    mutation = report.get("mutation_sensitivity", {})
    required_mutations = (
        "destruction_instruction_removed_detected",
        "mana_value_at_most_replaces_exact_equality_detected",
        "printed_cost_changed_detected",
    )
    if any(mutation.get(key) is not True for key in required_mutations) or mutation.get("sealed_mutation_authority") is not False:
        raise ScoringFailure("candidate_protocol_error: Disembowel gameplay mutations must be detected")


def parse_report(stdout: bytes) -> dict:
    for line in reversed(stdout.decode(errors="replace").splitlines()):
        try:
            report = json.loads(line)
        except json.JSONDecodeError:
            continue
        if report.get("schema_version") == "cardbench.magic.card-pvp.check.v1":
            validate_report(report)
            return report
    raise ScoringFailure("candidate_protocol_error: no Disembowel PvP report")


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
    name = f"cardbench-magic-disembowel-{uuid.uuid4().hex[:12]}"
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
        "cp -R /submission/. /work/ && cp /trusted-authority.rs /work/varieties/magic/session/src/bin/rav_disembowel_pvp.rs && "
        "cd /work && cargo run --offline --locked -j 2 --manifest-path varieties/magic/Cargo.toml "
        "-p cardbench-magic-session --bin rav-disembowel-pvp --" + flag,
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
        result = subprocess.run([sys.executable, str(MATERIALIZER), "--workspace", str(workspace), "--manifest", str(manifest_path), "--verify"], cwd=REPO, check=True, capture_output=True)
        manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
        image = os.environ.get("CARDBENCH_MAGIC_WORKER_IMAGE", DEFAULT_WORKER_IMAGE)
        if not (image.startswith("rust@sha256:") or image.startswith("sha256:")):
            raise ValueError("CARDBENCH_MAGIC_WORKER_IMAGE must be an immutable digest reference")
        with tempfile.TemporaryDirectory(prefix="disembowel-candidate-") as temporary:
            candidate = snapshot(workspace, manifest["files"], Path(temporary) / "workspace")
            code, stdout, stderr = run_worker(candidate, (REPO / AUTHORITY).resolve(), image, args.candidate_kind, args.timeout)
        args.output_root.mkdir(parents=True, exist_ok=True)
        (args.output_root / "worker.stdout").write_bytes(stdout)
        (args.output_root / "worker.stderr").write_bytes(stderr)
        if code != 0:
            raise ScoringFailure(stderr.decode(errors="replace")[-12000:] or f"worker exited {code}")
        report = parse_report(stdout)
        report["candidate"] = {"kind": args.candidate_kind, "card_definition": "RAV-DISEMBOWEL", "source_sha256": sha256((workspace / TARGET).read_bytes()), "materialized_workspace_sha256": manifest["workspace_sha256"]}
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
        write_result(args.output_root, {"schema_version":"cardbench.magic.card-pvp.check.v1", "task_id":"cardbench/magic/card", "passed":False, "pilot_card_passed":False, "cards_passed":0, "cards_total":291, "printings_total":306, "cards_credited":0, "coverage_complete":False, "full_reward":False, "harbor_reward":0.0, "pilot_reward":0.0, "sealed_authority":False, "error":f"{type(error).__name__}: {error}"})
        print(str(error), file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
