#!/usr/bin/env python3
"""Run the Hunted Horror candidate in a bounded offline worker and validate its full-game trace."""

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
MATERIALIZER = Path(__file__).with_name("materialize_hunted_horror_task.py")
TARGET = Path("varieties/magic/sets/ravnica_city_of_guilds/src/cards/hunted_horror.rs")
AUTHORITY = Path("varieties/magic/session/src/bin/rav_hunted_horror_pvp.rs")
IMAGE = "rust@sha256:d7482085ff5b415f84dba5647ae71606650bdef00db7aeb69f4b3d170c3e4082"
MAX_OUTPUT = 2 * 1024 * 1024


class WorkerFailure(RuntimeError):
    pass


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def verify_workspace(workspace: Path, manifest: Path) -> None:
    subprocess.run([sys.executable, str(MATERIALIZER), "--workspace", str(workspace), "--manifest", str(manifest), "--verify"], check=True, cwd=REPO)


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
        "cp /trusted-authority.rs /work/varieties/magic/session/src/bin/rav_hunted_horror_pvp.rs && "
        "cd /work && cargo run --offline --locked -j 2 --manifest-path varieties/magic/Cargo.toml "
        "-p cardbench-magic-session --bin rav-hunted-horror-pvp --" + candidate,
    ]


def validate_trace(trace: object) -> None:
    if not isinstance(trace, dict) or trace.get("id") != "rav-hunted-horror-opponent-centaurs-trample" or trace.get("passed") is not True or trace.get("terminal") != "Winner(PlayerId(0))" or trace.get("winner_seat") != 0 or trace.get("rejected_moves") != 1:
        raise WorkerFailure("candidate_protocol_error: Hunted Horror did not reach the expected terminal duel result")
    if not isinstance(trace.get("turns"), int) or not 0 < trace["turns"] <= 60 or not isinstance(trace.get("accepted_moves"), int) or trace["accepted_moves"] <= 0:
        raise WorkerFailure("candidate_protocol_error: Hunted Horror game bounds or accepted-move receipt is invalid")
    if trace.get("seed") != 0 or not isinstance(trace.get("digest"), str) or not trace["digest"]:
        raise WorkerFailure("candidate_protocol_error: Hunted Horror trace lacks its fixed seed or replay digest")
    source = trace.get("horror_object")
    horror, etb, combat = trace.get("horror"), trace.get("etb"), trace.get("combat")
    if not isinstance(source, int) or not all(isinstance(item, dict) for item in (horror, etb, combat)):
        raise WorkerFailure("candidate_protocol_error: Hunted Horror mechanics are missing")
    if (horror.get("mana_cost") != "{B}{B}" or horror.get("paid_colors") != ["Black", "Black"]
        or horror.get("power") != 7 or horror.get("toughness") != 7 or horror.get("trample") is not True
        or horror.get("printed_subtype") != "Horror" or horror.get("subtype_asserted") is not False):
        raise WorkerFailure("candidate_protocol_error: Hunted Horror body, Trample, or exact black payment differs")
    tokens = etb.get("tokens")
    if (etb.get("target_seat") != 1 or etb.get("trigger_controller_seat") != 0
        or etb.get("target_decision_actor_seat") != etb.get("trigger_controller_seat")
        or etb.get("target_decision_submissions") != 1
        or not isinstance(etb.get("source_incarnation"), int)
        or etb.get("resolved_incarnation") != etb.get("source_incarnation") or not isinstance(tokens, list) or len(tokens) != 2):
        raise WorkerFailure("candidate_protocol_error: Hunted Horror ETB controller-submitted target or token count differs")
    token_ids: list[int] = []
    for token in tokens:
        if (not isinstance(token, dict) or not isinstance(token.get("object"), int)
            or token.get("controller_seat") != 1 or token.get("green") is not True
            or token.get("creature") is not True or token.get("centaur_subtype") is not True
            or token.get("power") != 3 or token.get("toughness") != 3
            or token.get("keywords") != ["Protection(Black)"] or token.get("present_after_creation") is not True):
            raise WorkerFailure("candidate_protocol_error: opponent did not receive two live green 3/3 Centaurs with Protection from Black")
        token_ids.append(token["object"])
    if len(set(token_ids)) != 2:
        raise WorkerFailure("candidate_protocol_error: Hunted Horror Centaur object identities are not distinct")
    blocker = combat.get("watchwolf_object")
    if (combat.get("protection_block_probe_rejected") is not True or combat.get("probe_state_unchanged") is not True
        or not isinstance(blocker, int) or combat.get("watchwolf_blocked") is not True
        or combat.get("damage_to_watchwolf") != 3 or combat.get("damage_to_player", 0) < 4):
        raise WorkerFailure("candidate_protocol_error: protection boundary or exact trample combat proof is missing")
    events = trace.get("events")
    if not isinstance(events, list) or not all(isinstance(event, dict) for event in events):
        raise WorkerFailure("candidate_protocol_error: Hunted Horror event trace is malformed")
    sequences = [event.get("sequence") for event in events]
    if any(not isinstance(sequence, int) for sequence in sequences) or sequences != sorted(sequences) or len(set(sequences)) != len(sequences):
        raise WorkerFailure("candidate_protocol_error: Hunted Horror event order is malformed")
    if trace.get("ordered_event_sequence") != sequences:
        raise WorkerFailure("candidate_protocol_error: Hunted Horror ordered event sequence differs from its projected events")
    by_kind: dict[str, list[dict]] = {}
    for event in events:
        by_kind.setdefault(event.get("kind"), []).append(event)
    ability_id = "etb-opponent-centaurs"
    paid = next((e for e in by_kind.get("SpellManaPaid", []) if e.get("facts", {}).get("objects") == [source] and e.get("facts", {}).get("seats") == [0]), None)
    cast = next((e for e in by_kind.get("SpellCast", []) if e.get("facts", {}).get("objects") == [source]), None)
    resolved = next((e for e in by_kind.get("SpellResolved", []) if e.get("facts", {}).get("objects") == [source]), None)
    entered = next((e for e in by_kind.get("CardMoved", []) if e.get("facts", {}).get("objects") == [source] and e.get("facts", {}).get("zone") == "Battlefield"), None)
    stacked = next((e for e in by_kind.get("TriggeredAbilityStacked", []) if e.get("facts", {}).get("objects") == [source] and e.get("facts", {}).get("ability") == ability_id), None)
    target_choice = next((e for e in by_kind.get("PolicyMoveSubmitted", []) if e.get("facts", {}).get("seats") == [0] and e.get("facts", {}).get("policy") == "rav.hunted-horror-pilot.v1" and e.get("facts", {}).get("move_kind") == "ChooseTriggeredAbilityTargets"), None)
    created = [next((e for e in by_kind.get("TokenCreated", []) if e.get("facts", {}).get("objects") == [token] and e.get("facts", {}).get("seats") == [1]), None) for token in token_ids]
    ability = next((e for e in by_kind.get("AbilityResolved", []) if e.get("facts", {}).get("objects") == [source] and e.get("facts", {}).get("ability") == ability_id), None)
    attack = next((e for e in by_kind.get("AttackersDeclared", []) if e.get("facts", {}).get("seats") == [0] and source in e.get("facts", {}).get("objects", [])), None)
    block = next((e for e in by_kind.get("BlockersDeclared", []) if e.get("facts", {}).get("seats") == [1] and f"(ObjectId({source}), ObjectId({blocker}))" in e.get("canonical", "")), None)
    permanent_damage = next((e for e in by_kind.get("DamageDealtToPermanent", []) if f"source: ObjectId({source})" in e.get("canonical", "") and f"permanent: ObjectId({blocker})" in e.get("canonical", "") and e.get("facts", {}).get("amount") == 3), None)
    player_damage = next((e for e in by_kind.get("DamageDealtToPlayer", []) if e.get("facts", {}).get("seats") == [1] and f"source: ObjectId({source})" in e.get("canonical", "") and e.get("facts", {}).get("amount") == 4), None)
    ended = next((e for e in by_kind.get("GameEnded", []) if e.get("facts", {}).get("seats") == [0]), None)
    if any(e is None for e in (paid, cast, resolved, entered, stacked, target_choice, ability, attack, block, permanent_damage, player_damage, ended)) or any(e is None for e in created):
        raise WorkerFailure("candidate_protocol_error: Hunted Horror cast, entry, controller target choice, ETB, tokens, protection, trample, or terminal receipts are missing")
    colors = paid.get("facts", {}).get("mana_colors", [])
    if colors != ["Black", "Black"]:
        raise WorkerFailure("candidate_protocol_error: Hunted Horror payment receipt is not exactly {B}{B}")
    if not (paid["sequence"] < cast["sequence"] < resolved["sequence"] < entered["sequence"] < stacked["sequence"] < target_choice["sequence"] < created[0]["sequence"] <= created[1]["sequence"] < ability["sequence"] < attack["sequence"] < block["sequence"] < permanent_damage["sequence"] < player_damage["sequence"] < ended["sequence"]):
        raise WorkerFailure("candidate_protocol_error: Hunted Horror cast/ETB/token/trample event order differs")


def validate_report(report: dict) -> None:
    expected_card = {"collector_number": 90, "definition_id": "RAV-HUNTED-HORROR", "name": "Hunted Horror"}
    if (report.get("task_id") != "cardbench/magic/card" or report.get("passed") is not True
        or report.get("pilot_card_passed") is not True or report.get("cards_passed") != 1
        or report.get("cards_credited") != 0 or report.get("cards_total") != 291
        or report.get("printings_total") != 306 or report.get("coverage_complete") is not False
        or report.get("full_reward") is not False or report.get("harbor_reward") != 0.0
        or report.get("pilot_reward") != 0.0 or report.get("viewer_scope") != "Judge"
        or report.get("card") != expected_card):
        raise WorkerFailure("candidate_protocol_error: incomplete Hunted Horror diagnostic result")
    candidate = report.get("candidate")
    if not isinstance(candidate, dict) or candidate.get("card_definition") != "RAV-HUNTED-HORROR":
        raise WorkerFailure("candidate_protocol_error: wrong Hunted Horror candidate identity")
    source_validation = report.get("candidate_source_validation")
    if (not isinstance(source_validation, dict)
        or source_validation.get("candidate_ast_contract_matched") is not True
        or source_validation.get("compile_valid_injected_side_effect_candidate_rejected") is not True
        or source_validation.get("guard_ran_before_candidate_function_calls") is not True):
        raise WorkerFailure("candidate_protocol_error: Hunted Horror source guard attestations are incomplete")
    traces = report.get("traces")
    if not isinstance(traces, list) or len(traces) != 1:
        raise WorkerFailure("candidate_protocol_error: exactly one Hunted Horror full-game trace is required")
    validate_trace(traces[0])
    breakdown = report.get("per_card_breakdown")
    if not isinstance(breakdown, list) or sum(
        1 for row in breakdown
        if isinstance(row, dict)
        and row.get("collector_number") == 90
        and row.get("name") == "Hunted Horror"
        and row.get("definition_id") == "RAV-HUNTED-HORROR"
        and row.get("trace_status") == "passed"
        and row.get("credited") is False
    ) != 1:
        raise WorkerFailure("candidate_protocol_error: Hunted Horror catalog identity or zero-credit row is missing")
    required = ("body_changed_detected_in_game", "trample_removed_detected_in_game", "etb_trigger_removed_detected_in_game", "opponent_target_requirement_removed_detected_in_game", "centaur_protection_removed_detected_in_game", "centaur_count_changed_detected_in_game", "mana_cost_changed_detected_in_game", "compile_valid_candidate_side_effect_rejected_before_execution")
    mutation = report.get("mutation_sensitivity")
    if not isinstance(mutation, dict) or any(mutation.get(key) is not True for key in required):
        raise WorkerFailure("candidate_protocol_error: Hunted Horror mutation controls are incomplete")


def parse_report(stdout: bytes) -> dict:
    for line in reversed(stdout.decode(errors="replace").splitlines()):
        try:
            report = json.loads(line)
        except json.JSONDecodeError:
            continue
        if report.get("schema_version") == "cardbench.magic.card-pvp.check.v1":
            validate_report(report)
            return report
    raise WorkerFailure("candidate_protocol_error: no valid Hunted Horror PvP result")


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
        raise FileNotFoundError(f"trusted Hunted Horror authority is missing: {authority}")
    if not shutil.which("docker"):
        raise FileNotFoundError("Docker is required for Magic candidate isolation")
    verify_workspace(workspace, manifest)
    snapshot = snapshot_workspace(workspace, data["files"], output_root / "candidate-workspace")
    digest = sha256((snapshot / TARGET).read_bytes())
    container = f"cardbench-magic-hunted-horror-{uuid.uuid4().hex[:16]}"
    code, stdout, stderr = run_bounded(sandbox_command(snapshot, registry.resolve(), authority, container, image, candidate_kind), 900, container)
    (output_root / "worker.stdout").write_bytes(stdout)
    (output_root / "worker.stderr").write_bytes(stderr)
    if code != 0:
        result = {"schema_version": "cardbench.magic.card-pvp.check.v1", "task_id": "cardbench/magic/card", "candidate": {"kind": candidate_kind, "source_sha256": digest}, "passed": False, "pilot_card_passed": False, "cards_passed": 0, "cards_total": 291, "printings_total": 306, "cards_credited": 0, "coverage_complete": False, "full_reward": False, "harbor_reward": 0.0, "pilot_reward": 0.0, "sealed_authority": False, "worker_image": image, "sandbox": {"network": "none", "rootfs": "read-only", "pids_limit": 128}, "error": "candidate compile or Hunted Horror reference trace failed", "diagnostics": stderr.decode(errors="replace")[-12000:]}
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
