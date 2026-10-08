#!/usr/bin/env python3
"""Run the isolated Bathe in Light reference or candidate PvP pilot."""

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
MATERIALIZER = Path(__file__).with_name("materialize_bathe_in_light_task.py")
TARGET = Path("varieties/magic/sets/ravnica_city_of_guilds/src/cards/bathe_in_light.rs")
AUTHORITY_SOURCE = Path("varieties/magic/session/src/bin/rav_bathe_in_light_pvp.rs")
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
    output = bytearray()
    diagnostics = bytearray()
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
                destination = output if key.data == "stdout" else diagnostics
                destination.extend(chunk)
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
        "cp /trusted-authority.rs /work/varieties/magic/session/src/bin/rav_bathe_in_light_pvp.rs && "
        "cd /work && cargo run --offline --locked -j 2 --manifest-path varieties/magic/Cargo.toml "
        "-p cardbench-magic-session --bin rav-bathe-in-light-pvp --" + candidate_flag,
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
    raise WorkerFailure("candidate_protocol_error: no valid Bathe in Light PvP result")


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
        or report.get("card") != {"collector_number": 2, "printings": 1, "definition_id": "RAV-BATHE-IN-LIGHT", "name": "Bathe in Light"}
    ):
        raise WorkerFailure("candidate_protocol_error: incomplete Bathe in Light single-card report")
    candidate = report.get("candidate")
    if not isinstance(candidate, dict) or candidate.get("card_definition") != "RAV-BATHE-IN-LIGHT":
        raise WorkerFailure("candidate_protocol_error: wrong Bathe in Light identity")
    traces = report.get("traces")
    if not isinstance(traces, list) or len(traces) != 1:
        raise WorkerFailure("candidate_protocol_error: one full-game Bathe in Light Radiance trace is required")
    trace = traces[0]
    bathe = trace.get("bathe_in_light", {})
    radiance = trace.get("radiance", {})
    legality = trace.get("protection_target_legality", {})
    recipients = radiance.get("expected_recipient_objects")
    created = radiance.get("continuous_effect_recipient_objects")
    excluded = radiance.get("excluded_nonmatching_creatures")
    creatures = radiance.get("creature_results")
    excluded_ids = {item.get("object") for item in excluded if isinstance(item, dict)} if isinstance(excluded, list) else set()
    protected_ids = {
        item.get("object")
        for item in creatures
        if isinstance(item, dict) and item.get("has_protection_from_chosen_red") is True
    } if isinstance(creatures, list) else set()
    target_id = bathe.get("target_object")
    if (
        trace.get("trace_id") != "rav-bathe-in-light-targeted-radiance-cross-controller-full-game"
        or trace.get("passed") is not True
        or trace.get("terminal") != "Winner(PlayerId(0))"
        or trace.get("winner_seat") != 0
        or trace.get("rejected_moves") != 0
        or bathe.get("collector_number") != 2
        or bathe.get("mana_cost") != "{1}{W}"
        or len(bathe.get("mana_paid", [])) != 2
        or "White" not in bathe.get("mana_paid", [])
        or bathe.get("mana_payment_source") != "before_after_mana_pool_delta"
        or bathe.get("chosen_color") != "Red"
        or bathe.get("target_controller") != 1
        or not isinstance(recipients, list)
        or not isinstance(bathe.get("target_colors"), list)
        or target_id not in recipients
        or not {"Green", "White"} <= set(bathe.get("target_colors", []))
        or bathe.get("zone_after_resolution") != "Graveyard"
        or len(recipients) < 2
        or len(set(recipients)) != len(recipients)
        or not isinstance(created, list)
        or set(created) != set(recipients)
        or len(created) != len(recipients)
        or not set(recipients) <= protected_ids
        or set(excluded_ids) & protected_ids
        or radiance.get("matching_recipients_cross_both_controllers") is not True
        or radiance.get("cross_controller_nonmatching_blue_excluded") is not True
        or radiance.get("colorless_creature_excluded") is not True
        or not isinstance(excluded, list)
        or not {item.get("definition") for item in excluded if isinstance(item, dict)} >= {"RAV-GLASS-GOLEM", "RAV-SNAPPING-DRAKE"}
        or not any(item.get("definition") == "RAV-SNAPPING-DRAKE" and item.get("colors") == ["Blue"] for item in excluded if isinstance(item, dict))
        or not any(item.get("definition") == "RAV-GLASS-GOLEM" and item.get("colors") == [] for item in excluded if isinstance(item, dict))
        or not isinstance(creatures, list)
        or not any(item.get("controller") == 0 and item.get("object") in recipients and item.get("has_protection_from_chosen_red") is True for item in creatures if isinstance(item, dict))
        or not any(item.get("controller") == 1 and item.get("object") in recipients and item.get("has_protection_from_chosen_red") is True for item in creatures if isinstance(item, dict))
        or not any(item.get("definition") == "RAV-SNAPPING-DRAKE" and item.get("object") in excluded_ids and item.get("object") not in recipients for item in excluded if isinstance(item, dict))
        or not any(item.get("definition") == "RAV-GLASS-GOLEM" and item.get("object") in excluded_ids and item.get("object") not in recipients for item in excluded if isinstance(item, dict))
        or not any(item.get("object") in excluded_ids and item.get("has_protection_from_chosen_red") is False for item in creatures if isinstance(item, dict))
        or legality.get("protected_watchwolf_target_rejected") is not True
        or legality.get("rejection_was_atomic") is not True
        or legality.get("nonmatching_blue_drake_target_accepted") is not True
        or not isinstance(legality.get("challenge_spell_object"), int)
        or not isinstance(legality.get("legal_target"), int)
        or legality.get("legal_target") not in excluded_ids
        or trace.get("combat", {}).get("total_damage_to_opponent", 0) < 20
    ):
        raise WorkerFailure("candidate_protocol_error: Bathe in Light trace did not satisfy the targeted cross-controller Radiance contract")
    events = trace.get("events")
    if not isinstance(events, list) or len(events) < 14:
        raise WorkerFailure("candidate_protocol_error: Bathe in Light trace is missing full-game events")
    sequences = [event.get("sequence") for event in events if isinstance(event, dict)]
    if len(sequences) != len(events) or any(not isinstance(value, int) for value in sequences) or sequences != sorted(sequences):
        raise WorkerFailure("candidate_protocol_error: event sequence is malformed")
    mutations = report.get("mutation_sensitivity")
    required = (
        "wrong_exact_mana_cost_detected",
        "wrong_card_color_detected",
        "wrong_target_requirement_detected",
        "controller_only_protection_mutation_detected",
    )
    if not isinstance(mutations, dict) or any(mutations.get(key) is not True for key in required) or mutations.get("sealed_mutation_authority") is not False:
        raise WorkerFailure("candidate_protocol_error: Bathe in Light semantic mutation gates are incomplete")


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
        raise FileNotFoundError(f"trusted Bathe in Light authority is missing: {authority}")
    if not shutil.which("docker"):
        raise FileNotFoundError("Docker is required for Magic candidate isolation")
    verify_workspace(workspace, manifest)
    snapshot = make_scoring_snapshot(workspace, manifest_data["files"], output_root / "candidate-workspace")
    candidate_hash = sha256((snapshot / TARGET).read_bytes())
    name = f"cardbench-magic-bathe-in-light-{uuid.uuid4().hex[:16]}"
    command = sandbox_command(snapshot, cargo_registry.resolve(), authority, name, worker_image, candidate_kind)
    code, stdout, stderr = run_bounded(command, timeout=900, name=name)
    (output_root / "worker.stdout").write_bytes(stdout)
    (output_root / "worker.stderr").write_bytes(stderr)
    if code != 0:
        result = {"schema_version":"cardbench.magic.card-pvp.check.v1","task_id":"cardbench/magic/card","candidate":{"kind":candidate_kind,"source_sha256":candidate_hash},"passed":False,"pilot_card_passed":False,"cards_passed":0,"cards_total":291,"printings_total":306,"cards_credited":0,"coverage_complete":False,"full_reward":False,"harbor_reward":0.0,"pilot_reward":0.0,"sealed_authority":False,"worker_image":worker_image,"sandbox":{"network":"none","rootfs":"read-only","pids_limit":128},"error":"candidate compile or Bathe in Light reference trace failed","diagnostics":stderr.decode(errors="replace")[-12000:]}
    else:
        result = parse_report(stdout)
        result["candidate"] = {"kind":candidate_kind,"card_definition":"RAV-BATHE-IN-LIGHT","source_sha256":candidate_hash,"materialized_workspace_sha256":manifest_data["workspace_sha256"]}
        result["candidate_kind"] = candidate_kind
        result["starting_workspace_sha256"] = manifest_data["workspace_sha256"]
        result["sealed_authority"] = False
        result["worker_image"] = worker_image
        result["sandbox"] = {"network":"none","rootfs":"read-only","pids_limit":128}
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
        result = {"schema_version":"cardbench.magic.card-pvp.check.v1","task_id":"cardbench/magic/card","passed":False,"pilot_card_passed":False,"cards_passed":0,"cards_total":291,"printings_total":306,"cards_credited":0,"coverage_complete":False,"full_reward":False,"harbor_reward":0.0,"sealed_authority":False,"error":str(error)}
        (args.output_root / "result.json").write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
        (args.output_root / "reward.txt").write_text("0.00000000\n", encoding="utf-8")
        print(f"Magic candidate verifier failed: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
