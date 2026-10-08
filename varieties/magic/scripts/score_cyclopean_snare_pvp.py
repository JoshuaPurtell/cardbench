#!/usr/bin/env python3
"""Run Cyclopean Snare candidates in an offline worker and validate PvP evidence."""

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
MATERIALIZER = Path(__file__).with_name("materialize_cyclopean_snare_task.py")
TARGET = Path("varieties/magic/sets/ravnica_city_of_guilds/src/cards/cyclopean_snare.rs")
AUTHORITY_SOURCE = Path("varieties/magic/session/src/bin/rav_cyclopean_snare_pvp.rs")
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
        "docker", "run", "--rm", "--name", name, "--network", "none", "--read-only", "--cap-drop", "ALL",
        "--security-opt", "no-new-privileges", "--pids-limit", "128", "--memory", "3g", "--cpus", "2",
        "--user", "1000:1000", "--tmpfs", "/work:rw,exec,size=2147483648,uid=1000,gid=1000",
        "--tmpfs", "/tmp:rw,noexec,size=67108864,uid=1000,gid=1000", "--mount", f"type=bind,src={workspace},dst=/submission,readonly",
        "--mount", f"type=bind,src={registry},dst=/usr/local/cargo/registry,readonly", "--mount", f"type=bind,src={authority.resolve()},dst=/trusted-authority.rs,readonly",
        "--env", "HOME=/tmp/home", "--env", "CARGO_HOME=/usr/local/cargo", "--env", "RUSTUP_HOME=/usr/local/rustup",
        "--env", "CARGO_NET_OFFLINE=true", "--env", "CARGO_BUILD_JOBS=2", "--env", "RAYON_NUM_THREADS=2", image, "sh", "-c",
        "cp -R /submission/. /work/ && cp /trusted-authority.rs /work/varieties/magic/session/src/bin/rav_cyclopean_snare_pvp.rs && "
        "cd /work && cargo run --offline --locked -j 2 --manifest-path varieties/magic/Cargo.toml -p cardbench-magic-session --bin rav-cyclopean-snare-pvp --" + candidate_flag,
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
    raise WorkerFailure("candidate_protocol_error: no valid Cyclopean Snare PvP result")


def validate_report(report: dict) -> None:
    card_id = "RAV-CYCLOPEAN-SNARE"
    expected_card = {"collector_number": 259, "printings": 1, "definition_id": card_id, "name": "Cyclopean Snare"}
    if (report.get("task_id") != "cardbench/magic/card" or report.get("passed") is not True or report.get("pilot_card_passed") is not True
        or report.get("cards_passed") != 1 or report.get("cards_credited") != 0 or report.get("cards_total") != 291
        or report.get("printings_total") != 306 or report.get("coverage_complete") is not False or report.get("full_reward") is not False
        or report.get("harbor_reward") != 0.0 or report.get("viewer_scope") != "Judge" or report.get("card") != expected_card):
        raise WorkerFailure("candidate_protocol_error: incomplete Cyclopean Snare single-card report")
    candidate = report.get("candidate")
    if not isinstance(candidate, dict) or candidate.get("card_definition") != card_id:
        raise WorkerFailure("candidate_protocol_error: wrong Cyclopean Snare identity")
    rules = report.get("rules_authority")
    if not isinstance(rules, dict) or rules.get("comprehensive_rules") != "2026-08-07" or rules.get("authority_cutoff") != "2026-09-24" or not any("602.1-602.2" in section for section in rules.get("checked_sections", [])):
        raise WorkerFailure("candidate_protocol_error: Cyclopean Snare rules authority is inconsistent")
    traces = report.get("traces")
    if not isinstance(traces, list) or len(traces) != 1 or traces[0].get("trace_id") != "rav-cyclopean-snare-tap-and-return-full-game":
        raise WorkerFailure("candidate_protocol_error: one Cyclopean Snare full-game trace is required")
    trace = traces[0]
    snare, target, combat = trace.get("snare"), trace.get("target"), trace.get("combat")
    life = combat.get("life_at_terminal") if isinstance(combat, dict) else None
    if (trace.get("terminal") != "Winner(PlayerId(0))" or trace.get("winner_seat") != 0 or trace.get("rejected_moves") != 0
        or not isinstance(trace.get("shuffle_seed"), int) or not isinstance(snare, dict) or not isinstance(target, dict) or not isinstance(combat, dict)
        or snare.get("mana_cost") != "{2}" or not isinstance(snare.get("object"), int) or not isinstance(snare.get("mana_paid"), list) or len(snare.get("mana_paid", [])) != 2
        or snare.get("ability_id") != "tap-target-creature" or snare.get("ability_mana_cost") != "{3}" or snare.get("activation_mana_paid") != 3
        or snare.get("tap_cost_paid") is not True or snare.get("source_zone_after_resolution") != "Hand"
        or not isinstance(target.get("object"), int) or target.get("definition") != "RAV-WATCHWOLF" or target.get("owner") != 1
        or target.get("zone_after_resolution") != "Battlefield" or target.get("tapped_after_resolution") is not True
        or combat.get("card") != "Watchwolf" or not isinstance(combat.get("object"), int) or not isinstance(combat.get("attack_turns"), list) or len(combat.get("attack_turns", [])) < 3
        or not isinstance(combat.get("damage_to_opponent"), int) or combat.get("damage_to_opponent") < 9
        or not isinstance(life, list) or len(life) != 2 or life[0] <= 0 or life[1] > 0):
        raise WorkerFailure("candidate_protocol_error: Snare costs, exact target, tap/return state, combat, or terminal result is wrong")
    events, ordered = trace.get("events"), trace.get("ordered_event_sequence")
    if not isinstance(events, list) or len(events) < 40 or any(not isinstance(event, dict) for event in events):
        raise WorkerFailure("candidate_protocol_error: projected Cyclopean Snare event trace is missing or incomplete")
    sequences = [event.get("sequence") for event in events]
    if any(not isinstance(value, int) for value in sequences) or sequences != sorted(sequences):
        raise WorkerFailure("candidate_protocol_error: projected event sequence is malformed")
    if not isinstance(ordered, list) or len(ordered) != 12 or any(not isinstance(value, int) for value in ordered) or ordered != sorted(ordered):
        raise WorkerFailure("candidate_protocol_error: Cyclopean Snare event order is malformed")
    source, target_id, wolf = snare["object"], target["object"], combat["object"]
    def find(kind: str, predicate, after: int = -1) -> int:
        for index, event in enumerate(events):
            if index <= after:
                continue
            if event.get("kind") == kind and predicate(event.get("facts", {}), event.get("canonical", "")):
                return index
        raise WorkerFailure(f"candidate_protocol_error: missing {kind} receipt in Cyclopean Snare trace")
    paid = find("SpellManaPaid", lambda facts, _: facts.get("objects") == [source] and facts.get("seats") == [0] and facts.get("amount") == 2)
    cast = find("SpellCast", lambda facts, _: facts.get("objects") == [source] and facts.get("seats") == [0], paid)
    resolved = find("SpellResolved", lambda facts, _: facts.get("objects") == [source], cast)
    entered = find("CardMoved", lambda facts, _: facts.get("objects") == [source] and facts.get("zone") == "Battlefield", resolved)
    ability_paid = find("AbilityManaPaid", lambda facts, _: facts.get("objects") == [source] and facts.get("ability") == "tap-target-creature" and facts.get("mana_cost_generic") == 3, entered)
    activated = find("AbilityActivated", lambda facts, _: facts.get("objects") == [source] and facts.get("ability") == "tap-target-creature" and facts.get("seats") == [0], ability_paid)
    # PermanentTapped does not expose normalized `facts` in project_events;
    # require both exact object ids in the canonical engine receipt instead.
    tapped = find(
        "PermanentTapped",
        lambda _facts, canonical: canonical.startswith(
            f"PermanentTapped {{ source: ObjectId({source}), card: ObjectId({target_id}) }}"
        ),
        activated,
    )
    returned = find("CardMoved", lambda facts, _: facts.get("objects") == [source] and facts.get("zone") == "Hand", tapped)
    ability_resolved = find("AbilityResolved", lambda facts, _: facts.get("objects") == [source] and facts.get("ability") == "tap-target-creature", returned)
    attack = find("AttackersDeclared", lambda facts, _: facts.get("seats") == [0] and wolf in facts.get("objects", []), ability_resolved)
    damage = find("DamageDealtToPlayer", lambda facts, _: facts.get("objects") == [wolf] and facts.get("seats") == [1] and facts.get("amount", 0) > 0, attack)
    ended = find("GameEnded", lambda facts, _: facts.get("seats") == [0], damage)
    actual = [events[i]["sequence"] for i in [paid, cast, resolved, entered, ability_paid, activated, tapped, returned, ability_resolved, attack, damage, ended]]
    if actual != ordered:
        raise WorkerFailure("candidate_protocol_error: reported event cursor does not match Snare receipts")
    mutations = report.get("mutation_sensitivity")
    required = ("missing_target_tap_mutation_detected_in_game", "missing_source_return_mutation_detected_in_game", "missing_source_tap_cost_mutation_detected_in_game", "wrong_activation_mana_cost_mutation_detected_in_game", "missing_creature_target_mutation_detected_in_game")
    if not isinstance(mutations, dict) or any(mutations.get(key) is not True for key in required) or mutations.get("sealed_mutation_authority") is not False:
        raise WorkerFailure("candidate_protocol_error: Cyclopean Snare gameplay mutations are incomplete")


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
        raise FileNotFoundError("trusted Cyclopean Snare authority is missing")
    if not shutil.which("docker"):
        raise FileNotFoundError("Docker is required for Magic candidate isolation")
    verify_workspace(workspace, manifest)
    scoring_workspace = make_scoring_snapshot(workspace, manifest_data["files"], output_root / "candidate-workspace")
    candidate_hash = sha256((scoring_workspace / TARGET).read_bytes())
    name = f"cardbench-magic-cyclopean-snare-{uuid.uuid4().hex[:16]}"
    code, stdout, stderr = run_bounded(sandbox_command(scoring_workspace, cargo_registry.resolve(), authority, name, worker_image, candidate_kind), timeout=900, name=name)
    (output_root / "worker.stdout").write_bytes(stdout)
    (output_root / "worker.stderr").write_bytes(stderr)
    if code != 0:
        result = {"schema_version":"cardbench.magic.card-pvp.check.v1","task_id":"cardbench/magic/card","candidate":{"kind":candidate_kind,"source_sha256":candidate_hash},"passed":False,"pilot_card_passed":False,"cards_passed":0,"cards_total":291,"printings_total":306,"cards_credited":0,"coverage_complete":False,"full_reward":False,"harbor_reward":0.0,"sealed_authority":False,"worker_image":worker_image,"sandbox":{"network":"none","rootfs":"read-only","pids_limit":128},"error":"candidate compile or Cyclopean Snare reference trace failed","diagnostics":stderr.decode(errors="replace")[-12000:]}
    else:
        result = parse_report(stdout)
        result["candidate"] = {"kind":candidate_kind,"card_definition":"RAV-CYCLOPEAN-SNARE","source_sha256":candidate_hash,"materialized_workspace_sha256":manifest_data["workspace_sha256"]}
        result["candidate_kind"] = candidate_kind
        result["starting_workspace_sha256"] = manifest_data["workspace_sha256"]
        result["sealed_authority"] = False
        result["worker_image"] = worker_image
        result["sandbox"] = {"network":"none","rootfs":"read-only","pids_limit":128}
        result["harbor_reward"] = 0.0
        result["full_reward"] = False
    (output_root / "result.json").write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
    (output_root / "reward.txt").write_text(f"{result['harbor_reward']:.8f}\n", encoding="utf-8")
    print(f"candidate_passed={result.get('pilot_card_passed',False)} cards={result.get('cards_passed',0)}/291 coverage_complete=false")
    return 0 if result.get("passed") is True and result.get("pilot_card_passed") is True else 1


def main() -> int:
    parser=argparse.ArgumentParser();parser.add_argument("--workspace",required=True,type=Path);parser.add_argument("--manifest",required=True,type=Path);parser.add_argument("--output-root",required=True,type=Path);parser.add_argument("--candidate-kind",choices=["agent","reference"],default="agent");args=parser.parse_args()
    try:return score(args.workspace.resolve(),args.manifest.resolve(),args.output_root.resolve(),args.candidate_kind)
    except Exception as error:
        args.output_root.mkdir(parents=True,exist_ok=True);result={"schema_version":"cardbench.magic.card-pvp.check.v1","task_id":"cardbench/magic/card","passed":False,"pilot_card_passed":False,"cards_passed":0,"cards_total":291,"printings_total":306,"cards_credited":0,"coverage_complete":False,"full_reward":False,"harbor_reward":0.0,"sealed_authority":False,"error":str(error)};(args.output_root/"result.json").write_text(json.dumps(result,indent=2)+"\n",encoding="utf-8");(args.output_root/"reward.txt").write_text("0.0\n",encoding="utf-8");print(f"Magic candidate verifier failed: {error}",file=sys.stderr);return 1


if __name__=="__main__":raise SystemExit(main())
