#!/usr/bin/env python3
"""Score Grave-Shell Scarab in the bounded offline candidate worker."""

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
MATERIALIZER = Path(__file__).with_name("materialize_grave_shell_scarab_task.py")
TARGET = Path("varieties/magic/sets/ravnica_city_of_guilds/src/cards/grave_shell_scarab.rs")
AUTHORITY = Path("varieties/magic/session/src/bin/rav_grave_shell_scarab_pvp.rs")
IMAGE = "rust@sha256:d7482085ff5b415f84dba5647ae71606650bdef00db7aeb69f4b3d170c3e4082"
MAX_OUTPUT = 2 * 1024 * 1024


class WorkerFailure(RuntimeError):
    pass


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def verify_workspace(workspace: Path, manifest: Path) -> None:
    subprocess.run([sys.executable, str(MATERIALIZER), "--workspace", str(workspace), "--manifest", str(manifest), "--verify"], check=True, cwd=REPO)


def snapshot_workspace(workspace: Path, files: dict[str, str], destination: Path) -> Path:
    if destination.is_symlink():
        raise WorkerFailure("candidate_snapshot_path_is_symlink")
    if destination.exists():
        shutil.rmtree(destination)
    destination.mkdir(parents=True)
    for relative in sorted(files):
        source = workspace / Path(relative)
        if source.is_symlink() or not source.is_file():
            raise WorkerFailure(f"candidate_snapshot_source_missing: {relative}")
        target = destination / Path(relative)
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(source, target)
    return destination


def run_bounded(command: list[str], timeout: int, container: str) -> tuple[int, bytes, bytes]:
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
                raise WorkerFailure("candidate_timeout: offline authority exceeded its deadline")
            for key, _ in selector.select(0.2):
                chunk = os.read(key.fileobj.fileno(), 65536)
                if not chunk:
                    selector.unregister(key.fileobj)
                    continue
                (stdout if key.data == "stdout" else stderr).extend(chunk)
                if len(stdout) + len(stderr) > MAX_OUTPUT:
                    raise WorkerFailure("candidate_output_limit: worker output exceeded 2 MiB")
        return process.wait(), bytes(stdout), bytes(stderr)
    except BaseException:
        process.kill()
        process.wait()
        subprocess.run(["docker", "rm", "-f", container], capture_output=True, check=False)
        raise


def sandbox_command(workspace: Path, registry: Path, authority: Path, container: str, image: str, candidate_kind: str) -> list[str]:
    if authority.is_symlink() or not authority.is_file():
        raise WorkerFailure("trusted_grave_shell_scarab_authority_missing")
    candidate_flag = " --candidate" if candidate_kind == "agent" else ""
    return [
        "docker", "run", "--rm", "--name", container, "--network", "none", "--read-only",
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
        "cp /trusted-authority.rs /work/varieties/magic/session/src/bin/rav_grave_shell_scarab_pvp.rs && "
        "cd /work && cargo run --offline --locked -j 2 --manifest-path varieties/magic/Cargo.toml "
        "-p cardbench-magic-session --bin rav_grave_shell_scarab_pvp --" + candidate_flag,
    ]


def validate_trace(trace: object) -> None:
    if not isinstance(trace, dict) or trace.get("trace_id") != "rav-grave-shell-scarab-activated-draw-dredge-and-decline-full-game" or trace.get("passed") is not True:
        raise WorkerFailure("candidate_protocol_error: missing Grave-Shell Scarab PvP trace")
    if trace.get("terminal") != "Winner(PlayerId(0))" or trace.get("winner_seat") != 0 or trace.get("rejected_moves") != 0:
        raise WorkerFailure("candidate_protocol_error: Grave-Shell Scarab match did not end in a clean pilot win")
    if trace.get("deck_p0") != "4 Grave-Shell Scarab + 28 Forest + 28 Swamp (60 shuffled cards)" or trace.get("deck_p1") != "60 Forest (60 shuffled cards)":
        raise WorkerFailure("candidate_protocol_error: shuffled PvP deck contract changed")
    if trace.get("activation_mana_cost") != "{1}" or trace.get("dredge_count") != 1 or trace.get("normal_draw_after_decline") is not True:
        raise WorkerFailure("candidate_protocol_error: activation cost, Dredge 1, or decline draw is missing")
    if not isinstance(trace.get("dredge_decisions"), int) or trace["dredge_decisions"] < 2:
        raise WorkerFailure("candidate_protocol_error: the two ability draws did not reach Dredge decisions")
    if not isinstance(trace.get("battlefield_scarabs"), list) or not trace["battlefield_scarabs"]:
        raise WorkerFailure("candidate_protocol_error: terminal pilot win lacks a Scarab on the battlefield")
    if not isinstance(trace.get("life_totals"), list) or len(trace["life_totals"]) != 2 or trace["life_totals"][1] > 0:
        raise WorkerFailure("candidate_protocol_error: opponent life total did not reach zero")
    if not isinstance(trace.get("turns"), int) or not 0 < trace["turns"] <= 80 or not isinstance(trace.get("accepted_moves"), int) or not 0 < trace["accepted_moves"] <= 5000:
        raise WorkerFailure("candidate_protocol_error: bounded full-game receipts are missing")
    events = trace.get("events")
    if not isinstance(events, list) or len(events) < 20 or not isinstance(trace.get("event_count"), int) or trace["event_count"] < len(events):
        raise WorkerFailure("candidate_protocol_error: event trace is incomplete")
    event_text = "\n".join(str(event) for event in events)
    for required in ("AbilityManaPaid", "AbilityActivated", "SacrificedAsAbilityCost", "DecisionOpened", "DecisionCompleted", "Dredged", "CardMoved", "AbilityResolved", "SpellCast", "GameEnded"):
        if required not in event_text:
            raise WorkerFailure(f"candidate_protocol_error: required {required} receipt is absent")
    if "count: 1" not in event_text:
        raise WorkerFailure("candidate_protocol_error: exact Dredge 1 receipt is absent")


def validate_report(report: dict) -> None:
    expected_card = {"collector_number": 211, "printings": 1, "definition_id": "RAV-GRAVE-SHELL-SCARAB", "name": "Grave-Shell Scarab"}
    if (report.get("schema_version") != "cardbench.magic.card-pvp.check.v1" or report.get("task_id") != "cardbench/magic/card" or report.get("expansion") != "RAV" or report.get("passed") is not True or report.get("pilot_card_passed") is not True or report.get("replay_deterministic") is not True or report.get("cards_passed") != 1 or report.get("cards_credited") != 0 or report.get("cards_total") != 291 or report.get("printings_total") != 306 or report.get("coverage_complete") is not False or report.get("full_reward") is not False or report.get("harbor_reward") != 0.0 or report.get("pilot_reward") != 0.0 or report.get("viewer_scope") != "Judge" or report.get("card") != expected_card):
        raise WorkerFailure("candidate_protocol_error: incomplete Grave-Shell Scarab zero-credit report")
    guard = report.get("candidate_source_validation")
    if not isinstance(guard, dict) or guard.get("direct_literal_ast_checked_before_candidate_calls") is not True or guard.get("guard_ran_before_candidate_function_calls") is not True:
        raise WorkerFailure("candidate_protocol_error: direct-literal source guard is missing")
    traces = report.get("traces")
    if not isinstance(traces, list) or len(traces) != 1:
        raise WorkerFailure("candidate_protocol_error: exactly one full-game trace is required")
    validate_trace(traces[0])
    mutation = report.get("mutation_sensitivity")
    required = ("missing_dredge_mutation_detected", "wrong_dredge_count_mutation_detected", "wrong_mana_cost_mutation_detected", "wrong_creature_body_mutation_detected", "missing_activated_ability_mutation_detected", "wrong_activation_mana_cost_mutation_detected", "missing_source_sacrifice_mutation_detected", "wrong_activation_effect_mutation_detected")
    if not isinstance(mutation, dict) or any(mutation.get(key) is not True for key in required) or mutation.get("sealed_mutation_authority") is not False:
        raise WorkerFailure("candidate_protocol_error: semantic mutation controls are incomplete")
    breakdown = report.get("per_card_breakdown")
    if not isinstance(breakdown, list) or sum(1 for row in breakdown if isinstance(row, dict) and row.get("collector_number") == 211 and row.get("name") == "Grave-Shell Scarab" and row.get("definition_id") == "RAV-GRAVE-SHELL-SCARAB" and row.get("trace_status") == "passed") != 1:
        raise WorkerFailure("candidate_protocol_error: RAV #211 catalog diagnostic is missing")


def parse_report(stdout: bytes) -> dict:
    for line in reversed(stdout.decode(errors="replace").splitlines()):
        try:
            report = json.loads(line)
        except json.JSONDecodeError:
            continue
        if report.get("schema_version") == "cardbench.magic.card-pvp.check.v1":
            validate_report(report)
            return report
    raise WorkerFailure("candidate_protocol_error: no valid Grave-Shell Scarab report")


def score(workspace: Path, manifest: Path, output_root: Path, candidate_kind: str) -> int:
    output_root.mkdir(parents=True, exist_ok=True)
    data = json.loads(manifest.read_text(encoding="utf-8"))
    image = os.environ.get("CARDBENCH_MAGIC_WORKER_IMAGE", IMAGE)
    if not (image.startswith("rust@sha256:") or image.startswith("sha256:")):
        raise ValueError("CARDBENCH_MAGIC_WORKER_IMAGE must be an immutable digest reference")
    registry = Path.home() / ".cargo" / "registry"
    if not registry.is_dir():
        raise FileNotFoundError(f"offline Cargo registry cache is missing: {registry}")
    authority = REPO / AUTHORITY
    if authority.is_symlink() or not authority.is_file():
        raise FileNotFoundError(f"trusted Grave-Shell Scarab authority is missing: {authority}")
    if not shutil.which("docker"):
        raise FileNotFoundError("Docker is required for Magic candidate isolation")
    verify_workspace(workspace, manifest)
    snapshot = snapshot_workspace(workspace, data["files"], output_root / "candidate-workspace")
    digest = sha256((snapshot / TARGET).read_bytes())
    container = f"cardbench-magic-grave-shell-scarab-{uuid.uuid4().hex[:16]}"
    code, stdout, stderr = run_bounded(sandbox_command(snapshot, registry.resolve(), authority, container, image, candidate_kind), 900, container)
    (output_root / "worker.stdout").write_bytes(stdout)
    (output_root / "worker.stderr").write_bytes(stderr)
    if code != 0:
        result = {"schema_version":"cardbench.magic.card-pvp.check.v1","task_id":"cardbench/magic/card","candidate":{"kind":candidate_kind,"source_sha256":digest},"passed":False,"pilot_card_passed":False,"cards_passed":0,"cards_total":291,"printings_total":306,"cards_credited":0,"coverage_complete":False,"full_reward":False,"harbor_reward":0.0,"pilot_reward":0.0,"sealed_authority":False,"worker_image":image,"sandbox":{"network":"none","rootfs":"read-only","pids_limit":128},"error":"candidate compile or Grave-Shell Scarab trace failed","diagnostics":stderr.decode(errors="replace")[-12000:]}
        (output_root / "result.json").write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
        print(json.dumps(result))
        return 1
    report = parse_report(stdout)
    report["candidate"].update({"kind":candidate_kind,"source_sha256":digest})
    report.update({"sealed_authority":False,"worker_image":image,"sandbox":{"network":"none","rootfs":"read-only","pids_limit":128}})
    (output_root / "result.json").write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    print(json.dumps(report))
    return 0


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--workspace", required=True, type=Path)
    parser.add_argument("--manifest", required=True, type=Path)
    parser.add_argument("--output-root", required=True, type=Path)
    parser.add_argument("--candidate-kind", choices=("agent", "reference"), default="agent")
    args = parser.parse_args()
    return score(args.workspace.resolve(), args.manifest.resolve(), args.output_root.resolve(), args.candidate_kind)


if __name__ == "__main__":
    raise SystemExit(main())
