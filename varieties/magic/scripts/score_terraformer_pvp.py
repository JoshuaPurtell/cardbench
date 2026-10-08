#!/usr/bin/env python3
"""Run and validate the isolated Terraformer reference/candidate authority."""

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
MATERIALIZER = Path(__file__).with_name("materialize_terraformer_task.py")
TARGET = Path("varieties/magic/sets/ravnica_city_of_guilds/src/cards/terraformer.rs")
AUTHORITY_SOURCE = Path("varieties/magic/session/src/bin/rav_terraformer_pvp.rs")
DEFAULT_WORKER_IMAGE = "rust@sha256:d7482085ff5b415f84dba5647ae71606650bdef00db7aeb69f4b3d170c3e4082"
MAX_OUTPUT_BYTES = 2 * 1024 * 1024


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
        "cp /trusted-authority.rs /work/varieties/magic/session/src/bin/rav_terraformer_pvp.rs && "
        "cd /work && cargo run --offline --locked -j 2 --manifest-path varieties/magic/Cargo.toml "
        "-p cardbench-magic-session --bin rav-terraformer-pvp --" + candidate_flag,
    ]


def validate_report(report: dict) -> None:
    expected_card = {"collector_number": 70, "definition_id": "RAV-TERRAFORMER", "name": "Terraformer", "printings": 1}
    if (
        report.get("schema_version") != "cardbench.magic.card-pvp.check.v1"
        or report.get("task_id") != "cardbench/magic/card"
        or report.get("expansion") != "RAV"
        or report.get("passed") is not True
        or report.get("pilot_card_passed") is not True
        or report.get("cards_passed") != 1
        or report.get("cards_credited") != 0
        or report.get("cards_total") != 291
        or report.get("printings_total") != 306
        or report.get("coverage_complete") is not False
        or report.get("full_reward") is not False
        or report.get("harbor_reward") != 0.0
        or report.get("pilot_reward") != 0.0
        or report.get("viewer_scope") != "Judge"
        or report.get("card") != expected_card
        or report.get("deterministic_replay") is not True
    ):
        raise WorkerFailure("candidate_protocol_error: incomplete Terraformer single-card report")
    traces = report.get("traces")
    if not isinstance(traces, list) or len(traces) != 1:
        raise WorkerFailure("candidate_protocol_error: exactly one Terraformer trace is required")
    trace = traces[0]
    if (
        not isinstance(trace, dict)
        or trace.get("trace_id") != "rav-terraformer-chosen-basic-type-intrinsic-mana-full-game"
        or trace.get("passed") is not True
        or trace.get("terminal") != "Winner(PlayerId(0))"
        or trace.get("winner_seat") != 0
        or not isinstance(trace.get("turns"), int)
        or trace["turns"] <= 0
        or trace.get("rejected_moves") != 0
    ):
        raise WorkerFailure("candidate_protocol_error: Terraformer duel did not reach a clean terminal pilot win")
    terraformer = trace.get("terraformer")
    if (
        not isinstance(terraformer, dict)
        or terraformer.get("collector_number") != 70
        or terraformer.get("mana_cost") != "{2}{U}"
        or len(terraformer.get("mana_paid", [])) != 3
        or "Blue" not in terraformer["mana_paid"]
        or terraformer.get("power") != 2
        or terraformer.get("toughness") != 2
        or terraformer.get("activated_ability_id") != "choose-controller-land-basic-type"
        or terraformer.get("activation_count") != 2
    ):
        raise WorkerFailure("candidate_protocol_error: Terraformer definition, casting, or activated identity is incomplete")
    activations = trace.get("activations")
    if not isinstance(activations, list) or len(activations) != 2:
        raise WorkerFailure("candidate_protocol_error: exactly two Terraformer activations are required")
    source = terraformer.get("object")
    source_incarnation = None
    target_counts = []
    for ordinal, activation in enumerate(activations):
        if not isinstance(activation, dict):
            raise WorkerFailure("candidate_protocol_error: activation record is invalid")
        before = activation.get("controller_lands_before")
        after = activation.get("controller_lands_after")
        opposing_before = activation.get("opponent_lands_before")
        opposing_after = activation.get("opponent_lands_after")
        restored = activation.get("controller_lands_restored")
        opposing_restored = activation.get("opponent_lands_restored")
        effect_targets = activation.get("effect_targets")
        expiry_targets = activation.get("expiry_targets")
        if (
            activation.get("active_player") != (1 if ordinal == 0 else 0)
            or activation.get("step") != "PrecombatMain"
            or activation.get("ability_id") != "choose-controller-land-basic-type"
            or activation.get("source") != source
            or not isinstance(activation.get("source_incarnation"), int)
            or activation.get("resolved_source_incarnation") != activation.get("source_incarnation")
            or (source_incarnation is not None and activation.get("source_incarnation") != source_incarnation)
            or activation.get("mana_cost") != "{1}"
            or activation.get("chosen_type") != "Forest"
            or not isinstance(before, list) or not before
            or any(land.get("definition") != "RAV-ISLAND" or land.get("basic_land_type") != "Island" for land in before)
            or [land.get("object") for land in before] != effect_targets
            or len(after or []) != len(before)
            or [(land.get("object"), land.get("definition")) for land in after] != [(land.get("object"), land.get("definition")) for land in before]
            or any(land.get("basic_land_type") != "Forest" for land in after)
            or not isinstance(restored, list)
            or [(land.get("object"), land.get("basic_land_type")) for land in restored]
            != [(land.get("object"), land.get("basic_land_type")) for land in before]
            or not isinstance(opposing_before, list) or not isinstance(opposing_after, list)
            or [(land.get("object"), land.get("basic_land_type")) for land in opposing_before]
            != [(land.get("object"), land.get("basic_land_type")) for land in opposing_after]
            or [(land.get("object"), land.get("basic_land_type")) for land in opposing_before]
            != [(land.get("object"), land.get("basic_land_type")) for land in opposing_restored]
            or sorted(effect_targets or []) != sorted(expiry_targets or [])
        ):
            raise WorkerFailure(f"candidate_protocol_error: Terraformer activation {ordinal} lost exact identity, scope, or expiry evidence")
        source_incarnation = activation["source_incarnation"]
        green_land = activation.get("former_island_that_made_green")
        if green_land not in [land.get("object") for land in before]:
            raise WorkerFailure(f"candidate_protocol_error: activation {ordinal} did not produce Green from its converted Island")
        target_counts.append(len(effect_targets))
    if activations[0]["turn"] >= activations[1]["turn"]:
        raise WorkerFailure("candidate_protocol_error: opponent-turn activation must precede the controller-turn activation")
    mana_proof = trace.get("intrinsic_mana_proof")
    if not isinstance(mana_proof, dict) or mana_proof.get("source_land_definition") != "RAV-ISLAND" or mana_proof.get("chosen_color") != "Green" or mana_proof.get("during_each_activation") is not True:
        raise WorkerFailure("candidate_protocol_error: intrinsic converted-Island Green mana proof is missing")
    followup = trace.get("followup_creature")
    if not isinstance(followup, dict) or followup.get("definition") != "RAV-GREATER-MOSSDOG" or followup.get("mana_cost") != "{3}{G}" or len(followup.get("mana_paid", [])) != 4 or followup["mana_paid"].count("Green") < 1 or followup.get("resolved") is not True:
        raise WorkerFailure("candidate_protocol_error: Greater Mossdog was not paid for and resolved with converted Green")
    combat = trace.get("combat")
    if not isinstance(combat, dict) or len(combat.get("attacks_by_mossdog", [])) < 5 or combat.get("damage_to_opponent", 0) < 20 or combat.get("life_at_terminal", [0, 1])[0] <= 0 or combat["life_at_terminal"][1] > 0:
        raise WorkerFailure("candidate_protocol_error: terminal combat evidence is incomplete")
    events, order = trace.get("events"), trace.get("ordered_event_sequence")
    if not isinstance(events, list) or not isinstance(order, list) or not events:
        raise WorkerFailure("candidate_protocol_error: ordered event receipts are missing")
    sequences = [event.get("sequence") for event in events]
    if any(not isinstance(value, int) for value in sequences) or sequences != sorted(sequences) or order != sequences:
        raise WorkerFailure("candidate_protocol_error: event sequence is not strictly ordered")
    kinds = [event.get("kind") for event in events]
    expected_counts = {
        "AbilityManaPaid": 2, "AbilityActivated": 2, "AbilityResolved": 2,
        "ContinuousEffectCreated": sum(target_counts), "ContinuousEffectExpired": sum(target_counts),
        "ManaAbilityActivated": 2, "SpellManaPaid": 2, "SpellCast": 2,
        "SpellResolved": 2, "GameEnded": 1,
    }
    if any(kinds.count(kind) != count for kind, count in expected_counts.items()):
        raise WorkerFailure("candidate_protocol_error: required activation, type-change, Green mana, spell, and terminal receipts are incomplete")
    canonical = [event.get("canonical", "") for event in events]
    for ordinal, activation in enumerate(activations):
        land_ids = activation["effect_targets"]
        green_land = activation["former_island_that_made_green"]
        if not any(event.get("kind") == "AbilityManaPaid" and f"source: ObjectId({source})" in event.get("canonical", "") and 'ability: "choose-controller-land-basic-type"' in event.get("canonical", "") and "generic: 1" in event.get("canonical", "") for event in events):
            raise WorkerFailure(f"candidate_protocol_error: activation {ordinal} lacks its exact generic-one payment")
        if not any(event.get("kind") == "AbilityActivated" and f"source: ObjectId({source})" in event.get("canonical", "") and f"source_incarnation: {activation['source_incarnation']}" in event.get("canonical", "") and 'ability: "choose-controller-land-basic-type"' in event.get("canonical", "") for event in events):
            raise WorkerFailure(f"candidate_protocol_error: activation {ordinal} lacks its source-incarnation receipt")
        if not any(event.get("kind") == "AbilityResolved" and f"source: ObjectId({source})" in event.get("canonical", "") and f"source_incarnation: {activation['source_incarnation']}" in event.get("canonical", "") for event in events):
            raise WorkerFailure(f"candidate_protocol_error: activation {ordinal} lacks its resolution receipt")
        for kind, ids in (("ContinuousEffectCreated", land_ids), ("ContinuousEffectExpired", land_ids)):
            receipts = [event.get("canonical", "") for event in events if event.get("kind") == kind and f"source: ObjectId({source})" in event.get("canonical", "")]
            if any(not any(f"target: ObjectId({target})" in receipt and "layer: Type" in receipt for receipt in receipts) for target in ids):
                raise WorkerFailure(f"candidate_protocol_error: activation {ordinal} lacks a per-land {kind} receipt")
        if not any(event.get("kind") == "ManaAbilityActivated" and f"land: ObjectId({green_land})" in event.get("canonical", "") and "color: Green" in event.get("canonical", "") for event in events):
            raise WorkerFailure(f"candidate_protocol_error: activation {ordinal} lacks Green mana from its converted Island")
    mossdog_object = followup.get("object")
    terraformer_receipts = [event for event in events if event.get("kind") in {"SpellManaPaid", "SpellCast", "SpellResolved"} and f"card: ObjectId({source})" in event.get("canonical", "")]
    mossdog_receipts = [event for event in events if event.get("kind") in {"SpellManaPaid", "SpellCast", "SpellResolved"} and f"card: ObjectId({mossdog_object})" in event.get("canonical", "")]
    if len(terraformer_receipts) != 3 or len(mossdog_receipts) != 3 or not any("colors: [Blue" in event.get("canonical", "") for event in terraformer_receipts) or not any("colors: [Green, Green, Green, Green]" in event.get("canonical", "") for event in mossdog_receipts):
        raise WorkerFailure("candidate_protocol_error: exact Terraformer and Greater Mossdog spell receipts are incomplete")
    if not any("Some(PlayerId(0))" in event for event in canonical if event.startswith("GameEnded")):
        raise WorkerFailure("candidate_protocol_error: terminal win receipt does not identify the pilot")
    mutations = report.get("mutation_sensitivity")
    required_mutations = (
        "ability_removed_mutation_detected_in_game",
        "generic_activation_cost_changed_mutation_detected_in_game",
        "source_tap_cost_added_mutation_detected_in_game",
        "instant_speed_activation_removed_mutation_detected_in_game",
        "chosen_land_type_effect_changed_mutation_detected_in_game",
        "printed_body_changed_mutation_detected_in_game",
    )
    if not isinstance(mutations, dict) or any(mutations.get(key) is not True for key in required_mutations):
        raise WorkerFailure("candidate_protocol_error: all six Terraformer gameplay mutants must be detected")


def parse_report(stdout: bytes) -> dict:
    for line in reversed(stdout.decode(errors="replace").splitlines()):
        try:
            value = json.loads(line)
        except json.JSONDecodeError:
            continue
        if value.get("schema_version") == "cardbench.magic.card-pvp.check.v1":
            validate_report(value)
            return value
    raise WorkerFailure("candidate_protocol_error: no valid Terraformer PvP result")


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
        raise FileNotFoundError(f"trusted Terraformer authority source is missing: {authority}")
    if not shutil.which("docker"):
        raise FileNotFoundError("Docker is required for Magic candidate isolation")
    verify_workspace(workspace, manifest)
    scoring_workspace = make_scoring_snapshot(workspace, manifest_data["files"], output_root / "candidate-workspace")
    candidate_hash = sha256((scoring_workspace / TARGET).read_bytes())
    name = f"cardbench-magic-terraformer-{uuid.uuid4().hex[:16]}"
    command = sandbox_command(scoring_workspace, cargo_registry.resolve(), authority, name, worker_image, candidate_kind)
    code, stdout, stderr = run_bounded(command, timeout=900, name=name)
    (output_root / "worker.stdout").write_bytes(stdout)
    (output_root / "worker.stderr").write_bytes(stderr)
    if code != 0:
        result = {
            "schema_version": "cardbench.magic.card-pvp.check.v1", "task_id": "cardbench/magic/card",
            "candidate": {"kind": candidate_kind, "source_sha256": candidate_hash}, "passed": False,
            "pilot_card_passed": False, "cards_passed": 0, "cards_total": 291, "printings_total": 306,
            "cards_credited": 0, "coverage_complete": False, "full_reward": False,
            "harbor_reward": 0.0, "pilot_reward": 0.0, "sealed_authority": False,
            "worker_image": worker_image, "sandbox": {"network": "none", "rootfs": "read-only", "pids_limit": 128},
            "error": "candidate compile or Terraformer reference trace failed",
            "diagnostics": stderr.decode(errors="replace")[-12000:],
        }
    else:
        result = parse_report(stdout)
        result["candidate"] = {
            "kind": candidate_kind, "card_definition": "RAV-TERRAFORMER", "source_sha256": candidate_hash,
            "materialized_workspace_sha256": manifest_data["workspace_sha256"],
        }
        result["candidate_kind"] = candidate_kind
        result["starting_workspace_sha256"] = manifest_data["workspace_sha256"]
        result["sealed_authority"] = False
        result["worker_image"] = worker_image
        result["sandbox"] = {"network": "none", "rootfs": "read-only", "pids_limit": 128}
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
        result = {
            "schema_version": "cardbench.magic.card-pvp.check.v1", "task_id": "cardbench/magic/card",
            "passed": False, "pilot_card_passed": False, "cards_passed": 0, "cards_total": 291,
            "printings_total": 306, "cards_credited": 0, "coverage_complete": False,
            "full_reward": False, "harbor_reward": 0.0, "sealed_authority": False, "error": str(error),
        }
        (args.output_root / "result.json").write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
        (args.output_root / "reward.txt").write_text("0.0\n", encoding="utf-8")
        print(f"Magic candidate verifier failed: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
