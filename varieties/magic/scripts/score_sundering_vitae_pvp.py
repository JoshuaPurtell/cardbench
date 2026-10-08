#!/usr/bin/env python3
"""Run the Sundering Vitae diagnostic in an offline isolated worker."""

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
MATERIALIZER = Path(__file__).with_name("materialize_sundering_vitae_task.py")
TARGET = Path("varieties/magic/sets/ravnica_city_of_guilds/src/cards/sundering_vitae.rs")
AUTHORITY_SOURCE = Path("varieties/magic/session/src/bin/rav_sundering_vitae_pvp.rs")
DEFAULT_WORKER_IMAGE = "rust@sha256:d7482085ff5b415f84dba5647ae71606650bdef00db7aeb69f4b3d170c3e4082"
MAX_OUTPUT_BYTES = 2 * 1024 * 1024


class WorkerFailure(RuntimeError):
    pass


def digest(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def verify_workspace(workspace: Path, manifest: Path) -> None:
    subprocess.run([sys.executable, str(MATERIALIZER), "--workspace", str(workspace), "--manifest", str(manifest), "--verify"], check=True, cwd=REPO)


def snapshot(workspace: Path, files: dict[str, str], destination: Path) -> Path:
    if destination.is_symlink():
        raise WorkerFailure("candidate_snapshot_path_is_symlink")
    if destination.exists():
        shutil.rmtree(destination)
    destination.mkdir(parents=True)
    for relative in sorted(files):
        source = workspace / relative
        if source.is_symlink() or not source.is_file():
            raise WorkerFailure(f"candidate_snapshot_source_missing: {relative}")
        target = destination / relative
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(source, target)
    return destination


def bounded(command: list[str], *, timeout: int, name: str) -> tuple[int, bytes, bytes]:
    proc = subprocess.Popen(command, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    assert proc.stdout is not None and proc.stderr is not None
    selector = selectors.DefaultSelector()
    selector.register(proc.stdout, selectors.EVENT_READ, "stdout")
    selector.register(proc.stderr, selectors.EVENT_READ, "stderr")
    output, errors, started = bytearray(), bytearray(), time.monotonic()
    try:
        while selector.get_map() or proc.poll() is None:
            if time.monotonic() - started > timeout:
                raise WorkerFailure("candidate_timeout")
            for key, _ in selector.select(0.2):
                chunk = os.read(key.fileobj.fileno(), 65536)
                if not chunk:
                    selector.unregister(key.fileobj)
                    continue
                (output if key.data == "stdout" else errors).extend(chunk)
                if len(output) + len(errors) > MAX_OUTPUT_BYTES:
                    raise WorkerFailure("candidate_output_limit")
        return proc.wait(), bytes(output), bytes(errors)
    except BaseException:
        proc.kill()
        proc.wait()
        subprocess.run(["docker", "rm", "-f", name], capture_output=True, check=False)
        raise


def validate_report(report: dict) -> None:
    if report.get("schema_version") != "cardbench.magic.card-pvp.check.v1" or report.get("task_id") != "cardbench/magic/card" or report.get("passed") is not True or report.get("pilot_card_passed") is not True or report.get("cards_passed") != 1 or report.get("cards_total") != 291 or report.get("printings_total") != 306 or report.get("cards_credited") != 0 or report.get("coverage_complete") is not False or report.get("full_reward") is not False or report.get("harbor_reward") != 0.0 or report.get("pilot_reward") != 0.0 or report.get("viewer_scope") != "Judge" or report.get("card") != {"collector_number":185,"printings":1,"definition_id":"RAV-SUNDERING-VITAE","name":"Sundering Vitae"}:
        raise WorkerFailure("candidate_protocol_error: incomplete Sundering Vitae report or nonzero credit/reward")
    traces = report.get("traces")
    if not isinstance(traces, list) or len(traces) != 1:
        raise WorkerFailure("candidate_protocol_error: exactly one full-game trace is required")
    trace = traces[0]
    card = trace.get("sundering_vitae", {})
    if trace.get("trace_id") != "rav-sundering-vitae-convoke-destroys-artifact-full-game" or trace.get("terminal") != "Winner(PlayerId(0))" or trace.get("winner_seat") != 0 or trace.get("rejected_moves") != 0 or trace.get("passed") is not True or card.get("mana_cost") != "{2}{G}" or card.get("convoke_contribution") != "green" or card.get("direct_mana_paid") != ["white","white"] or card.get("target_definition") != "RAV-BOROS-SIGNET" or card.get("target_owner_seat") != 1 or card.get("target_zone_after_resolution") != "Graveyard" or card.get("spell_zone_after_resolution") != "Graveyard" or card.get("convoke_creature_tapped") is not True or card.get("terminal_combat_win") is not True:
        raise WorkerFailure("candidate_protocol_error: Convoke, exact target destruction, or complete game facts differ")
    spell, target, convoker = card.get("object"), card.get("target_object"), card.get("convoke_creature")
    if not all(isinstance(value, int) for value in (spell, target, convoker)) or len({spell,target,convoker}) != 3:
        raise WorkerFailure("candidate_protocol_error: spell, target, and Convoke creature identities must be distinct")
    events = trace.get("events")
    required = ["ConvokeUsed","SpellManaPaid","SpellCast","CardDestroyed","CardMoved","SpellResolved","CardMoved","GameEnded"]
    if not isinstance(events, list) or [event.get("kind") for event in events] != required:
        raise WorkerFailure("candidate_protocol_error: incomplete ordered Convoke/destruction/resolution event chain")
    if [event.get("sequence") for event in events] != sorted(event.get("sequence") for event in events):
        raise WorkerFailure("candidate_protocol_error: event projection is not ordered")
    facts = [event.get("facts", {}) for event in events]
    canonical = [event.get("canonical", "") for event in events]
    if facts[0].get("objects") != [convoker] or facts[0].get("seats") != [0] or "contribution: Some(Green)" not in canonical[0] or facts[1].get("objects") != [spell] or facts[1].get("seats") != [0] or facts[1].get("mana_colors") != ["White","White"] or facts[2].get("objects") != [spell] or facts[2].get("seats") != [0] or facts[3].get("objects") != [spell,target] or f"source: ObjectId({spell})" not in canonical[3] or f"card: ObjectId({target})" not in canonical[3] or facts[4].get("objects") != [target] or facts[4].get("zone") != "Graveyard" or facts[5].get("objects") != [spell] or facts[6].get("objects") != [spell] or facts[6].get("zone") != "Graveyard" or facts[7].get("seats") != [0]:
        raise WorkerFailure("candidate_protocol_error: typed payments, target source, resolution, or terminal event differ")
    mutations = report.get("mutation_sensitivity", {})
    if any(mutations.get(key) is not True for key in ("convoke_removed_mutation_detected","cost_mutation_detected","destruction_removed_mutation_detected")) or mutations.get("sealed_mutation_authority") is not False:
        raise WorkerFailure("candidate_protocol_error: semantic mutations are missing or passed")


def sandbox_command(workspace: Path, registry: Path, authority: Path, name: str, image: str, kind: str) -> list[str]:
    if authority.is_symlink() or not authority.is_file():
        raise WorkerFailure("trusted_authority_source_missing")
    flag = " --candidate" if kind == "agent" else ""
    return ["docker","run","--rm","--name",name,"--network","none","--read-only","--cap-drop","ALL","--security-opt","no-new-privileges","--pids-limit","128","--memory","3g","--cpus","2","--user","1000:1000","--tmpfs","/work:rw,exec,size=2147483648,uid=1000,gid=1000","--tmpfs","/tmp:rw,noexec,size=67108864,uid=1000,gid=1000","--mount",f"type=bind,src={workspace},dst=/submission,readonly","--mount",f"type=bind,src={registry},dst=/usr/local/cargo/registry,readonly","--mount",f"type=bind,src={authority.resolve()},dst=/trusted-authority.rs,readonly","--env","HOME=/tmp/home","--env","CARGO_HOME=/usr/local/cargo","--env","RUSTUP_HOME=/usr/local/rustup","--env","CARGO_NET_OFFLINE=true","--env","CARGO_BUILD_JOBS=2","--env","RAYON_NUM_THREADS=2",image,"sh","-c","cp -R /submission/. /work/ && cp /trusted-authority.rs /work/varieties/magic/session/src/bin/rav_sundering_vitae_pvp.rs && cd /work && cargo run --offline --locked -j 2 --manifest-path varieties/magic/Cargo.toml -p cardbench-magic-session --bin rav-sundering-vitae-pvp --" + flag]


def score(workspace: Path, manifest: Path, output: Path, kind: str) -> int:
    output.mkdir(parents=True, exist_ok=True)
    data = json.loads(manifest.read_text(encoding="utf-8"))
    image = os.environ.get("CARDBENCH_MAGIC_WORKER_IMAGE", DEFAULT_WORKER_IMAGE)
    if not (image.startswith("rust@sha256:") or image.startswith("sha256:")):
        raise ValueError("worker image must use an immutable digest")
    authority = REPO / AUTHORITY_SOURCE
    registry = Path.home() / ".cargo" / "registry"
    if not registry.is_dir():
        raise FileNotFoundError("offline Cargo registry cache is missing")
    verify_workspace(workspace, manifest)
    scoring_workspace = snapshot(workspace, data["files"], output / "candidate-workspace")
    candidate_hash = digest((scoring_workspace / TARGET).read_bytes())
    name = f"cardbench-magic-sundering-vitae-{uuid.uuid4().hex[:16]}"
    code, stdout, stderr = bounded(sandbox_command(scoring_workspace, registry.resolve(), authority, name, image, kind), timeout=900, name=name)
    (output / "worker.stdout").write_bytes(stdout)
    (output / "worker.stderr").write_bytes(stderr)
    report = None
    if code == 0:
        for line in reversed(stdout.decode(errors="replace").splitlines()):
            try:
                candidate = json.loads(line)
            except json.JSONDecodeError:
                continue
            if candidate.get("schema_version") == "cardbench.magic.card-pvp.check.v1":
                validate_report(candidate)
                report = candidate
                break
    if report is None:
        report = {"schema_version":"cardbench.magic.card-pvp.check.v1","task_id":"cardbench/magic/card","candidate":{"kind":kind,"source_sha256":candidate_hash},"passed":False,"pilot_card_passed":False,"cards_passed":0,"cards_total":291,"printings_total":306,"cards_credited":0,"coverage_complete":False,"full_reward":False,"harbor_reward":0.0,"pilot_reward":0.0,"error":"candidate compile or Sundering Vitae trace failed","diagnostics":stderr.decode(errors="replace")[-12000:]}
    else:
        report["candidate"] = {"kind":kind,"card_definition":"RAV-SUNDERING-VITAE","source_sha256":candidate_hash,"materialized_workspace_sha256":data["workspace_sha256"]}
        report["candidate_kind"] = kind
        report["starting_workspace_sha256"] = data["workspace_sha256"]
        report["sealed_authority"] = False
        report["worker_image"] = image
        report["sandbox"] = {"network":"none","rootfs":"read-only","pids_limit":128}
        report["cards_credited"] = 0
        report["harbor_reward"] = 0.0
        report["pilot_reward"] = 0.0
    (output / "result.json").write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    (output / "reward.txt").write_text("0.0\n", encoding="utf-8")
    return 0 if report.get("passed") is True and report.get("pilot_card_passed") is True else 1


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--workspace", type=Path)
    parser.add_argument("--manifest", type=Path)
    parser.add_argument("--output-root", type=Path)
    parser.add_argument("--candidate-kind", choices=["agent","reference"], default="agent")
    parser.add_argument("--validate-report", type=Path, help="validate an existing report without running a worker")
    args = parser.parse_args()
    if args.validate_report is not None:
        try:
            report = json.loads(args.validate_report.read_text(encoding="utf-8"))
            validate_report(report)
        except Exception as error:
            print(f"Sundering Vitae report validation failed: {error}", file=sys.stderr)
            return 1
        print("Sundering Vitae report validated; rewards and credits are zero.")
        return 0
    if args.workspace is None or args.manifest is None or args.output_root is None:
        parser.error("--workspace, --manifest, and --output-root are required unless --validate-report is used")
    try:
        return score(args.workspace.resolve(), args.manifest.resolve(), args.output_root.resolve(), args.candidate_kind)
    except Exception as error:
        args.output_root.mkdir(parents=True, exist_ok=True)
        result = {"schema_version":"cardbench.magic.card-pvp.check.v1","task_id":"cardbench/magic/card","passed":False,"pilot_card_passed":False,"cards_passed":0,"cards_total":291,"printings_total":306,"cards_credited":0,"coverage_complete":False,"full_reward":False,"harbor_reward":0.0,"pilot_reward":0.0,"error":str(error)}
        (args.output_root / "result.json").write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
        (args.output_root / "reward.txt").write_text("0.0\n", encoding="utf-8")
        print(f"Sundering Vitae verifier failed: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
