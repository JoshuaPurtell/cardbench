#!/usr/bin/env python3
"""Score Muddle the Mixture in an isolated offline worker."""

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
MATERIALIZER = Path(__file__).with_name("materialize_muddle_the_mixture_task.py")
TARGET = Path("varieties/magic/sets/ravnica_city_of_guilds/src/cards/muddle_the_mixture.rs")
AUTHORITY = Path("varieties/magic/session/src/bin/rav_muddle_the_mixture_pvp.rs")
IMAGE = "rust@sha256:d7482085ff5b415f84dba5647ae71606650bdef00db7aeb69f4b3d170c3e4082"
MAX_OUTPUT = 2 * 1024 * 1024


class WorkerFailure(RuntimeError):
    pass


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def verify_workspace(workspace: Path, manifest: Path) -> None:
    subprocess.run(
        [sys.executable, str(MATERIALIZER), "--workspace", str(workspace), "--manifest", str(manifest), "--verify"],
        check=True,
        cwd=REPO,
    )


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
        raise WorkerFailure("trusted_muddle_authority_missing")
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
        "cp /trusted-authority.rs /work/varieties/magic/session/src/bin/rav_muddle_the_mixture_pvp.rs && "
        "cd /work && cargo run --offline --locked -j 2 --manifest-path varieties/magic/Cargo.toml "
        "-p cardbench-magic-session --bin rav-muddle-the-mixture-pvp --" + candidate_flag,
    ]


def validate_trace(trace: object) -> None:
    if not isinstance(trace, dict) or trace.get("trace_id") != "rav-muddle-the-mixture-transmute-counter-full-game" or trace.get("passed") is not True:
        raise WorkerFailure("candidate_protocol_error: missing Muddle full-game trace")
    if trace.get("terminal") != "Winner(PlayerId(0))" or trace.get("winner_seat") != 0 or trace.get("rejected_moves") != 0:
        raise WorkerFailure("candidate_protocol_error: Muddle duel did not finish in a clean seat-zero win")
    if not isinstance(trace.get("turns"), int) or trace["turns"] <= 0 or not isinstance(trace.get("accepted_moves"), int) or trace["accepted_moves"] <= 0:
        raise WorkerFailure("candidate_protocol_error: Muddle game bounds are missing")
    transmute, counter, combat = (trace.get(name) for name in ("transmute", "counter", "combat"))
    if not all(isinstance(value, dict) for value in (transmute, counter, combat)):
        raise WorkerFailure("candidate_protocol_error: Muddle mechanic receipts are missing")
    source, found = transmute.get("source"), transmute.get("found")
    counter_source, target = counter.get("source"), counter.get("target_spell")
    if (not isinstance(source, int) or not isinstance(found, int) or source == found
        or transmute.get("source_definition") != "RAV-MUDDLE-THE-MIXTURE"
        or transmute.get("source_incarnation") is None
        or transmute.get("mana_cost") != "{1}{U}{U}"
        or transmute.get("activation_actor") != 0
        or transmute.get("activation_step") != "Some(PrecombatMain)"
        or transmute.get("sorcery_speed_timing") is not True
        or not isinstance(transmute.get("activation_turn"), int)
        or transmute.get("found") == transmute.get("source")
        or transmute.get("found_definition") != "RAV-MUDDLE-THE-MIXTURE"
        or transmute.get("revealed") is not True
        or transmute.get("destination") != "Hand"
        or transmute.get("found_in_hand") is not True
        or transmute.get("source_zone_after_cost") != "Some(Graveyard)"
        or transmute.get("private_decision") is not True
        or transmute.get("hidden_from_opponent_during_choice") is not True
        or transmute.get("shuffle_after_search") is not True
        or transmute.get("resolved") is not True):
        raise WorkerFailure("candidate_protocol_error: exact Transmute source, timing, private reveal, hand movement, or shuffle is absent")
    if (not isinstance(counter_source, int) or counter_source != found or not isinstance(target, int) or target == counter_source
        or counter.get("source_definition") != "RAV-MUDDLE-THE-MIXTURE"
        or counter.get("mana_cost") != "{U}{U}" or counter.get("mana_paid") != ["Blue", "Blue"]
        or counter.get("target_definition") != "RAV-RAIN-OF-EMBERS"
        or counter.get("target_card_type") != "Sorcery"
        or counter.get("target_mana_cost") != "{1}{R}"
        or counter.get("target_owner") != 1
        or counter.get("target_mana_paid") != ["Red", "Red"]
        or counter.get("countered") is not True
        or counter.get("target_zone_after") != "Some(Graveyard)"
        or counter.get("source_resolved") is not True
        or counter.get("source_zone_after") != "Some(Graveyard)"
        or counter.get("target_spell_resolved") is not False):
        raise WorkerFailure("candidate_protocol_error: exact Muddle counter target/payment/resolution receipt is missing")
    attacks, life = combat.get("attack_turns"), combat.get("life_at_terminal")
    if (not isinstance(combat.get("attacking_watchwolf"), int)
        or combat.get("watchwolf_owner") != 0 or combat.get("watchwolf_controller") != 0
        or not isinstance(attacks, list) or len(attacks) < 7
        or not isinstance(combat.get("damage_to_opponent"), int) or combat["damage_to_opponent"] < 20
        or not isinstance(life, list) or len(life) != 2 or life[0] <= 0 or life[1] > 0):
        raise WorkerFailure("candidate_protocol_error: terminal combat damage and life totals are incomplete")

    events = trace.get("events")
    order = trace.get("ordered_event_indices")
    expected_kinds = [
        "AbilityManaPaid", "DiscardedAsAbilityCost", "CardMoved", "AbilityActivated",
        "DecisionOpened", "DecisionCompleted", "CardRevealed", "CardMoved",
        "LibrarySearchResolved", "LibraryShuffled", "Transmuted", "AbilityResolved",
        "SpellCast", "SpellManaPaid", "SpellCast", "SpellCountered", "CardMoved",
        "SpellResolved", "CardMoved", "GameEnded",
    ]
    if not isinstance(events, list) or not all(isinstance(event, dict) for event in events):
        raise WorkerFailure("candidate_protocol_error: projected Muddle events are malformed")
    sequences = [event.get("sequence") for event in events]
    if (any(not isinstance(sequence, int) for sequence in sequences)
        or sequences != list(range(len(events)))
        or not isinstance(order, list) or len(order) != len(expected_kinds)
        or any(not isinstance(index, int) or index < 0 or index >= len(events) for index in order)
        or order != sorted(order) or len(set(order)) != len(order)):
        raise WorkerFailure("candidate_protocol_error: ordered Muddle event sequence is malformed")
    receipts = [events[index] for index in order]
    if [receipt.get("kind") for receipt in receipts] != expected_kinds:
        raise WorkerFailure("candidate_protocol_error: Transmute/counter/combat event ordering differs")
    canon = [str(event.get("canonical", "")) for event in receipts]
    if not (
        f"source: ObjectId({source})" in canon[0] and 'ability: "transmute"' in canon[0]
        and f"source: ObjectId({source})" in canon[1] and f"card: ObjectId({source})" in canon[1]
        and f"card: ObjectId({source})" in canon[2] and "to: Graveyard" in canon[2]
        and f"source: ObjectId({source})" in canon[3] and 'definition: "RAV-MUDDLE-THE-MIXTURE"' in canon[3]
        and "visibility: Private" in canon[4] and "kind: LibrarySearch" in canon[4]
        and f"DecisionId({decision_id(canon[4])})" in canon[5]
        and f"card: ObjectId({found})" in canon[6] and 'definition: "RAV-MUDDLE-THE-MIXTURE"' in canon[6]
        and f"card: ObjectId({found})" in canon[7] and "to: Hand" in canon[7]
        and f"source: ObjectId({source})" in canon[8] and f"found: Some(ObjectId({found}))" in canon[8]
        and "destination: Hand" in canon[8]
        and "player: PlayerId(0)" in canon[9]
        and f"discarded: ObjectId({source})" in canon[10] and f"found: Some(ObjectId({found}))" in canon[10]
        and f"source: ObjectId({source})" in canon[11] and 'ability: "transmute"' in canon[11]
        and f"card: ObjectId({target})" in canon[12]
        and f"card: ObjectId({target})" in canon[13] and "colors: [Red, Red]" in canon[13]
        and f"card: ObjectId({counter_source})" in canon[14]
        and f"card: ObjectId({target})" in canon[15] and f"source: ObjectId({counter_source})" in canon[15]
        and f"card: ObjectId({target})" in canon[16] and "to: Graveyard" in canon[16]
        and f"card: ObjectId({counter_source})" in canon[17]
        and f"card: ObjectId({counter_source})" in canon[18] and "to: Graveyard" in canon[18]
        and "winner: Some(PlayerId(0))" in canon[19]
    ):
        raise WorkerFailure("candidate_protocol_error: canonical object/decision/payment/zone receipts do not match the reported Muddle objects")


def decision_id(canonical: str) -> str:
    marker = "decision: DecisionId("
    if marker not in canonical:
        raise WorkerFailure("candidate_protocol_error: private Transmute decision has no id")
    return canonical.split(marker, 1)[1].split(")", 1)[0]


def validate_report(report: dict) -> None:
    expected_card = {"collector_number": 60, "printings": 1, "definition_id": "RAV-MUDDLE-THE-MIXTURE", "name": "Muddle the Mixture"}
    if (report.get("schema_version") != "cardbench.magic.card-pvp.check.v1"
        or report.get("task_id") != "cardbench/magic/card" or report.get("expansion") != "RAV"
        or report.get("passed") is not True or report.get("pilot_card_passed") is not True
        or report.get("replay_deterministic") is not True or report.get("deterministic_replay") is not True
        or report.get("cards_passed") != 1 or report.get("cards_credited") != 0
        or report.get("cards_total") != 291 or report.get("printings_total") != 306
        or report.get("coverage_complete") is not False or report.get("full_reward") is not False
        or report.get("harbor_reward") != 0.0 or report.get("pilot_reward") != 0.0
        or report.get("viewer_scope") != "Judge" or report.get("card") != expected_card):
        raise WorkerFailure("candidate_protocol_error: incomplete Muddle zero-credit diagnostic report")
    source = report.get("candidate_source_validation")
    if (not isinstance(source, dict)
        or source.get("normalized_literal_ast_schema_checked_before_candidate_calls") is not True
        or source.get("compile_valid_injected_side_effect_fixture_compiled_by_construction") is not True
        or source.get("compile_valid_injected_side_effect_candidate_rejected") is not True
        or source.get("injected_candidate_calls_before_rejection") != 0
        or source.get("guard_ran_before_candidate_function_calls") is not True):
        raise WorkerFailure("candidate_protocol_error: Muddle candidate source gate is not attested before calls")
    traces = report.get("traces")
    if not isinstance(traces, list) or len(traces) != 1:
        raise WorkerFailure("candidate_protocol_error: exactly one Muddle full-game trace is required")
    validate_trace(traces[0])
    breakdown = report.get("per_card_breakdown")
    if not isinstance(breakdown, list) or sum(
        1 for row in breakdown if isinstance(row, dict)
        and row.get("collector_number") == 60 and row.get("name") == "Muddle the Mixture"
        and row.get("definition_id") == "RAV-MUDDLE-THE-MIXTURE"
        and row.get("trace_status") == "passed" and row.get("credited") is False
    ) != 1:
        raise WorkerFailure("candidate_protocol_error: RAV #60 catalog row or zero-credit status is missing")
    mutation = report.get("mutation_sensitivity")
    required = (
        "wrong_spell_cost_mutation_detected", "wrong_counter_effect_mutation_detected",
        "broad_counter_target_mutation_detected", "transmute_removed_mutation_detected",
        "wrong_transmute_cost_mutation_detected",
        "compile_valid_injected_side_effect_candidate_rejected",
    )
    if not isinstance(mutation, dict) or any(mutation.get(key) is not True for key in required):
        raise WorkerFailure("candidate_protocol_error: Muddle semantic mutations are incomplete")


def parse_report(stdout: bytes) -> dict:
    for line in reversed(stdout.decode(errors="replace").splitlines()):
        try:
            report = json.loads(line)
        except json.JSONDecodeError:
            continue
        if report.get("schema_version") == "cardbench.magic.card-pvp.check.v1":
            validate_report(report)
            return report
    raise WorkerFailure("candidate_protocol_error: no valid Muddle PvP report")


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
        raise FileNotFoundError(f"trusted Muddle authority is missing: {authority}")
    if not shutil.which("docker"):
        raise FileNotFoundError("Docker is required for Magic candidate isolation")
    verify_workspace(workspace, manifest)
    snapshot = snapshot_workspace(workspace, data["files"], output_root / "candidate-workspace")
    digest = sha256((snapshot / TARGET).read_bytes())
    container = f"cardbench-magic-muddle-{uuid.uuid4().hex[:16]}"
    code, stdout, stderr = run_bounded(sandbox_command(snapshot, registry.resolve(), authority, container, image, candidate_kind), 900, container)
    (output_root / "worker.stdout").write_bytes(stdout)
    (output_root / "worker.stderr").write_bytes(stderr)
    if code != 0:
        result = {
            "schema_version": "cardbench.magic.card-pvp.check.v1", "task_id": "cardbench/magic/card",
            "candidate": {"kind": candidate_kind, "source_sha256": digest}, "passed": False,
            "pilot_card_passed": False, "cards_passed": 0, "cards_total": 291, "printings_total": 306,
            "cards_credited": 0, "coverage_complete": False, "full_reward": False,
            "harbor_reward": 0.0, "pilot_reward": 0.0, "sealed_authority": False,
            "worker_image": image, "sandbox": {"network": "none", "rootfs": "read-only", "pids_limit": 128},
            "error": "candidate compile or Muddle the Mixture reference trace failed",
            "diagnostics": stderr.decode(errors="replace")[-12000:],
        }
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
