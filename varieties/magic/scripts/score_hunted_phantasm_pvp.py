#!/usr/bin/env python3
"""Score a sandboxed Hunted Phantasm task workspace with the trusted PvP authority."""

from __future__ import annotations

import argparse
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
MAGIC = REPO / "varieties" / "magic"
MATERIALIZER = MAGIC / "scripts" / "materialize_hunted_phantasm_task.py"
TARGET = Path("varieties/magic/sets/ravnica_city_of_guilds/src/cards/hunted_phantasm.rs")
AUTHORITY = Path("varieties/magic/session/src/bin/rav_hunted_phantasm_pvp.rs")
IMAGE = "rust@sha256:d7482085ff5b415f84dba5647ae71606650bdef00db7aeb69f4b3d170c3e4082"
MAX_OUTPUT = 2 * 1024 * 1024


class WorkerFailure(RuntimeError):
    pass


def sha256(data: bytes) -> str:
    import hashlib

    return hashlib.sha256(data).hexdigest()


def verify_workspace(workspace: Path, manifest: Path) -> None:
    subprocess.run(
        [sys.executable, str(MATERIALIZER), "--workspace", str(workspace), "--manifest", str(manifest), "--verify"],
        check=True,
        cwd=REPO,
    )


def snapshot_workspace(workspace: Path, files: dict[str, str], target_root: Path) -> Path:
    if target_root.is_symlink():
        raise WorkerFailure("candidate_snapshot_path_is_symlink")
    if target_root.exists():
        shutil.rmtree(target_root)
    target_root.mkdir(parents=True)
    for relative in sorted(files):
        source = workspace / relative
        if source.is_symlink() or not source.is_file():
            raise WorkerFailure(f"candidate_snapshot_source_missing: {relative}")
        destination = target_root / relative
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(source, destination)
    return target_root


def run_bounded(command: list[str], timeout: int, container: str) -> tuple[int, bytes, bytes]:
    process = subprocess.Popen(command, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    assert process.stdout is not None and process.stderr is not None
    selector = selectors.DefaultSelector()
    selector.register(process.stdout, selectors.EVENT_READ, "stdout")
    selector.register(process.stderr, selectors.EVENT_READ, "stderr")
    out, err = bytearray(), bytearray()
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
                (out if key.data == "stdout" else err).extend(chunk)
                if len(out) + len(err) > MAX_OUTPUT:
                    raise WorkerFailure("candidate_output_limit: worker output exceeded 2 MiB")
        return process.wait(), bytes(out), bytes(err)
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
        "cp /trusted-authority.rs /work/varieties/magic/session/src/bin/rav_hunted_phantasm_pvp.rs && "
        "cd /work && cargo run --offline --locked -j 2 --manifest-path varieties/magic/Cargo.toml "
        "-p cardbench-magic-session --bin rav-hunted-phantasm-pvp --" + candidate,
    ]


def validate_trace(trace: object) -> None:
    if not isinstance(trace, dict) or trace.get("id") != "rav-hunted-phantasm-opponent-goblins-etb-unblockable" or trace.get("passed") is not True or trace.get("terminal") != "Winner(PlayerId(0))" or trace.get("winner_seat") != 0 or trace.get("rejected_moves") != 0:
        raise WorkerFailure("candidate_protocol_error: Hunted Phantasm did not reach a clean terminal P0 win")
    phantasm, etb = trace.get("phantasm"), trace.get("etb")
    source = trace.get("phantasm_object")
    tokens = etb.get("tokens") if isinstance(etb, dict) else None
    if not isinstance(phantasm, dict) or not isinstance(etb, dict):
        raise WorkerFailure("candidate_protocol_error: Hunted Phantasm mechanics are missing")
    if (
        not isinstance(source, int) or phantasm.get("mana_cost") != "{1}{U}{U}"
        or phantasm.get("paid_colors") != ["Blue"] * 3
        or phantasm.get("power") != 4 or phantasm.get("toughness") != 6
        or phantasm.get("unblockable") is not True or phantasm.get("attacked") is not True
        or phantasm.get("later_turn_attack") is not True
        or not isinstance(phantasm.get("entered_turn"), int)
        or not isinstance(phantasm.get("attack_turn"), int)
        or phantasm["attack_turn"] <= phantasm["entered_turn"]
        or phantasm.get("combat_damage", 0) < 20
        or etb.get("target_seat") != 1 or etb.get("legal_target_seats") != [1]
        or etb.get("target_decision_actor_seat") != 0
        or etb.get("target_decision_submissions") != 1
        or etb.get("trigger_controller_seat") != 0
        or not isinstance(etb.get("source_incarnation"), int)
        or etb.get("resolved_incarnation") != etb.get("source_incarnation")
        or etb.get("tokens_at_resolution") != 5
        or etb.get("both_players_alive_at_creation") is not True
        or etb.get("both_players_alive_at_resolution") is not True
        or not isinstance(tokens, list) or len(tokens) != 5
    ):
        raise WorkerFailure("candidate_protocol_error: Hunted Phantasm cost, body, opponent ETB, Goblins, or unblockable combat differs")
    token_ids: set[int] = set()
    for token in tokens:
        if (
            not isinstance(token, dict) or not isinstance(token.get("object"), int)
            or token.get("controller_seat") != 1 or token.get("red") is not True
            or token.get("creature") is not True or token.get("goblin_subtype") is not True
            or token.get("power") != 1 or token.get("toughness") != 1
            or token.get("keywords") != []
        ):
            raise WorkerFailure("candidate_protocol_error: opponent did not receive five red 1/1 Goblins")
        token_ids.add(token["object"])
    if len(token_ids) != 5:
        raise WorkerFailure("candidate_protocol_error: Goblin token identities are not distinct")
    events = trace.get("events")
    if not isinstance(events, list) or not all(isinstance(event, dict) for event in events):
        raise WorkerFailure("candidate_protocol_error: Hunted Phantasm event trace is malformed")
    sequences = [event.get("sequence") for event in events]
    if any(not isinstance(sequence, int) for sequence in sequences) or sequences != sorted(sequences) or len(set(sequences)) != len(sequences):
        raise WorkerFailure("candidate_protocol_error: Hunted Phantasm event order is malformed")
    by_kind: dict[str, list[dict]] = {}
    for event in events:
        by_kind.setdefault(event.get("kind"), []).append(event)
    ability_id = "etb-opponent-goblins"
    mana_paid = next((e for e in by_kind.get("SpellManaPaid", []) if e.get("facts", {}).get("objects") == [source] and e.get("facts", {}).get("seats") == [0]), None)
    cast = next((e for e in by_kind.get("SpellCast", []) if e.get("facts", {}).get("objects") == [source]), None)
    resolved = next((e for e in by_kind.get("SpellResolved", []) if e.get("facts", {}).get("objects") == [source]), None)
    entered = next((e for e in by_kind.get("CardMoved", []) if e.get("facts", {}).get("objects") == [source] and "to: Battlefield" in e.get("canonical", "")), None)
    stacked = next((e for e in by_kind.get("TriggeredAbilityStacked", []) if e.get("facts", {}).get("objects") == [source] and e.get("facts", {}).get("ability") == ability_id), None)
    target_choices = [
        e for e in by_kind.get("PolicyMoveSubmitted", [])
        if e.get("facts", {}).get("seats") == [0]
        and e.get("facts", {}).get("policy") == "rav.hunted-phantasm-controller.v1"
        and e.get("facts", {}).get("move_kind") == "ChooseTriggeredAbilityTargets"
    ]
    target_choice = target_choices[0] if len(target_choices) == 1 else None
    token_created = [e for e in by_kind.get("TokenCreated", []) if e.get("facts", {}).get("seats") == [1]]
    ability_resolved = next((e for e in by_kind.get("AbilityResolved", []) if e.get("facts", {}).get("objects") == [source] and e.get("facts", {}).get("ability") == ability_id), None)
    attacks = [e for e in by_kind.get("AttackersDeclared", []) if e.get("facts", {}).get("seats") == [0] and source in e.get("facts", {}).get("objects", [])]
    damage = next((e for e in by_kind.get("DamageDealtToPlayer", []) if e.get("facts", {}).get("seats") == [1] and f"source: ObjectId({source})" in e.get("canonical", "") and e.get("facts", {}).get("amount", 0) > 0), None)
    ended = next((e for e in by_kind.get("GameEnded", []) if e.get("facts", {}).get("seats") == [0]), None)
    if any(e is None for e in (mana_paid, cast, resolved, entered, stacked, target_choice, ability_resolved, damage, ended)) or len(token_created) != 5 or not attacks:
        raise WorkerFailure("candidate_protocol_error: Hunted Phantasm cast, controller target decision, ETB, token, combat, or terminal receipts are missing")
    colors = mana_paid.get("facts", {}).get("mana_colors", [])
    if colors != ["Blue"] * 3:
        raise WorkerFailure("candidate_protocol_error: Hunted Phantasm payment is not exactly {1}{U}{U}")
    event_token_ids = {e.get("facts", {}).get("objects", [None])[0] for e in token_created}
    if event_token_ids != token_ids:
        raise WorkerFailure("candidate_protocol_error: token receipts differ from captured Goblin object identities")
    attack = attacks[0]
    if entered.get("turn") == attack.get("turn") or not (mana_paid["sequence"] < cast["sequence"] < resolved["sequence"] < entered["sequence"] < stacked["sequence"] < target_choice["sequence"] < token_created[0]["sequence"] < token_created[1]["sequence"] < token_created[2]["sequence"] < token_created[3]["sequence"] < token_created[4]["sequence"] < ability_resolved["sequence"] < attack["sequence"] < damage["sequence"] < ended["sequence"]):
        raise WorkerFailure("candidate_protocol_error: Hunted Phantasm cast/ETB/token/combat order differs")


def parse_report(stdout: bytes) -> dict:
    for line in reversed(stdout.decode(errors="replace").splitlines()):
        try:
            report = json.loads(line)
        except json.JSONDecodeError:
            continue
        if report.get("schema_version") == "cardbench.magic.card-pvp.check.v1":
            validate_report(report)
            return report
    raise WorkerFailure("candidate_protocol_error: no valid Hunted Phantasm PvP result")


def validate_report(report: dict) -> None:
    expected_card = {"collector_number": 55, "definition_id": "RAV-HUNTED-PHANTASM", "name": "Hunted Phantasm"}
    if (
        report.get("task_id") != "cardbench/magic/card" or report.get("passed") is not True
        or report.get("pilot_card_passed") is not True or report.get("cards_passed") != 1
        or report.get("cards_credited") != 0 or report.get("cards_total") != 291
        or report.get("printings_total") != 306 or report.get("coverage_complete") is not False
        or report.get("full_reward") is not False or report.get("harbor_reward") != 0.0
        or report.get("pilot_reward") != 0.0 or report.get("viewer_scope") != "Judge"
        or report.get("card") != expected_card
    ):
        raise WorkerFailure("candidate_protocol_error: incomplete Hunted Phantasm diagnostic result")
    candidate = report.get("candidate")
    if not isinstance(candidate, dict) or candidate.get("card_definition") != "RAV-HUNTED-PHANTASM":
        raise WorkerFailure("candidate_protocol_error: wrong Hunted Phantasm candidate identity")
    traces = report.get("traces")
    if not isinstance(traces, list) or len(traces) != 1:
        raise WorkerFailure("candidate_protocol_error: exactly one Hunted Phantasm full-game trace is required")
    validate_trace(traces[0])
    required = (
        "body_changed_detected_in_game", "unblockable_removed_detected_in_game",
        "mana_cost_changed_detected_in_game", "etb_trigger_condition_changed_detected_in_game",
        "opponent_target_requirement_changed_detected_in_game", "goblin_characteristics_changed_detected_in_game",
        "goblin_count_changed_detected_in_game", "compile_valid_candidate_side_effect_rejected_before_execution",
    )
    mutation = report.get("mutation_sensitivity")
    if not isinstance(mutation, dict) or any(mutation.get(key) is not True for key in required):
        raise WorkerFailure("candidate_protocol_error: Hunted Phantasm mutation controls are incomplete")


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
        raise FileNotFoundError(f"trusted Hunted Phantasm authority is missing: {authority}")
    if not shutil.which("docker"):
        raise FileNotFoundError("Docker is required for Magic candidate isolation")
    verify_workspace(workspace, manifest)
    snapshot = snapshot_workspace(workspace, manifest_data["files"], output_root / "candidate-workspace")
    digest = sha256((snapshot / TARGET).read_bytes())
    container = f"cardbench-magic-hunted-phantasm-{uuid.uuid4().hex[:16]}"
    command = sandbox_command(snapshot, registry.resolve(), authority, container, image, candidate_kind)
    code, stdout, stderr = run_bounded(command, 900, container)
    (output_root / "worker.stdout").write_bytes(stdout)
    (output_root / "worker.stderr").write_bytes(stderr)
    if code != 0:
        result = {"schema_version": "cardbench.magic.card-pvp.check.v1", "task_id": "cardbench/magic/card", "candidate": {"kind": candidate_kind, "source_sha256": digest}, "passed": False, "pilot_card_passed": False, "cards_passed": 0, "cards_total": 291, "printings_total": 306, "cards_credited": 0, "coverage_complete": False, "full_reward": False, "harbor_reward": 0.0, "pilot_reward": 0.0, "sealed_authority": False, "worker_image": image, "sandbox": {"network": "none", "rootfs": "read-only", "pids_limit": 128}, "error": "candidate compile or Hunted Phantasm reference trace failed", "diagnostics": stderr.decode(errors="replace")[-12000:]}
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
