#!/usr/bin/env python3
"""Run the Hunted Troll diagnostic reference in an isolated offline worker."""

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
MATERIALIZER = Path(__file__).with_name("materialize_hunted_troll_task.py")
TARGET = Path("varieties/magic/sets/ravnica_city_of_guilds/src/cards/hunted_troll.rs")
AUTHORITY_SOURCE = Path("varieties/magic/session/src/bin/rav_hunted_troll_pvp.rs")
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
        "cp /trusted-authority.rs /work/varieties/magic/session/src/bin/rav_hunted_troll_pvp.rs && "
        "cd /work && cargo run --offline --locked -j 2 --manifest-path varieties/magic/Cargo.toml "
        "-p cardbench-magic-session --bin rav-hunted-troll-pvp --" + candidate_flag,
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
    raise WorkerFailure("candidate_protocol_error: no valid Hunted Troll PvP result")


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
        or report.get("card") != {"collector_number": 170, "definition_id": "RAV-HUNTED-TROLL", "name": "Hunted Troll"}
    ):
        raise WorkerFailure("candidate_protocol_error: incomplete Hunted Troll single-card result")
    candidate = report.get("candidate")
    if not isinstance(candidate, dict) or candidate.get("card_definition") != "RAV-HUNTED-TROLL":
        raise WorkerFailure("candidate_protocol_error: wrong Hunted Troll identity")
    traces = report.get("traces")
    if not isinstance(traces, list) or len(traces) != 2:
        raise WorkerFailure("candidate_protocol_error: two focused Hunted Troll full-game traces are required")
    validate_trace(traces[0], regeneration=False)
    validate_trace(traces[1], regeneration=True)
    mutation = report.get("mutation_sensitivity", {})
    required = (
        "body_mutation_detected_in_game",
        "etb_trigger_removal_detected_in_game",
        "wrong_target_shape_detected_in_game",
        "wrong_token_detected_in_game",
        "regeneration_cost_mutation_detected_in_game",
    )
    if not isinstance(mutation, dict) or any(mutation.get(key) is not True for key in required):
        raise WorkerFailure("candidate_protocol_error: Hunted Troll semantic controls are incomplete")


def validate_trace(trace: object, *, regeneration: bool) -> None:
    trace_id = "rav-hunted-troll-targeted-etb-and-regeneration" if regeneration else "rav-hunted-troll-targeted-etb-faeries"
    if not isinstance(trace, dict) or trace.get("id") != trace_id or trace.get("passed") is not True or trace.get("terminal") != "Winner(PlayerId(0))" or trace.get("winner_seat") != 0 or trace.get("rejected_moves") != 0:
        raise WorkerFailure("candidate_protocol_error: Hunted Troll trace did not reach a clean terminal result")
    if trace.get("troll_characteristics") != {"power": 8, "toughness": 4}:
        raise WorkerFailure("candidate_protocol_error: resolved Hunted Troll is not a live 8/4")
    etb = trace.get("etb")
    if not isinstance(etb, dict) or etb.get("target_seat") != 1 or not isinstance(etb.get("source_incarnation"), int) or etb.get("resolved_incarnation") != etb.get("source_incarnation"):
        raise WorkerFailure("candidate_protocol_error: Hunted Troll ETB target or incarnation differs")
    tokens = etb.get("tokens")
    if not isinstance(tokens, list) or len(tokens) != 4:
        raise WorkerFailure("candidate_protocol_error: Hunted Troll must create exactly four tokens")
    for token in tokens:
        if not isinstance(token, dict) or token.get("controller_seat") != 1 or token.get("colors") != ["Blue"] or token.get("card_types") != ["Creature"] or token.get("subtypes") != ["Faerie"] or token.get("power") != 1 or token.get("toughness") != 1 or token.get("keywords") != ["Flying"]:
            raise WorkerFailure("candidate_protocol_error: Hunted Troll token characteristics differ")
    troll, brainspoil = trace.get("troll_object"), trace.get("brainspoil_object")
    if not isinstance(troll, int) or (regeneration and not isinstance(brainspoil, int)):
        raise WorkerFailure("candidate_protocol_error: Hunted Troll or Brainspoil spell identities are missing")
    if regeneration and trace.get("brainspoil_target_object") != troll:
        raise WorkerFailure("candidate_protocol_error: Brainspoil must target the live Hunted Troll")
    events = trace.get("events")
    kinds = [event.get("kind") for event in events if isinstance(event, dict)] if isinstance(events, list) else []
    expected = ["SpellManaPaid", "SpellCast", "SpellResolved", "CardMoved", "TriggeredAbilityStacked", *("TokenCreated" for _ in range(4)), "AbilityResolved"]
    if regeneration:
        expected += ["SpellManaPaid", "SpellCast", "AbilityManaPaid", "AbilityActivated", "RegenerationShieldCreated", "AbilityResolved", "RegenerationShieldUsed", "SpellResolved"]
    expected += ["GameEnded"]
    if kinds != expected:
        raise WorkerFailure("candidate_protocol_error: ordered Hunted Troll cast, ETB, token, regeneration, or end events differ")
    sequences = [event.get("sequence") for event in events]
    if any(not isinstance(sequence, int) for sequence in sequences) or sequences != sorted(sequences):
        raise WorkerFailure("candidate_protocol_error: Hunted Troll event sequence is malformed")
    facts = [event.get("facts", {}) for event in events]
    if facts[0].get("objects") != [troll] or facts[0].get("seats") != [0] or len(facts[0].get("mana_colors", [])) != 4 or facts[0].get("mana_colors", []).count("Green") < 2:
        raise WorkerFailure("candidate_protocol_error: Hunted Troll four-mana payment differs")
    if facts[1].get("objects") != [troll] or facts[1].get("seats") != [0] or facts[2].get("objects") != [troll] or facts[3].get("objects") != [troll] or facts[3].get("zone") != "Battlefield":
        raise WorkerFailure("candidate_protocol_error: Hunted Troll cast, resolution, or entry facts differ")
    trigger = facts[4]
    if trigger.get("objects") != [troll] or trigger.get("ability") != "etb-opponent-faeries" or trigger.get("incarnation") != etb.get("source_incarnation"):
        raise WorkerFailure("candidate_protocol_error: Hunted Troll ETB trigger facts differ")
    for index, token in enumerate(tokens, start=5):
        if facts[index].get("objects") != [token.get("object")] or facts[index].get("seats") != [1]:
            raise WorkerFailure("candidate_protocol_error: Hunted Troll token event facts differ")
    etb_resolved = facts[9]
    if etb_resolved.get("objects") != [troll] or etb_resolved.get("ability") != "etb-opponent-faeries" or etb_resolved.get("incarnation") != trigger.get("incarnation"):
        raise WorkerFailure("candidate_protocol_error: Hunted Troll ETB resolution facts differ")
    if regeneration:
        regeneration_result = trace.get("regeneration")
        if not isinstance(regeneration_result, dict) or regeneration_result.get("mana_cost") != {"generic": 0, "colored": ["Green"]} or regeneration_result.get("source_incarnation") != regeneration_result.get("resolved_incarnation") or regeneration_result.get("shield_created") is not True or regeneration_result.get("shield_used") is not True or regeneration_result.get("permanent_tapped") is not True or regeneration_result.get("still_on_battlefield") is not True:
            raise WorkerFailure("candidate_protocol_error: Hunted Troll regeneration outcome differs")
        base = 10
        if facts[base].get("objects") != [brainspoil] or facts[base].get("seats") != [1] or len(facts[base].get("mana_colors", [])) != 5 or facts[base].get("mana_colors", []).count("Black") < 2:
            raise WorkerFailure("candidate_protocol_error: Brainspoil payment differs")
        if facts[base + 1].get("objects") != [brainspoil] or facts[base + 1].get("seats") != [1]:
            raise WorkerFailure("candidate_protocol_error: Brainspoil cast facts differ")
        if facts[base + 2].get("objects") != [troll] or facts[base + 2].get("seats") != [0] or facts[base + 2].get("ability") != "self-regeneration" or facts[base + 2].get("mana_cost_generic") != 0 or facts[base + 2].get("mana_cost_colors") != ["Green"]:
            raise WorkerFailure("candidate_protocol_error: Hunted Troll green ability payment facts differ")
        if facts[base + 3].get("objects") != [troll] or facts[base + 3].get("seats") != [0] or facts[base + 3].get("ability") != "self-regeneration" or facts[base + 3].get("incarnation") != regeneration_result.get("source_incarnation"):
            raise WorkerFailure("candidate_protocol_error: Hunted Troll regeneration activation facts differ")
        if facts[base + 4].get("objects") != [troll, troll] or facts[base + 5].get("ability") != "self-regeneration" or facts[base + 5].get("incarnation") != facts[base + 3].get("incarnation") or facts[base + 6].get("objects") != [troll, troll] or facts[base + 7].get("objects") != [brainspoil]:
            raise WorkerFailure("candidate_protocol_error: Hunted Troll regeneration shield and Brainspoil order differ")
    if facts[-1].get("seats") != [0]:
        raise WorkerFailure("candidate_protocol_error: Hunted Troll trace winner fact differs")


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
        raise FileNotFoundError(f"trusted Hunted Troll authority source is missing: {authority}")
    if not shutil.which("docker"):
        raise FileNotFoundError("Docker is required for Magic candidate isolation")
    verify_workspace(workspace, manifest)
    scoring_workspace = make_scoring_snapshot(workspace, manifest_data["files"], output_root / "candidate-workspace")
    candidate_hash = sha256((scoring_workspace / TARGET).read_bytes())
    name = f"cardbench-magic-hunted-troll-{uuid.uuid4().hex[:16]}"
    command = sandbox_command(scoring_workspace, cargo_registry.resolve(), authority, name, worker_image, candidate_kind)
    code, stdout, stderr = run_bounded(command, timeout=900, name=name)
    (output_root / "worker.stdout").write_bytes(stdout)
    (output_root / "worker.stderr").write_bytes(stderr)
    if code != 0:
        result = {"schema_version":"cardbench.magic.card-pvp.check.v1","task_id":"cardbench/magic/card","candidate":{"kind":candidate_kind,"source_sha256":candidate_hash},"passed":False,"pilot_card_passed":False,"cards_passed":0,"cards_total":291,"printings_total":306,"cards_credited":0,"coverage_complete":False,"full_reward":False,"harbor_reward":0.0,"pilot_reward":0.0,"sealed_authority":False,"worker_image":worker_image,"sandbox":{"network":"none","rootfs":"read-only","pids_limit":128},"error":"candidate compile or Hunted Troll reference trace failed","diagnostics":stderr.decode(errors="replace")[-12000:]}
    else:
        result = parse_report(stdout)
        result["candidate"] = {"kind":candidate_kind,"card_definition":"RAV-HUNTED-TROLL","source_sha256":candidate_hash,"materialized_workspace_sha256":manifest_data["workspace_sha256"]}
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
        (args.output_root / "reward.txt").write_text("0.0\n", encoding="utf-8")
        print(f"Magic candidate verifier failed: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
