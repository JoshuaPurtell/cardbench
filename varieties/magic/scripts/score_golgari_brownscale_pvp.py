#!/usr/bin/env python3
"""Run Golgari Brownscale in a bounded offline worker and validate its full-game trace."""

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
MATERIALIZER = Path(__file__).with_name("materialize_golgari_brownscale_task.py")
TARGET = Path("varieties/magic/sets/ravnica_city_of_guilds/src/cards/golgari_brownscale.rs")
AUTHORITY = Path("varieties/magic/session/src/bin/rav_golgari_brownscale_pvp.rs")
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
        raise WorkerFailure("trusted_golgari_brownscale_authority_missing")
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
        "cp /trusted-authority.rs /work/varieties/magic/session/src/bin/rav_golgari_brownscale_pvp.rs && "
        "cd /work && cargo run --offline --locked -j 2 --manifest-path varieties/magic/Cargo.toml "
        "-p cardbench-magic-session --bin rav-golgari-brownscale-pvp --" + candidate,
    ]


def validate_trace(trace: object) -> None:
    if not isinstance(trace, dict) or trace.get("id") != "rav-golgari-brownscale-dredge-two-life-trigger-terminal-game" or trace.get("passed") is not True or trace.get("terminal") != "Winner(PlayerId(0))" or trace.get("winner_seat") != 0 or trace.get("rejected_moves") != 0:
        raise WorkerFailure("candidate_protocol_error: Golgari Brownscale did not reach the expected terminal duel result")
    if trace.get("seed") != 12 or not isinstance(trace.get("digest"), str) or not trace["digest"]:
        raise WorkerFailure("candidate_protocol_error: Golgari Brownscale trace lacks the fixed seed or replay digest")
    if not isinstance(trace.get("turns"), int) or not 0 < trace["turns"] <= 80 or not isinstance(trace.get("accepted_moves"), int) or trace["accepted_moves"] <= 0:
        raise WorkerFailure("candidate_protocol_error: Golgari Brownscale game bounds or accepted-move receipt is invalid")
    brownscale, combat, dredge = trace.get("brownscale"), trace.get("combat"), trace.get("dredge")
    card_id, target_id = trace.get("brownscale_object"), trace.get("watchwolf_object")
    if not all(isinstance(value, int) for value in (card_id, target_id)) or not all(isinstance(value, dict) for value in (brownscale, combat, dredge)):
        raise WorkerFailure("candidate_protocol_error: Golgari Brownscale identity or mechanic details are missing")
    if (brownscale.get("mana_cost") != "{1}{G}{G}" or brownscale.get("power") != 2 or brownscale.get("toughness") != 3
        or brownscale.get("dredge") != 2 or brownscale.get("printed_subtype") != "Lizard" or brownscale.get("subtype_asserted") is not False
        or not isinstance(brownscale.get("paid_colors"), list) or len(brownscale["paid_colors"]) != 3 or any(color != "Green" for color in brownscale["paid_colors"])):
        raise WorkerFailure("candidate_protocol_error: Golgari Brownscale identity/cost/characteristics differ")
    if (combat.get("damaged_target_object") != target_id or combat.get("source_object") != card_id
        or combat.get("brownscale_damage") != 2 or combat.get("watchwolf_damage") != 3 or combat.get("brownscale_died") is not True
        or not isinstance(combat.get("source_incarnation"), int)):
        raise WorkerFailure("candidate_protocol_error: Brownscale did not die after the exact Watchwolf combat interaction")
    mills = dredge.get("mill_objects")
    if (dredge.get("card_object") != card_id or dredge.get("count") != 2
        or dredge.get("same_object_returned_to_hand") is not True or dredge.get("same_object_recast") is not True
        or dredge.get("source_graveyard_incarnation") != combat.get("source_incarnation") or dredge.get("life_gained") != 2
        or not isinstance(mills, list) or len(mills) != 2 or len(set(mills)) != 2 or card_id in mills):
        raise WorkerFailure("candidate_protocol_error: exact Dredge 2 replacement, return, trigger, and life gain are missing")
    events = trace.get("events")
    if not isinstance(events, list) or not all(isinstance(event, dict) for event in events):
        raise WorkerFailure("candidate_protocol_error: projected Golgari Brownscale events are malformed")
    sequences = [event.get("sequence") for event in events]
    if any(not isinstance(sequence, int) for sequence in sequences) or sequences != sorted(sequences) or len(set(sequences)) != len(sequences):
        raise WorkerFailure("candidate_protocol_error: projected event sequence is malformed")
    if trace.get("ordered_event_sequence") != sequences:
        raise WorkerFailure("candidate_protocol_error: ordered event sequence differs from projected receipts")
    names = ("brownscale_cast", "brownscale_first_resolved", "watchwolf_attack", "brownscale_block", "brownscale_damage_to_watchwolf", "watchwolf_damage_to_brownscale", "brownscale_death", "brownscale_returned_to_hand", "dredged", "trigger_stacked", "trigger_resolved", "life_gained", "brownscale_recast", "brownscale_recast_resolved", "terminal")
    positions = trace.get("event_positions")
    if not isinstance(positions, dict) or any(not isinstance(positions.get(name), int) for name in names):
        raise WorkerFailure("candidate_protocol_error: Golgari Brownscale ordered event positions are missing")
    damage_window = sorted((positions["brownscale_damage_to_watchwolf"], positions["watchwolf_damage_to_brownscale"]))
    ordered = [positions["brownscale_cast"], positions["brownscale_first_resolved"], positions["watchwolf_attack"], positions["brownscale_block"], *damage_window, positions["brownscale_death"], positions["brownscale_returned_to_hand"], positions["dredged"], positions["trigger_stacked"], positions["trigger_resolved"], positions["life_gained"], positions["brownscale_recast"], positions["brownscale_recast_resolved"], positions["terminal"]]
    if ordered != sorted(ordered) or len(set(positions[name] for name in names)) != len(names):
        raise WorkerFailure("candidate_protocol_error: combat, Dredge, trigger, life gain, recast, and terminal order differs")
    by_sequence = {event["sequence"]: event for event in events}
    expected_kinds = {
        "brownscale_cast": "SpellCast", "brownscale_first_resolved": "SpellResolved", "watchwolf_attack": "AttackersDeclared",
        "brownscale_block": "BlockersDeclared", "brownscale_damage_to_watchwolf": "DamageDealtToPermanent",
        "watchwolf_damage_to_brownscale": "DamageDealtToPermanent", "brownscale_death": "CardMoved",
        "brownscale_returned_to_hand": "CardMoved", "dredged": "Dredged", "trigger_stacked": "TriggeredAbilityStacked",
        "trigger_resolved": "AbilityResolved", "life_gained": "LifeGained", "brownscale_recast": "SpellCast",
        "brownscale_recast_resolved": "SpellResolved", "terminal": "GameEnded",
    }
    receipts = {}
    for name, kind in expected_kinds.items():
        receipt = by_sequence.get(positions[name])
        if receipt is None or receipt.get("kind") != kind:
            raise WorkerFailure(f"candidate_protocol_error: ordered {name} receipt does not match {kind}")
        receipts[name] = receipt
    canonical = "\n".join(str(event.get("canonical", "")) for event in events)
    brownscale_hit = str(receipts["brownscale_damage_to_watchwolf"].get("canonical", ""))
    watchwolf_hit = str(receipts["watchwolf_damage_to_brownscale"].get("canonical", ""))
    graveyard_move = str(receipts["brownscale_death"].get("canonical", ""))
    if (f"ObjectId({card_id})" not in canonical or f"ObjectId({target_id})" not in canonical
        or "graveyard-to-hand-gain-life" not in canonical or "count: 2" not in str(receipts["dredged"].get("canonical", ""))
        or f"source: ObjectId({card_id})" not in brownscale_hit or f"permanent: ObjectId({target_id})" not in brownscale_hit or "amount: 2" not in brownscale_hit
        or f"source: ObjectId({target_id})" not in watchwolf_hit or f"permanent: ObjectId({card_id})" not in watchwolf_hit or "amount: 3" not in watchwolf_hit
        or f"card: ObjectId({card_id})" not in graveyard_move or "to: Graveyard" not in graveyard_move
        or f"card: ObjectId({card_id})" not in str(receipts["dredged"].get("canonical", ""))
        or f"card: ObjectId({card_id})" not in str(receipts["brownscale_returned_to_hand"].get("canonical", ""))
        or f"card: ObjectId({card_id})" not in str(receipts["brownscale_cast"].get("canonical", ""))
        or f"card: ObjectId({card_id})" not in str(receipts["brownscale_recast"].get("canonical", ""))
        or f"source: ObjectId({card_id})" not in str(receipts["trigger_stacked"].get("canonical", ""))
        or f"source: ObjectId({card_id})" not in str(receipts["trigger_resolved"].get("canonical", ""))
        or f"source_incarnation: {combat['source_incarnation']}" not in str(receipts["trigger_stacked"].get("canonical", ""))
        or f"source_incarnation: {combat['source_incarnation']}" not in str(receipts["trigger_resolved"].get("canonical", ""))
        or "amount: 2" not in str(receipts["life_gained"].get("canonical", ""))):
        raise WorkerFailure("candidate_protocol_error: exact source/target/Dredge/life-gain event provenance is missing")
    mill_sequences = []
    for mill in mills:
        matches = [event.get("sequence") for event in events if event.get("kind") == "CardMoved"
            and "to: Graveyard" in str(event.get("canonical", ""))
            and f"card: ObjectId({mill})" in str(event.get("canonical", ""))]
        if not matches:
            raise WorkerFailure("candidate_protocol_error: exact library-top object is missing from the Dredge mill receipts")
        mill_sequences.append(matches[0])
    if mill_sequences != sorted(mill_sequences) or max(mill_sequences) >= positions["brownscale_returned_to_hand"]:
        raise WorkerFailure("candidate_protocol_error: the two Dredge cards are not milled in order before Brownscale returns")


def validate_report(report: dict) -> None:
    card = {"collector_number": 166, "definition_id": "RAV-GOLGARI-BROWNSCALE", "name": "Golgari Brownscale"}
    if (report.get("schema_version") != "cardbench.magic.card-pvp.check.v1" or report.get("task_id") != "cardbench/magic/card"
        or report.get("passed") is not True or report.get("pilot_card_passed") is not True or report.get("cards_passed") != 1
        or report.get("cards_credited") != 0 or report.get("cards_total") != 291 or report.get("printings_total") != 306
        or report.get("coverage_complete") is not False or report.get("full_reward") is not False
        or report.get("harbor_reward") != 0.0 or report.get("pilot_reward") != 0.0
        or report.get("viewer_scope") != "Judge" or report.get("card") != card):
        raise WorkerFailure("candidate_protocol_error: incomplete Golgari Brownscale zero-credit diagnostic report")
    source = report.get("candidate_source_validation")
    if (not isinstance(source, dict) or source.get("candidate_ast_contract_matched") is not True
        or source.get("compile_valid_injected_side_effect_candidate_rejected") is not True
        or source.get("guard_ran_before_candidate_function_calls") is not True):
        raise WorkerFailure("candidate_protocol_error: Golgari Brownscale source-gate attestations are incomplete")
    traces = report.get("traces")
    if not isinstance(traces, list) or len(traces) != 1:
        raise WorkerFailure("candidate_protocol_error: exactly one Golgari Brownscale full-game trace is required")
    validate_trace(traces[0])
    breakdown = report.get("per_card_breakdown")
    if not isinstance(breakdown, list) or sum(1 for row in breakdown if isinstance(row, dict)
        and row.get("collector_number") == 166 and row.get("name") == "Golgari Brownscale"
        and row.get("definition_id") == "RAV-GOLGARI-BROWNSCALE" and row.get("trace_status") == "passed" and row.get("credited") is False) != 1:
        raise WorkerFailure("candidate_protocol_error: Golgari Brownscale RAV catalog identity/zero-credit row is missing")
    required = ("wrong_identity_mutation_detected", "wrong_mana_cost_mutation_detected", "wrong_creature_body_mutation_detected", "missing_dredge_mutation_detected", "wrong_dredge_count_mutation_detected", "graveyard_trigger_removed_mutation_detected", "graveyard_trigger_condition_changed_mutation_detected", "graveyard_trigger_effect_changed_mutation_detected", "graveyard_trigger_amount_changed_mutation_detected", "compile_valid_candidate_side_effect_rejected_before_execution")
    mutation = report.get("mutation_sensitivity")
    if not isinstance(mutation, dict) or any(mutation.get(key) is not True for key in required):
        raise WorkerFailure("candidate_protocol_error: Golgari Brownscale semantic mutation controls are incomplete")


def parse_report(stdout: bytes) -> dict:
    for line in reversed(stdout.decode(errors="replace").splitlines()):
        try:
            report = json.loads(line)
        except json.JSONDecodeError:
            continue
        if report.get("schema_version") == "cardbench.magic.card-pvp.check.v1":
            validate_report(report)
            return report
    raise WorkerFailure("candidate_protocol_error: no valid Golgari Brownscale PvP report")


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
        raise FileNotFoundError(f"trusted Golgari Brownscale authority is missing: {authority}")
    if not shutil.which("docker"):
        raise FileNotFoundError("Docker is required for Magic candidate isolation")
    verify_workspace(workspace, manifest)
    snapshot = snapshot_workspace(workspace, data["files"], output_root / "candidate-workspace")
    digest = sha256((snapshot / TARGET).read_bytes())
    container = f"cardbench-magic-golgari-brownscale-{uuid.uuid4().hex[:16]}"
    code, stdout, stderr = run_bounded(sandbox_command(snapshot, registry.resolve(), authority, container, image, candidate_kind), 900, container)
    (output_root / "worker.stdout").write_bytes(stdout)
    (output_root / "worker.stderr").write_bytes(stderr)
    if code != 0:
        result = {"schema_version": "cardbench.magic.card-pvp.check.v1", "task_id": "cardbench/magic/card", "candidate": {"kind": candidate_kind, "source_sha256": digest}, "passed": False, "pilot_card_passed": False, "cards_passed": 0, "cards_total": 291, "printings_total": 306, "cards_credited": 0, "coverage_complete": False, "full_reward": False, "harbor_reward": 0.0, "pilot_reward": 0.0, "sealed_authority": False, "worker_image": image, "sandbox": {"network": "none", "rootfs": "read-only", "pids_limit": 128}, "error": "candidate compile or Golgari Brownscale reference trace failed", "diagnostics": stderr.decode(errors="replace")[-12000:]}
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
