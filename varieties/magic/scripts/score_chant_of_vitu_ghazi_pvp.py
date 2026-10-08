#!/usr/bin/env python3
"""Score a candidate-owned Chant of Vitu-Ghazi module in an offline worker."""

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
MATERIALIZER = Path(__file__).with_name("materialize_chant_of_vitu_ghazi_task.py")
TARGET = Path("varieties/magic/sets/ravnica_city_of_guilds/src/cards/chant_of_vitu_ghazi.rs")
AUTHORITY_SOURCE = Path("varieties/magic/session/src/bin/rav_chant_of_vitu_ghazi_pvp.rs")
DEFAULT_WORKER_IMAGE = "rust@sha256:d7482085ff5b415f84dba5647ae71606650bdef00db7aeb69f4b3d170c3e4082"
MAX_OUTPUT_BYTES = 2 * 1024 * 1024


class WorkerFailure(RuntimeError):
    pass


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def fail(message: str) -> None:
    raise WorkerFailure(f"candidate_protocol_error: {message}")


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
                raise WorkerFailure("candidate_timeout: Docker worker exceeded its deadline")
            for key, _ in selector.select(0.2):
                chunk = os.read(key.fileobj.fileno(), 65536)
                if not chunk:
                    selector.unregister(key.fileobj)
                    continue
                destination = stdout if key.data == "stdout" else stderr
                destination.extend(chunk)
                if len(stdout) + len(stderr) > MAX_OUTPUT_BYTES:
                    raise WorkerFailure("candidate_output_limit: worker output exceeded 2 MiB")
        return process.wait(), bytes(stdout), bytes(stderr)
    except BaseException:
        process.kill()
        process.wait()
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
        "cp /trusted-authority.rs /work/varieties/magic/session/src/bin/rav_chant_of_vitu_ghazi_pvp.rs && "
        "cd /work && cargo run --offline --locked -j 2 --manifest-path varieties/magic/Cargo.toml "
        "-p cardbench-magic-session --bin rav-chant-of-vitu-ghazi-pvp --" + candidate_flag,
    ]


def validate_report(report: dict) -> None:
    if (
        report.get("schema_version") != "cardbench.magic.card-pvp.check.v1"
        or report.get("task_id") != "cardbench/magic/card"
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
        or report.get("deterministic_replay") is not True
        or report.get("card") != {"collector_number": 7, "definition_id": "RAV-CHANT-OF-VITU-GHAZI", "name": "Chant of Vitu-Ghazi"}
    ):
        fail("incomplete Chant diagnostic or incorrect 291-name/306-printing RAV denominator")
    candidate = report.get("candidate")
    if not isinstance(candidate, dict) or candidate.get("card_definition") != "RAV-CHANT-OF-VITU-GHAZI":
        fail("wrong Chant of Vitu-Ghazi candidate identity")
    mutations = report.get("mutation_sensitivity")
    expected_mutants = (
        "convoke_removed_mutation_detected",
        "creature_prevention_removed_mutation_detected",
        "combat_only_prevention_mutation_detected",
    )
    if not isinstance(mutations, dict) or any(mutations.get(key) is not True for key in expected_mutants) or mutations.get("sealed_mutation_authority") is not False:
        fail("compiled Convoke, prevention-removal, and combat-only mutants were not all rejected")
    traces = report.get("traces")
    if not isinstance(traces, list) or len(traces) != 1:
        fail("one terminal Chant full-game trace is required")
    trace = traces[0]
    if (
        not isinstance(trace, dict)
        or trace.get("trace_id") != "rav-chant-of-vitu-ghazi-convoke-prevent-creature-damage-gain-life-terminal"
        or trace.get("passed") is not True
        or trace.get("terminal") != "Winner(PlayerId(0))"
        or trace.get("winner_seat") != 0
        or trace.get("winner") != 0
        or trace.get("rejected_moves") != 0
        or not isinstance(trace.get("accepted_moves"), int)
        or trace["accepted_moves"] <= 0
        or not isinstance(trace.get("turns"), int)
        or trace["turns"] > 60
        or trace.get("spell_zone") != "Some(Graveyard)"
        or trace.get("global_prevention_expired") is not True
        or trace.get("decks") != ["rav_chant_vitu_ghazi_white_60", "rav_creature_damage_red_green_60"]
        or trace.get("policies") != ["rav.chant_of_vitu_ghazi-convoke-damage-prevention.v1", "rav.chant_of_vitu_ghazi-creature-damage-opponent.v1"]
    ):
        fail("Chant did not reach a clean terminal full-game result")
    spell = trace.get("spell")
    if not isinstance(spell, int) or trace.get("exact_payment_colors") != ["White"] * 7:
        fail("Chant must pay seven White mana in addition to one generic Convoke contribution")
    contributors = trace.get("convoke_contributors")
    if not isinstance(contributors, list) or len(contributors) != 1 or trace.get("convoke_contributors_tapped") != [True]:
        fail("one untapped creature must be tapped to Convoke one generic symbol")
    attackers = trace.get("combat_attackers")
    packets = trace.get("creature_damage_prevented")
    if not isinstance(attackers, list) or not attackers or not isinstance(packets, list) or len(packets) < 2:
        fail("full-game trace must include multiple prevented damage packets and combat attackers")
    sparkmage = trace.get("sparkmage_source")
    if not isinstance(sparkmage, int) or trace.get("sparkmage_noncombat_damage_prevented") != 1:
        fail("postcombat creature-sourced Sparkmage damage must be prevented")
    combat_packets = [packet for packet in packets if packet.get("source") in attackers]
    spark_packets = [packet for packet in packets if packet.get("source") == sparkmage and packet.get("target") == "Player(PlayerId(0))"]
    if not combat_packets or len(spark_packets) != 1 or spark_packets[0].get("amount") != 1:
        fail("prevention trace must include the opponent's combat damage and Sparkmage's noncombat damage")
    prevented = sum(packet.get("amount", 0) for packet in packets)
    gained = trace.get("life_gained_total")
    life = trace.get("life_at_terminal")
    installed = trace.get("life_when_prevention_installed")
    if (
        prevented != trace.get("prevented_damage_total")
        or prevented != gained
        or not isinstance(life, list)
        or len(life) != 2
        or not isinstance(installed, int)
        or life[0] - installed != gained
        or life[1] > 0
    ):
        fail("life gained must equal only the damage Chant actually prevented; seat zero must win")

    typed = trace.get("typed_event_trace")
    if not isinstance(typed, list) or len(typed) < 15:
        fail("typed event trace omitted cast, Convoke, replacement, prevention, life, or terminal receipts")
    if any(not isinstance(event, dict) or not isinstance(event.get("sequence"), int) for event in typed):
        fail("typed event sequence is malformed")
    sequences = [event["sequence"] for event in typed]
    if sequences != sorted(set(sequences)):
        fail("typed event sequence must be strictly increasing")
    by_sequence = {event["sequence"]: event for event in typed}
    facts = lambda sequence: by_sequence.get(sequence, {}).get("facts", {})
    kind = lambda sequence: by_sequence.get(sequence, {}).get("kind")
    sequences_by_kind: dict[str, list[int]] = {}
    for event in typed:
        sequences_by_kind.setdefault(event.get("kind", ""), []).append(event["sequence"])
    payment = next((seq for seq in sequences_by_kind.get("SpellManaPaid", []) if facts(seq).get("objects") == [spell]), None)
    cast = next((seq for seq in sequences_by_kind.get("SpellCast", []) if facts(seq).get("objects") == [spell]), None)
    resolved = next((seq for seq in sequences_by_kind.get("SpellResolved", []) if facts(seq).get("objects") == [spell]), None)
    moved = next((seq for seq in sequences_by_kind.get("CardMoved", []) if facts(seq).get("objects") == [spell] and facts(seq).get("zone") == "Graveyard"), None)
    ended = next((seq for seq in sequences_by_kind.get("GameEnded", []) if facts(seq).get("seats") == [0]), None)
    convoke = next((seq for seq in sequences_by_kind.get("ConvokeUsed", []) if facts(seq).get("seats") == [0] and facts(seq).get("objects") == contributors and facts(seq).get("convoke_contribution") == "generic"), None)
    created = next((seq for seq in sequences_by_kind.get("GlobalCreatureDamagePreventionCreated", []) if facts(seq).get("objects") == [spell] and facts(seq).get("seats") == [0] and facts(seq).get("replacement_id") == trace.get("prevention_record_id")), None)
    expired = next((seq for seq in sequences_by_kind.get("GlobalCreatureDamagePreventionExpired", []) if facts(seq).get("objects") == [spell] and facts(seq).get("seats") == [0] and facts(seq).get("replacement_id") == trace.get("prevention_record_id")), None)
    spark_cast = next((seq for seq in sequences_by_kind.get("SpellCast", []) if facts(seq).get("objects") == [sparkmage] and facts(seq).get("seats") == [1]), None)
    trigger = next((seq for seq in sequences_by_kind.get("TriggeredAbilityStacked", []) if facts(seq).get("objects") == [sparkmage] and facts(seq).get("seats") == [1] and facts(seq).get("ability") == "etb-deal-one"), None)
    if None in (payment, convoke, cast, created, resolved, moved, spark_cast, trigger, expired, ended):
        fail("typed payment, Convoke, cast, resolution, graveyard, effect lifetime, trigger, or end receipt is missing")
    if not (payment < cast < created < resolved < moved < spark_cast < trigger < expired < ended and convoke < payment):
        fail("typed spell payment/cast/resolution/move, Convoke, Sparkmage, expiry, and terminal events are out of order")
    if facts(payment).get("seats") != [0] or facts(payment).get("amount") != 7 or facts(payment).get("mana_colors") != ["White"] * 7 or facts(cast).get("seats") != [0] or facts(resolved).get("objects") != [spell] or facts(moved).get("objects") != [spell] or facts(ended).get("seats") != [0]:
        fail("typed Chant spell receipts do not bind the exact card, seat, payment, graveyard move, and winner")

    for packet in packets:
        replacement = packet.get("replacement_event_sequence")
        damage = packet.get("damage_prevented_event_sequence")
        prevention = packet.get("prevention_event_sequence")
        life_gain = packet.get("life_gain_event_sequence")
        if not all(isinstance(value, int) for value in (replacement, damage, prevention, life_gain)) or not replacement < damage < prevention < life_gain:
            fail("each damage packet needs ordered replacement, damage-prevented, creature-prevented, and life-gain events")
        if (
            kind(replacement) != "DamageReplacementApplied"
            or kind(damage) != "DamagePrevented"
            or kind(prevention) != "CreatureDamagePrevented"
            or kind(life_gain) != "LifeGained"
            or facts(replacement).get("replacement") != "creature_damage_prevention"
            or facts(replacement).get("replacement_id") != trace.get("prevention_record_id")
            or facts(replacement).get("objects") != [spell]
            or facts(replacement).get("target_player") != 0
            or facts(damage).get("objects", [None])[0] != packet.get("source")
            or facts(damage).get("target_player") != 0
            or facts(damage).get("amount") != packet.get("amount")
            or facts(prevention).get("objects") != [packet.get("source"), spell]
            or facts(prevention).get("replacement_id") != trace.get("prevention_record_id")
            or facts(prevention).get("target_player") != 0
            or facts(prevention).get("amount") != packet.get("amount")
            or facts(life_gain).get("seats") != [0]
            or facts(life_gain).get("amount") != packet.get("amount")
        ):
            fail("typed damage-prevention receipt fields do not bind source, player, record, amount, and life gain")
    canonical = trace.get("canonical_event_trace")
    if not isinstance(canonical, list) or len(canonical) != len(typed) or any(a.get("sequence") != b.get("sequence") or a.get("canonical") != b.get("canonical") for a, b in zip(canonical, typed)):
        fail("compact canonical event trace does not match typed event sequence")


def parse_report(stdout: bytes) -> dict:
    for line in reversed(stdout.decode(errors="replace").splitlines()):
        try:
            report = json.loads(line)
        except json.JSONDecodeError:
            continue
        if report.get("schema_version") == "cardbench.magic.card-pvp.check.v1":
            validate_report(report)
            return report
    fail("no valid Chant of Vitu-Ghazi PvP result")


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
        raise FileNotFoundError(f"trusted Chant of Vitu-Ghazi authority source is missing: {authority}")
    if not shutil.which("docker"):
        raise FileNotFoundError("Docker is required for Magic candidate isolation")
    verify_workspace(workspace, manifest)
    scoring_workspace = make_scoring_snapshot(workspace, manifest_data["files"], output_root / "candidate-workspace")
    candidate_hash = sha256((scoring_workspace / TARGET).read_bytes())
    name = f"cardbench-magic-chant-vitu-ghazi-{uuid.uuid4().hex[:16]}"
    command = sandbox_command(scoring_workspace, cargo_registry.resolve(), authority, name, worker_image, candidate_kind)
    code, stdout, stderr = run_bounded(command, timeout=900, name=name)
    (output_root / "worker.stdout").write_bytes(stdout)
    (output_root / "worker.stderr").write_bytes(stderr)
    if code != 0:
        result = {
            "schema_version": "cardbench.magic.card-pvp.check.v1",
            "task_id": "cardbench/magic/card",
            "candidate": {"kind": candidate_kind, "source_sha256": candidate_hash},
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
            "sealed_authority": False,
            "worker_image": worker_image,
            "sandbox": {"network": "none", "rootfs": "read-only", "pids_limit": 128},
            "error": "candidate compile or Chant of Vitu-Ghazi reference trace failed",
            "diagnostics": stderr.decode(errors="replace")[-12000:],
        }
    else:
        result = parse_report(stdout)
        result["candidate"] = {
            "kind": candidate_kind,
            "card_definition": "RAV-CHANT-OF-VITU-GHAZI",
            "source_sha256": candidate_hash,
            "materialized_workspace_sha256": manifest_data["workspace_sha256"],
        }
        result["candidate_kind"] = candidate_kind
        result["starting_workspace_sha256"] = manifest_data["workspace_sha256"]
        result["sealed_authority"] = False
        result["worker_image"] = worker_image
        result["sandbox"] = {"network": "none", "rootfs": "read-only", "pids_limit": 128}
        result["harbor_reward"] = 0.0
        result["pilot_reward"] = 0.0
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
            "sealed_authority": False,
            "error": str(error),
        }
        (args.output_root / "result.json").write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
        (args.output_root / "reward.txt").write_text("0.0\n", encoding="utf-8")
        print(f"Chant of Vitu-Ghazi candidate verifier failed: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
