#!/usr/bin/env python3
"""Run and validate the bounded offline Ethereal Usher PvP diagnostic."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import selectors
import shutil
import subprocess
import sys
import tempfile
import time
import uuid
from pathlib import Path

REPO = Path(__file__).resolve().parents[3]
MATERIALIZER = Path(__file__).with_name("materialize_ethereal_usher_task.py")
TARGET = Path("varieties/magic/sets/ravnica_city_of_guilds/src/cards/ethereal_usher.rs")
AUTHORITY = Path("varieties/magic/session/src/bin/rav_ethereal_usher_pvp.rs")
IMAGE = "rust@sha256:d7482085ff5b415f84dba5647ae71606650bdef00db7aeb69f4b3d170c3e4082"
MAX_OUTPUT = 2 * 1024 * 1024


class WorkerFailure(RuntimeError):
    pass


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def verify_workspace(workspace: Path, manifest: Path) -> dict:
    subprocess.run(
        [sys.executable, str(MATERIALIZER), "--workspace", str(workspace), "--manifest", str(manifest), "--verify"],
        check=True,
        cwd=REPO,
    )
    return json.loads(manifest.read_text(encoding="utf-8"))


def snapshot_workspace(workspace: Path, files: dict[str, str], destination: Path) -> Path:
    destination.mkdir(parents=True)
    for relative in sorted(files):
        source = workspace / relative
        if source.is_symlink() or not source.is_file():
            raise WorkerFailure(f"candidate_snapshot_source_missing: {relative}")
        target = destination / relative
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
                raise WorkerFailure("candidate_timeout: offline Rust worker exceeded its deadline")
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


def sandbox_command(workspace: Path, registry: Path, authority: Path, container: str, image: str, candidate: str) -> list[str]:
    if authority.is_symlink() or not authority.is_file():
        raise WorkerFailure("trusted_Ethereal_Usher_authority_missing")
    candidate_arg = " --candidate" if candidate == "agent" else ""
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
        "cp /trusted-authority.rs /work/varieties/magic/session/src/bin/rav_ethereal_usher_pvp.rs && "
        "cd /work && cargo run --offline --locked -j 2 --manifest-path varieties/magic/Cargo.toml "
        "-p cardbench-magic-session --bin rav-ethereal-usher-pvp --" + candidate_arg,
    ]


def validate_trace(trace: object) -> None:
    if not isinstance(trace, dict) or trace.get("trace_id") != "rav-ethereal-usher-transmute-unblockable-terminal-pvp":
        raise WorkerFailure("candidate_protocol_error: Ethereal Usher full-game trace is missing")
    if trace.get("passed") is not True or trace.get("terminal") != "Winner(PlayerId(0))" or trace.get("winner_seat") != 0 or trace.get("rejected_moves") != 0:
        raise WorkerFailure("candidate_protocol_error: Ethereal Usher did not reach a clean seat-zero terminal game")
    if trace.get("decks") != ["rav_ethereal_usher_azorius_watchwolf", "rav_watchwolf_defender_selesnya"]:
        raise WorkerFailure("candidate_protocol_error: declared legal two-player decks differ")
    if not isinstance(trace.get("shuffle_seed"), int) or not 0 <= trace["shuffle_seed"] <= 256:
        raise WorkerFailure("candidate_protocol_error: shuffle seed is outside the bounded authority search")
    if not isinstance(trace.get("turns"), int) or not 0 < trace["turns"] <= 48 or not isinstance(trace.get("accepted_moves"), int) or not 0 < trace["accepted_moves"] <= 8000:
        raise WorkerFailure("candidate_protocol_error: game turn/move bounds are invalid")
    card = trace.get("ethereal_usher")
    if not isinstance(card, dict) or card.get("collector_number") != 47 or card.get("mana_cost") != "{5}{U}" or card.get("mana_paid", []).count("Blue") < 1 or len(card.get("mana_paid", [])) != 6 or card.get("base_power") != 2 or card.get("base_toughness") != 3 or card.get("subtype_asserted") is not False:
        raise WorkerFailure("candidate_protocol_error: Ethereal Usher identity, payment, body, or Spirit model boundary is invalid")
    transmute = trace.get("transmute")
    if not isinstance(transmute, dict) or transmute.get("mana_cost") != "{1}{U}{U}" or transmute.get("source") == transmute.get("selected") or transmute.get("source_definition") != "RAV-ETHEREAL-USHER" or transmute.get("found_definition") != "RAV-ETHEREAL-USHER" or transmute.get("destination") != "Hand" or transmute.get("source_zone_after_cost") != "Graveyard" or transmute.get("sorcery_timing") is not True or transmute.get("private_choice") is not True or transmute.get("hidden_from_opponent") is not True or transmute.get("revealed") is not True or transmute.get("library_shuffled") is not True or transmute.get("resolved") is not True or transmute.get("same_found_object_cast") is not True:
        raise WorkerFailure("candidate_protocol_error: private exact-cost Transmute search/reveal/hand/shuffle trace is incomplete")
    activation = trace.get("activation")
    if not isinstance(activation, dict) or activation.get("mana_cost") != "{U}" or activation.get("tap_cost_paid") is not True or activation.get("target_definition") != "RAV-WATCHWOLF" or "Unblockable" not in activation.get("target_keywords", []) or activation.get("unblockable_at_attack") is not True or activation.get("unblockable_expired_next_turn") is not True or activation.get("target_was_blocked") is not False or activation.get("illegal_block_diagnostic") != "unblockable attacker cannot be blocked":
        raise WorkerFailure("candidate_protocol_error: target-bound unblockable activation, duration, or illegal-block evidence is incomplete")
    combat = trace.get("combat")
    if not isinstance(combat, dict) or not isinstance(combat.get("direct_damage_total"), int) or combat["direct_damage_total"] < 20 or combat.get("opponent_life", 1) > 0 or not isinstance(combat.get("life_at_terminal"), list) or len(combat["life_at_terminal"]) != 2 or combat["life_at_terminal"][0] <= 0 or combat["life_at_terminal"][1] > 0:
        raise WorkerFailure("candidate_protocol_error: target Watchwolf combat and terminal life totals are inconsistent")
    events = trace.get("events")
    if not isinstance(events, list) or not events or not all(isinstance(event, dict) for event in events):
        raise WorkerFailure("candidate_protocol_error: projected Ethereal Usher events are malformed")
    sequence = [event.get("sequence") for event in events]
    if any(not isinstance(value, int) for value in sequence) or sequence != sorted(sequence) or len(set(sequence)) != len(sequence):
        raise WorkerFailure("candidate_protocol_error: projected event sequence is invalid")
    required = {"AbilityManaPaid", "AbilityActivated", "AbilityResolved", "DecisionOpened", "DecisionCompleted", "CardRevealed", "LibrarySearchResolved", "LibraryShuffled", "Transmuted", "SpellCast", "ContinuousEffectCreated", "ContinuousEffectExpired", "AttackersDeclared", "BlockersDeclared", "DamageDealtToPlayer", "GameEnded"}
    if not required.issubset({event.get("kind") for event in events}):
        raise WorkerFailure("candidate_protocol_error: ordered event evidence does not cover all task mechanics and terminal state")
    ordered = trace.get("ordered_event_indices")
    if not isinstance(ordered, list) or len(ordered) != 25 or any(not isinstance(index, int) for index in ordered) or ordered != sorted(ordered) or len(set(ordered)) != len(ordered):
        raise WorkerFailure("candidate_protocol_error: authority's typed receipt order is missing or invalid")
    if sequence != ordered:
        raise WorkerFailure("candidate_protocol_error: projected receipts do not exactly match the authority's ordered indices")


def validate_report(report: dict) -> None:
    card = {"collector_number": 47, "definition_id": "RAV-ETHEREAL-USHER", "name": "Ethereal Usher"}
    if report.get("schema_version") != "cardbench.magic.card-pvp.check.v1" or report.get("task_id") != "cardbench/magic/card" or report.get("passed") is not True or report.get("pilot_card_passed") is not True or report.get("replay_deterministic") is not True or report.get("cards_passed") != 1 or report.get("cards_credited") != 0 or report.get("cards_total") != 291 or report.get("printings_total") != 306 or report.get("coverage_complete") is not False or report.get("full_reward") is not False or report.get("harbor_reward") != 0.0 or report.get("pilot_reward") != 0.0 or report.get("viewer_scope") != "Judge" or report.get("card") != card:
        raise WorkerFailure("candidate_protocol_error: incomplete Ethereal Usher zero-credit diagnostic report")
    source = report.get("candidate_source_validation")
    if not isinstance(source, dict) or source.get("canonical_normalized_source_checked") is not True or source.get("candidate_ast_contract_matched") is not True or source.get("compile_valid_injected_side_effect_fixture_compiled_by_construction") is not True or source.get("compile_valid_injected_side_effect_candidate_rejected") is not True or source.get("guard_ran_before_candidate_function_calls") is not True or source.get("injected_candidate_calls_before_rejection") != 0:
        raise WorkerFailure("candidate_protocol_error: pre-call normalized-source/AST side-effect attestation is incomplete")
    rules = report.get("rules_authority")
    if not isinstance(rules, dict) or rules.get("no_provider_trial") is not True or rules.get("authority_cutoff") != "2026-09-24" or "Spirit is retained identity only" not in rules.get("oracle_reconciliation", ""):
        raise WorkerFailure("candidate_protocol_error: source reconciliation, model limitation, or no-provider boundary is missing")
    traces = report.get("traces")
    if not isinstance(traces, list) or len(traces) != 1:
        raise WorkerFailure("candidate_protocol_error: exactly one Ethereal Usher full-game trace is required")
    validate_trace(traces[0])
    mutation = report.get("mutation_sensitivity")
    expected = ("unblockable_effect_mutation_detected_in_game", "activated_ability_cost_mutation_detected_in_game", "casting_cost_mutation_detected_in_game", "base_power_mutation_detected_in_game", "base_toughness_mutation_detected_in_game", "transmute_removed_mutation_detected_in_game", "transmute_cost_mutation_detected_in_game", "compile_valid_candidate_side_effect_rejected_before_execution")
    if not isinstance(mutation, dict) or any(mutation.get(key) is not True for key in expected) or mutation.get("sealed_mutation_authority") is not False:
        raise WorkerFailure("candidate_protocol_error: Ethereal Usher gameplay/security mutation controls are incomplete")
    rows = report.get("per_card_breakdown")
    if not isinstance(rows, list) or sum(1 for row in rows if isinstance(row, dict) and row.get("collector_number") == 47 and row.get("name") == "Ethereal Usher" and row.get("definition_id") == "RAV-ETHEREAL-USHER" and row.get("trace_status") == "passed" and row.get("credited") is False) != 1:
        raise WorkerFailure("candidate_protocol_error: Ethereal Usher zero-credit catalog row is missing")


def parse_report(stdout: bytes) -> dict:
    for line in reversed(stdout.decode(errors="replace").splitlines()):
        try:
            report = json.loads(line)
        except json.JSONDecodeError:
            continue
        if report.get("schema_version") == "cardbench.magic.card-pvp.check.v1":
            validate_report(report)
            return report
    raise WorkerFailure("candidate_protocol_error: no valid Ethereal Usher PvP report")


def score(workspace: Path, manifest_path: Path, output_root: Path, candidate_kind: str, timeout: int) -> int:
    output_root.mkdir(parents=True, exist_ok=True)
    manifest = verify_workspace(workspace.resolve(), manifest_path.resolve())
    registry = Path.home() / ".cargo" / "registry"
    if not registry.is_dir():
        raise FileNotFoundError(f"offline Cargo registry cache is missing: {registry}")
    if not shutil.which("docker"):
        raise FileNotFoundError("Docker is required for isolated Magic card scoring")
    image = os.environ.get("CARDBENCH_MAGIC_WORKER_IMAGE", IMAGE)
    if not (image.startswith("rust@sha256:") or image.startswith("sha256:")):
        raise ValueError("CARDBENCH_MAGIC_WORKER_IMAGE must be an immutable digest reference")
    container = f"cardbench-magic-ethereal-usher-{uuid.uuid4().hex[:12]}"
    with tempfile.TemporaryDirectory(prefix="ethereal-usher-candidate-") as temp:
        candidate = snapshot_workspace(workspace.resolve(), manifest["files"], Path(temp) / "workspace")
        authority = (REPO / AUTHORITY).resolve()
        command = sandbox_command(candidate, registry.resolve(), authority, container, image, candidate_kind)
        code, stdout, stderr = run_bounded(command, timeout, container)
    (output_root / "worker.stdout").write_bytes(stdout)
    (output_root / "worker.stderr").write_bytes(stderr)
    if code != 0:
        report = {"schema_version": "cardbench.magic.card-pvp.check.v1", "task_id": "cardbench/magic/card", "candidate": {"kind": candidate_kind, "source_sha256": sha256((workspace.resolve() / TARGET).read_bytes())}, "passed": False, "pilot_card_passed": False, "cards_passed": 0, "cards_total": 291, "printings_total": 306, "cards_credited": 0, "coverage_complete": False, "full_reward": False, "harbor_reward": 0.0, "pilot_reward": 0.0, "sealed_authority": False, "error": "candidate compile or Ethereal Usher trace gate failed", "diagnostics": stderr.decode(errors="replace")[-12000:]}
    else:
        report = parse_report(stdout)
        report["candidate"] = {"kind": candidate_kind, "card_definition": "RAV-ETHEREAL-USHER", "source_sha256": sha256((workspace.resolve() / TARGET).read_bytes()), "materialized_workspace_sha256": manifest["workspace_sha256"]}
        report["candidate_kind"] = candidate_kind
        report["starting_workspace_sha256"] = manifest["workspace_sha256"]
        report["sealed_authority"] = False
        report["worker_image"] = image
        report["sandbox"] = {"network": "none", "rootfs": "read-only", "pids_limit": 128}
        report["harbor_reward"] = 0.0
        report["full_reward"] = False
    (output_root / "result.json").write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    (output_root / "reward.txt").write_text("0.00000000\n", encoding="utf-8")
    return 0 if report.get("passed") is True else 1


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--workspace", required=True, type=Path)
    parser.add_argument("--manifest", required=True, type=Path)
    parser.add_argument("--output-root", required=True, type=Path)
    parser.add_argument("--candidate-kind", choices=("reference", "agent"), default="agent")
    parser.add_argument("--timeout", type=int, default=900)
    args = parser.parse_args()
    try:
        return score(args.workspace, args.manifest, args.output_root, args.candidate_kind, args.timeout)
    except Exception as error:
        args.output_root.mkdir(parents=True, exist_ok=True)
        failure = {"schema_version": "cardbench.magic.card-pvp.check.v1", "task_id": "cardbench/magic/card", "passed": False, "pilot_card_passed": False, "cards_passed": 0, "cards_total": 291, "printings_total": 306, "cards_credited": 0, "coverage_complete": False, "full_reward": False, "harbor_reward": 0.0, "pilot_reward": 0.0, "sealed_authority": False, "error": f"{type(error).__name__}: {error}"}
        (args.output_root / "result.json").write_text(json.dumps(failure, indent=2) + "\n", encoding="utf-8")
        (args.output_root / "reward.txt").write_text("0.00000000\n", encoding="utf-8")
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
