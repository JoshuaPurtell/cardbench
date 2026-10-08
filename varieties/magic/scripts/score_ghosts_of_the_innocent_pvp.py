#!/usr/bin/env python3
"""Score an isolated Ghosts of the Innocent submission against trusted PvP traces."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import shutil
import uuid
from pathlib import Path

from materialize_ghosts_of_the_innocent_task import verify
from score_watchwolf_pvp import WorkerFailure, make_scoring_snapshot, run_bounded

REPO = Path(__file__).resolve().parents[3]
AUTHORITY_SOURCE = Path("varieties/magic/session/src/bin/rav_ghosts_of_the_innocent_pvp.rs")
TARGET = Path("varieties/magic/sets/ravnica_city_of_guilds/src/cards/ghosts_of_the_innocent.rs")
DEFAULT_WORKER_IMAGE = "rust@sha256:d7482085ff5b415f84dba5647ae71606650bdef00db7aeb69f4b3d170c3e4082"


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def validate_report(report: dict) -> None:
    expected_card = {"collector_number": 20, "definition_id": "RAV-GHOSTS-OF-THE-INNOCENT", "name": "Ghosts of the Innocent"}
    if (
        report.get("schema_version") != "cardbench.magic.card-pvp.check.v1"
        or report.get("task_id") != "cardbench/magic/card"
        or report.get("passed") is not True
        or report.get("pilot_card_passed") is not True
        or report.get("deterministic_replay") is not True
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
    ):
        raise WorkerFailure("candidate_protocol_error: incomplete Ghosts of the Innocent report")
    candidate = report.get("candidate")
    if not isinstance(candidate, dict) or candidate.get("card_definition") != expected_card["definition_id"]:
        raise WorkerFailure("candidate_protocol_error: wrong Ghosts candidate identity")
    traces = report.get("traces")
    if not isinstance(traces, list) or len(traces) != 1:
        raise WorkerFailure("candidate_protocol_error: one complete Ghosts duel trace is required")
    trace = traces[0]
    if not isinstance(trace, dict) or trace.get("passed") is not True:
        raise WorkerFailure("candidate_protocol_error: Ghosts trace did not pass")
    if (
        trace.get("winner_seat") != 0
        or trace.get("terminal") != "Winner(PlayerId(0))"
        or trace.get("rejected_moves") != 0
        or not isinstance(trace.get("seed"), int)
        or not isinstance(trace.get("turns"), int)
        or trace["turns"] <= 0
        or not isinstance(trace.get("accepted_moves"), int)
        or trace["accepted_moves"] <= 0
        or trace.get("policies") != ["rav.ghosts-of-the-innocent-pilot.v1", "rav.ghosts-of-the-innocent-opponent.v1"]
        or trace.get("decks") != ["rav_ghosts_white_midrange", "rav_char_red_control"]
    ):
        raise WorkerFailure("candidate_protocol_error: Ghosts result is not a legal terminal two-player duel")
    mechanics = trace.get("mechanics")
    if not isinstance(mechanics, dict) or (
        mechanics.get("ghost_mana_cost") != "{5}{W}{W}"
        or mechanics.get("ghost_power") != 4
        or mechanics.get("ghost_toughness") != 5
        or mechanics.get("replacement_source") != expected_card["definition_id"]
        or mechanics.get("creature_packet_halved") is not True
        or mechanics.get("player_packet_halved") is not True
        or mechanics.get("self_packet_halved") is not True
        or mechanics.get("combat_packet_halved") is not True
    ):
        raise WorkerFailure("candidate_protocol_error: Ghosts identity, body, or global replacement differs")
    packets = mechanics.get("replacement_packets")
    if not isinstance(packets, list) or len(packets) != 5:
        raise WorkerFailure("candidate_protocol_error: five independent damage packet receipts are required")
    expected_amounts = [(4, 2), (2, 1), (4, 2), (2, 1), (3, 1)]
    for packet, (original, replacement) in zip(packets, expected_amounts, strict=True):
        if not isinstance(packet, dict) or (
            packet.get("original_amount") != original
            or packet.get("replacement_amount") != replacement
            or packet.get("committed_amount") != replacement
            or not isinstance(packet.get("replacement_index"), int)
            or not isinstance(packet.get("committed_index"), int)
            or packet["replacement_index"] >= packet["committed_index"]
        ):
            raise WorkerFailure("candidate_protocol_error: a Ghosts packet was not halved and committed in order")
    mutations = report.get("mutation_sensitivity")
    required = (
        "printed_cost_mutation_detected_in_game",
        "printed_toughness_mutation_detected_in_game",
        "missing_binding_mutation_detected_in_game",
        "wrong_replacement_mutation_detected_in_game",
    )
    if not isinstance(mutations, dict) or any(mutations.get(key) is not True for key in required):
        raise WorkerFailure("candidate_protocol_error: Ghosts cost, body, and replacement mutations must be detected")


def parse_report(stdout: bytes) -> dict:
    for line in reversed(stdout.decode(errors="replace").splitlines()):
        try:
            value = json.loads(line)
        except json.JSONDecodeError:
            continue
        if isinstance(value, dict) and value.get("schema_version") == "cardbench.magic.card-pvp.check.v1":
            validate_report(value)
            return value
    raise WorkerFailure("candidate_protocol_error: no valid Ghosts PvP result")


def sandbox_command(workspace: Path, registry: Path, authority: Path, name: str, image: str, candidate_kind: str) -> list[str]:
    if authority.is_symlink() or not authority.is_file():
        raise WorkerFailure("trusted Ghosts authority source is missing")
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
        "cp /trusted-authority.rs /work/varieties/magic/session/src/bin/rav_ghosts_of_the_innocent_pvp.rs && "
        "cd /work && cargo run --offline --locked -j 2 --manifest-path varieties/magic/Cargo.toml "
        "-p cardbench-magic-session --bin rav-ghosts-of-the-innocent-pvp --" + candidate_flag,
    ]


def score(workspace: Path, manifest: Path, output_root: Path, candidate_kind: str) -> int:
    output_root.mkdir(parents=True, exist_ok=True)
    manifest_data = json.loads(manifest.read_text(encoding="utf-8"))
    image = os.environ.get("CARDBENCH_MAGIC_WORKER_IMAGE", DEFAULT_WORKER_IMAGE)
    if not (image.startswith("rust@sha256:") or image.startswith("sha256:")):
        raise ValueError("CARDBENCH_MAGIC_WORKER_IMAGE must be an immutable digest reference")
    registry = Path.home() / ".cargo" / "registry"
    if not registry.is_dir():
        raise FileNotFoundError(f"offline Cargo registry cache is missing: {registry}")
    authority = REPO / AUTHORITY_SOURCE
    if authority.is_symlink() or not authority.is_file():
        raise FileNotFoundError(f"trusted Ghosts authority is missing: {authority}")
    if not shutil.which("docker"):
        raise FileNotFoundError("Docker is required for Magic candidate isolation")
    verify(workspace, manifest)
    snapshot = make_scoring_snapshot(workspace, manifest_data["files"], output_root / "candidate-workspace")
    candidate_hash = sha256((snapshot / TARGET).read_bytes())
    name = f"cardbench-magic-ghosts-of-the-innocent-{uuid.uuid4().hex[:16]}"
    command = sandbox_command(snapshot, registry.resolve(), authority, name, image, candidate_kind)
    code, stdout, stderr = run_bounded(command, timeout=900, name=name)
    (output_root / "worker.stdout").write_bytes(stdout)
    (output_root / "worker.stderr").write_bytes(stderr)
    if code != 0:
        report = {
            "schema_version": "cardbench.magic.card-pvp.check.v1",
            "task_id": "cardbench/magic/card",
            "passed": False,
            "pilot_card_passed": False,
            "cards_passed": 0,
            "cards_total": 291,
            "printings_total": 306,
            "cards_credited": 0,
            "coverage_complete": False,
            "full_reward": False,
            "harbor_reward": 0.0,
            "pilot_reward": 0.0,
            "error": "candidate compile or Ghosts trusted trace failed",
            "diagnostics": stderr.decode(errors="replace")[-12000:],
        }
    else:
        report = parse_report(stdout)
        report["candidate"] = {"kind": candidate_kind, "card_definition": "RAV-GHOSTS-OF-THE-INNOCENT", "source_sha256": candidate_hash, "materialized_workspace_sha256": manifest_data["workspace_sha256"]}
        report["candidate_kind"] = candidate_kind
        report["starting_workspace_sha256"] = manifest_data["workspace_sha256"]
        report["sealed_authority"] = False
        report["worker_image"] = image
        report["sandbox"] = {"network": "none", "rootfs": "read-only", "pids_limit": 128}
        report["harbor_reward"] = 0.0
        report["pilot_reward"] = 0.0
        report["full_reward"] = False
    (output_root / "result.json").write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    (output_root / "reward.txt").write_text("0.00000000\n", encoding="utf-8")
    print(f"candidate_passed={report.get('pilot_card_passed', False)} cards={report.get('cards_passed', 0)}/291 coverage_complete=false")
    return 0 if report.get("passed") is True and report.get("pilot_card_passed") is True else 1


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--workspace", required=True, type=Path)
    parser.add_argument("--manifest", required=True, type=Path)
    parser.add_argument("--output-root", required=True, type=Path)
    parser.add_argument("--candidate-kind", choices=["reference", "agent"], default="agent")
    args = parser.parse_args()
    return score(args.workspace.resolve(), args.manifest.resolve(), args.output_root.resolve(), args.candidate_kind)


if __name__ == "__main__":
    raise SystemExit(main())
