#!/usr/bin/env python3
"""Run Divebomber Griffin in a bounded offline worker and validate its PvP trace."""

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
MATERIALIZER = Path(__file__).with_name("materialize_divebomber_griffin_task.py")
TARGET = Path("varieties/magic/sets/ravnica_city_of_guilds/src/cards/divebomber_griffin.rs")
AUTHORITY = Path("varieties/magic/session/src/bin/rav_divebomber_griffin_pvp.rs")
IMAGE = "rust@sha256:d7482085ff5b415f84dba5647ae71606650bdef00db7aeb69f4b3d170c3e4082"
MAX_OUTPUT = 2 * 1024 * 1024


class WorkerFailure(RuntimeError):
    pass


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def verify_workspace(workspace: Path, manifest: Path) -> None:
    subprocess.run([sys.executable, str(MATERIALIZER), "--workspace", str(workspace), "--manifest", str(manifest), "--verify"], check=True, cwd=REPO)


def snapshot_workspace(workspace: Path, files: dict[str, str], root: Path) -> Path:
    if root.is_symlink():
        raise WorkerFailure("candidate_snapshot_path_is_symlink")
    if root.exists():
        shutil.rmtree(root)
    root.mkdir(parents=True)
    for relative in sorted(files):
        source = workspace / relative
        if source.is_symlink() or not source.is_file():
            raise WorkerFailure(f"candidate_snapshot_source_missing: {relative}")
        destination = root / relative
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(source, destination)
    return root


def run_bounded(command: list[str], timeout: int, container: str) -> tuple[int, bytes, bytes]:
    process = subprocess.Popen(command, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    assert process.stdout is not None and process.stderr is not None
    selector = selectors.DefaultSelector()
    selector.register(process.stdout, selectors.EVENT_READ, "stdout")
    selector.register(process.stderr, selectors.EVENT_READ, "stderr")
    output, diagnostics = bytearray(), bytearray()
    started = time.monotonic()
    try:
        while selector.get_map() or process.poll() is None:
            if time.monotonic() - started > timeout:
                raise WorkerFailure("candidate_timeout: Docker worker exceeded its deadline")
            for key, _ in selector.select(0.2):
                chunk = os.read(key.fileobj.fileno(), 65536)
                if not chunk:
                    selector.unregister(key.fileobj)
                    continue
                (output if key.data == "stdout" else diagnostics).extend(chunk)
                if len(output) + len(diagnostics) > MAX_OUTPUT:
                    raise WorkerFailure("candidate_output_limit: worker output exceeded 2 MiB")
        return process.wait(), bytes(output), bytes(diagnostics)
    except BaseException:
        process.kill()
        process.wait()
        subprocess.run(["docker", "rm", "-f", container], capture_output=True, check=False)
        raise


def sandbox_command(workspace: Path, registry: Path, authority: Path, container: str, image: str, candidate_kind: str) -> list[str]:
    if authority.is_symlink() or not authority.is_file():
        raise WorkerFailure("trusted_authority_source_missing")
    candidate = " --candidate" if candidate_kind == "agent" else ""
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
        "cp /trusted-authority.rs /work/varieties/magic/session/src/bin/rav_divebomber_griffin_pvp.rs && "
        "cd /work && cargo run --offline --locked -j 2 --manifest-path varieties/magic/Cargo.toml "
        "-p cardbench-magic-session --bin rav-divebomber-griffin-pvp --" + candidate,
    ]


def validate_trace(trace: object) -> None:
    if not isinstance(trace, dict) or trace.get("id") != "rav-divebomber-griffin-sacrifice-damage-attacker" or trace.get("passed") is not True or trace.get("terminal") != "Winner(PlayerId(0))" or trace.get("winner_seat") != 0 or trace.get("rejected_moves") != 0:
        raise WorkerFailure("candidate_protocol_error: Divebomber Griffin did not reach a clean terminal P0 win")
    source = trace.get("griffin_object")
    body, combat = trace.get("griffin"), trace.get("combat")
    if not isinstance(source, int) or not isinstance(body, dict) or not isinstance(combat, dict):
        raise WorkerFailure("candidate_protocol_error: Divebomber Griffin trace mechanics are missing")
    attacker, activation_source, target = (combat.get("opponent_attacker_object"), combat.get("activation_source_object"), combat.get("activation_target_object"))
    if (
        body.get("mana_cost") != "{3}{W}{W}" or body.get("power") != 3 or body.get("toughness") != 2
        or body.get("flying") is not True or body.get("sacrificed") is not True
        or not isinstance(attacker, int) or activation_source != source or target != attacker
        or combat.get("ability_resolved") is not True or combat.get("damage") != 3
        or combat.get("target_died") is not True or combat.get("p0_combat_damage", 0) < 20
    ):
        raise WorkerFailure("candidate_protocol_error: Divebomber body, combat target, sacrifice, damage, or terminal win differs")
    mana_colors = trace.get("mana_colors")
    if not isinstance(mana_colors, list) or len(mana_colors) != 5 or any(color != "White" for color in mana_colors):
        raise WorkerFailure("candidate_protocol_error: Divebomber Griffin payment is not exact {3}{W}{W}")
    events = trace.get("events")
    if not isinstance(events, list) or not all(isinstance(event, dict) for event in events):
        raise WorkerFailure("candidate_protocol_error: Divebomber event trace is malformed")
    sequences = [event.get("sequence") for event in events]
    if any(not isinstance(sequence, int) for sequence in sequences) or sequences != sorted(sequences) or len(set(sequences)) != len(sequences):
        raise WorkerFailure("candidate_protocol_error: Divebomber event order is malformed")
    by_kind: dict[str, list[dict]] = {}
    for event in events:
        by_kind.setdefault(event.get("kind"), []).append(event)
    def event_for(kind: str, object_id: int, *, zone: str | None = None) -> dict | None:
        return next((event for event in by_kind.get(kind, []) if object_id in event.get("facts", {}).get("objects", []) and (zone is None or event.get("facts", {}).get("zone") == zone)), None)
    paid = event_for("SpellManaPaid", source)
    cast = event_for("SpellCast", source)
    resolved = event_for("SpellResolved", source)
    attack_event = next((event for event in by_kind.get("AttackersDeclared", []) if attacker in event.get("facts", {}).get("objects", []) and event.get("facts", {}).get("seats") == [1]), None)
    source_death = event_for("CardMoved", source, zone="Graveyard")
    ability = next((event for event in by_kind.get("AbilityActivated", []) if event.get("facts", {}).get("objects") == [source] and event.get("facts", {}).get("ability") == "sacrifice-deal-three-to-attacker-or-blocker"), None)
    ability_resolved = next((event for event in by_kind.get("AbilityResolved", []) if event.get("facts", {}).get("objects") == [source] and event.get("facts", {}).get("ability") == "sacrifice-deal-three-to-attacker-or-blocker"), None)
    damage = next((event for event in by_kind.get("DamageDealtToPermanent", []) if source in event.get("facts", {}).get("objects", []) and attacker in event.get("facts", {}).get("objects", []) and event.get("facts", {}).get("amount") == 3), None)
    target_death = event_for("CardMoved", attacker, zone="Graveyard")
    ended = next((event for event in by_kind.get("GameEnded", []) if event.get("facts", {}).get("seats") == [0]), None)
    if any(event is None for event in (paid, cast, resolved, attack_event, source_death, ability, ability_resolved, damage, target_death, ended)):
        raise WorkerFailure("candidate_protocol_error: Divebomber cast, attack, activation, sacrifice, damage, death, or terminal receipt is missing")
    colors = paid.get("facts", {}).get("mana_colors", [])
    if len(colors) != 5 or any(color != "White" for color in colors):
        raise WorkerFailure("candidate_protocol_error: exact Divebomber spell payment receipt differs")
    if not (paid["sequence"] < cast["sequence"] < resolved["sequence"] < attack_event["sequence"] < source_death["sequence"] < ability["sequence"] < damage["sequence"] < ability_resolved["sequence"] < target_death["sequence"] < ended["sequence"]):
        raise WorkerFailure("candidate_protocol_error: Divebomber spell/combat/cost/effect/death/end ordering differs")


def parse_report(stdout: bytes) -> dict:
    for line in reversed(stdout.decode(errors="replace").splitlines()):
        try:
            report = json.loads(line)
        except json.JSONDecodeError:
            continue
        if report.get("schema_version") == "cardbench.magic.card-pvp.check.v1":
            validate_report(report)
            return report
    raise WorkerFailure("candidate_protocol_error: no valid Divebomber Griffin PvP result")


def validate_report(report: dict) -> None:
    if (
        report.get("task_id") != "cardbench/magic/card" or report.get("passed") is not True
        or report.get("pilot_card_passed") is not True or report.get("cards_passed") != 1
        or report.get("cards_credited") != 0 or report.get("cards_total") != 291
        or report.get("printings_total") != 306 or report.get("coverage_complete") is not False
        or report.get("full_reward") is not False or report.get("harbor_reward") != 0.0
        or report.get("pilot_reward") != 0.0 or report.get("viewer_scope") != "Judge"
        or report.get("card") != {"collector_number": 14, "definition_id": "RAV-DIVEBOMBER-GRIFFIN", "name": "Divebomber Griffin"}
    ):
        raise WorkerFailure("candidate_protocol_error: incomplete Divebomber Griffin diagnostic result")
    candidate = report.get("candidate")
    if not isinstance(candidate, dict) or candidate.get("card_definition") != "RAV-DIVEBOMBER-GRIFFIN":
        raise WorkerFailure("candidate_protocol_error: wrong Divebomber Griffin candidate identity")
    traces = report.get("traces")
    if not isinstance(traces, list) or len(traces) != 1:
        raise WorkerFailure("candidate_protocol_error: exactly one Divebomber Griffin full-game trace is required")
    validate_trace(traces[0])
    required = ("flying_removed_detected_in_game", "sacrifice_activation_removed_detected_in_game", "attacking_or_blocking_target_restriction_removed_detected_in_game", "damage_amount_changed_detected_in_game")
    mutations = report.get("mutation_sensitivity")
    if not isinstance(mutations, dict) or any(mutations.get(key) is not True for key in required):
        raise WorkerFailure("candidate_protocol_error: Divebomber Griffin gameplay mutations are incomplete")


def score(workspace: Path, manifest: Path, output_root: Path, candidate_kind: str) -> int:
    output_root.mkdir(parents=True, exist_ok=True)
    manifest_data = json.loads(manifest.read_text(encoding="utf-8"))
    image = os.environ.get("CARDBENCH_MAGIC_WORKER_IMAGE", IMAGE)
    if not (image.startswith("rust@sha256:") or image.startswith("sha256:")):
        raise ValueError("CARDBENCH_MAGIC_WORKER_IMAGE must be an immutable digest reference")
    registry = Path.home() / ".cargo" / "registry"
    if not registry.is_dir():
        raise FileNotFoundError(f"offline Cargo registry cache is missing: {registry}")
    authority = REPO / AUTHORITY
    if authority.is_symlink() or not authority.is_file():
        raise FileNotFoundError(f"trusted Divebomber Griffin authority is missing: {authority}")
    if not shutil.which("docker"):
        raise FileNotFoundError("Docker is required for Magic candidate isolation")
    verify_workspace(workspace, manifest)
    snapshot = snapshot_workspace(workspace, manifest_data["files"], output_root / "candidate-workspace")
    digest = sha256((snapshot / TARGET).read_bytes())
    container = f"cardbench-magic-divebomber-griffin-{uuid.uuid4().hex[:16]}"
    code, stdout, stderr = run_bounded(sandbox_command(snapshot, registry.resolve(), authority, container, image, candidate_kind), 900, container)
    (output_root / "worker.stdout").write_bytes(stdout)
    (output_root / "worker.stderr").write_bytes(stderr)
    if code != 0:
        result = {"schema_version": "cardbench.magic.card-pvp.check.v1", "task_id": "cardbench/magic/card", "candidate": {"kind": candidate_kind, "source_sha256": digest}, "passed": False, "pilot_card_passed": False, "cards_passed": 0, "cards_total": 291, "printings_total": 306, "cards_credited": 0, "coverage_complete": False, "full_reward": False, "harbor_reward": 0.0, "pilot_reward": 0.0, "sealed_authority": False, "worker_image": image, "sandbox": {"network": "none", "rootfs": "read-only", "pids_limit": 128}, "error": "candidate compile or Divebomber Griffin reference trace failed", "diagnostics": stderr.decode(errors="replace")[-12000:]}
        (output_root / "result.json").write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
        print(json.dumps(result))
        return 1
    report = parse_report(stdout)
    report["candidate"].update({"kind": candidate_kind, "source_sha256": digest})
    report.update({"sealed_authority": False, "worker_image": image, "sandbox": {"network": "none", "rootfs": "read-only", "pids_limit": 128}})
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
