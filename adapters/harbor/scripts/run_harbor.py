#!/usr/bin/env python3
"""Unified CardBench Harbor entry point for reference and Codex runs."""

from __future__ import annotations

import argparse
import base64
import datetime as dt
import hashlib
import importlib
import json
import os
import shutil
import subprocess
import sys
import tomllib
from pathlib import Path

REPO = Path(__file__).resolve().parents[3]
POKEMON = REPO / "varieties" / "pokemon"
MAGIC = REPO / "varieties" / "magic"
EVALS = Path(os.environ.get("CARDBENCH_EVALS_ROOT", Path.home() / "Documents" / "GitHub" / "evals"))
CODEX_RUNNER = EVALS / "core" / "harbor" / "runner" / "codex_harbor_runner.py"
# The oracle the `verify` lane submits as a candidate. It must be at least as
# strong as the roster's ranking origin, or the lane reports a loss for the
# reference solution itself.
#
# CURRENTLY UNSATISFIABLE, DELIBERATELY. The origin is now reference_policy_v2
# (64.9% on the train surface); v1 measures 35.6% and lost to it by 29 points.
# Pointing here at v2 makes oracle == origin, so the lane reports delta 0.0 --
# an honest "no lift" rather than a wrong "the reference is worse than the
# baseline". The lane stays red until a genuinely stronger policy exists; the
# one-ply greedy ladder plateaus around 65%, so that needs a better design, not
# a tuned copy. Do NOT resolve this by weakening the origin back to v1: an
# incompetent origin makes every candidate look good and the benchmark stops
# measuring anything.
REFERENCE_POLICY = POKEMON / "candidates" / "reference" / "reference_policy_v2.rs"
REFERENCE_DECK = POKEMON / "decks" / "gardevoir_delta_control.json"
CARDS = POKEMON / "cards"
SEALED = Path(os.environ.get("CARDBENCH_SEALED_ROOT", CARDS / ".sealed"))

FAMILIES = {
    "code-policy": "code_policy",
    "code_policy": "code_policy",
    "deck-opt": "deck_opt",
    "deck_opt": "deck_opt",
    "card": "card",
    "set-engine": "set_engine",
    "set_engine": "set_engine",
    "set-pvp": "set_pvp",
    "set_pvp": "set_pvp",
    "engine": "engine",
    "full-engine": "full_engine",
    "full_engine": "full_engine",
    "react": "react",
    "cybernetic": "cybernetic",
}


def fresh_output(family: str, command: str) -> Path:
    configured = os.environ.get("CARDBENCH_HARBOR_OUT")
    if configured:
        return Path(configured).expanduser().resolve()
    stamp = dt.datetime.now(dt.UTC).strftime("%Y%m%dT%H%M%SZ")
    return REPO / "artifacts" / "harbor" / f"{family}-{command}-{stamp}-{os.getpid()}"


def magic_card_task_config(instance: str) -> tuple[dict, Path]:
    """Resolve a single-card Harbor task from its task-local manifest."""
    tasks = MAGIC / "tasks"
    matches = []
    for manifest in sorted(tasks.glob("rav-*/harbor.toml")):
        if manifest.is_symlink() or not manifest.is_file():
            continue
        config = tomllib.loads(manifest.read_text(encoding="utf-8"))
        if config.get("instance") == instance:
            matches.append((config, manifest.parent))
    if len(matches) != 1:
        raise ValueError(
            f"expected one Harbor task manifest for Magic card instance {instance!r}; found {len(matches)}"
        )
    config, task_dir = matches[0]
    expected_fields = {
        "schema_version": "cardbench.magic.card-task.v1",
        "task_id": "cardbench/magic/card",
        "instance": instance,
    }
    if any(config.get(key) != value for key, value in expected_fields.items()):
        raise ValueError(f"invalid Magic card task manifest: {task_dir / 'harbor.toml'}")
    for key in ("authority_bin", "target_file", "materializer", "scorer", "bundle"):
        value = config.get(key)
        if not isinstance(value, str) or not value:
            raise ValueError(f"Magic card task manifest is missing {key!r}: {task_dir}")
        if key == "authority_bin":
            continue
        relative = Path(value)
        if relative.is_absolute() or ".." in relative.parts:
            raise ValueError(f"unsafe {key} path in Magic card task manifest: {value}")
        if key == "bundle":
            path = REPO / "adapters" / "harbor" / "bundles" / relative
        else:
            path = REPO / relative
        if path.is_symlink() or not path.exists():
            raise ValueError(f"Magic card task manifest path is missing or unsafe: {value}")
        if key == "bundle" and not path.is_dir():
            raise ValueError(f"Magic card task bundle must be a directory: {value}")
        if key != "bundle" and not path.is_file():
            raise ValueError(f"Magic card task source path must be a file: {value}")
        if not path.resolve().is_relative_to(REPO.resolve()):
            raise ValueError(f"Magic card task manifest path leaves the repository: {value}")
    for key in ("definition_id", "expansion", "variant"):
        if not isinstance(config.get(key), str) or not config[key]:
            raise ValueError(f"Magic card task manifest is missing {key!r}: {task_dir}")
    return config, task_dir


def score_command(
    family: str,
    candidate: Path | None,
    output: Path,
    *,
    instance: str,
    expansion: str,
    variant: str,
    suite: str,
    variety: str,
    split: str = "heldout",
) -> list[str]:
    if variety == "magic":
        if family == "card":
            config, _task_dir = magic_card_task_config(instance)
            verifier_out = output / "logs" / "verifier"
            if candidate is None:
                return [
                    "cargo",
                    "run",
                    "--manifest-path",
                    str(MAGIC / "Cargo.toml"),
                    "-p",
                    "cardbench-magic-session",
                    "--bin",
                    config["authority_bin"],
                    "--",
                    "--output-root",
                    str(verifier_out),
                ]
            workspace = output / "workspace"
            expected_candidate = workspace / config["target_file"]
            if candidate.resolve() != expected_candidate.resolve():
                raise ValueError(f"Magic single-card accepts only its configured target: {config['target_file']}")
            return [
                sys.executable,
                str(REPO / config["scorer"]),
                "--workspace",
                str(workspace),
                "--manifest",
                str(output / "task-materialization.json"),
                "--output-root",
                str(verifier_out),
            ]
        if family == "set_pvp":
            verifier_out = output / "logs" / "verifier"
            if candidate is None:
                return [
                    "cargo",
                    "run",
                    "--manifest-path",
                    str(MAGIC / "Cargo.toml"),
                    "-p",
                    "cardbench-magic-session",
                    "--bin",
                    "rav-char-pvp",
                    "--",
                    "--output-root",
                    str(verifier_out),
                ]
            workspace = output / "workspace"
            expected_candidate = workspace / "varieties" / "magic" / "sets" / "ravnica_city_of_guilds" / "src" / "cards" / "char.rs"
            if candidate.resolve() != expected_candidate.resolve():
                raise ValueError("Magic set-PvP accepts only the materialized Char source")
            return [
                sys.executable,
                str(MAGIC / "scripts" / "score_char_pvp.py"),
                "--workspace",
                str(workspace),
                "--manifest",
                str(output / "task-materialization.json"),
                "--output-root",
                str(verifier_out),
            ]
        if family != "engine":
            raise ValueError(f"cardbench/magic/{family} is scaffold-only")
        return [
            "cargo",
            "run",
            "--manifest-path",
            str(MAGIC / "Cargo.toml"),
            "-p",
            "cardbench-magic-rav",
            "--bin",
            "rav-engine-parity",
            "--",
            "--output-root",
            str(output / "logs" / "verifier"),
        ]
    if family == "code_policy":
        # Train output is diagnostic and lands beside the authority run; the
        # heldout sweep keeps the canonical verifier path the receipt reads.
        destination = output / "logs" / "verifier"
        if split == "train":
            destination = destination / "train"
        return [
            sys.executable,
            str(POKEMON / "scripts" / "run_policy_sweep.py"),
            "--candidate",
            str(candidate),
            "--candidate-id",
            "reference" if candidate == REFERENCE_POLICY else "agent",
            "--split",
            split,
            "--output-root",
            str(destination),
        ]
    if family == "deck_opt":
        return [
            sys.executable,
            str(POKEMON / "scripts" / "run_deck_eval.py"),
            "--candidate",
            str(candidate),
            "--candidate-id",
            "reference" if candidate == REFERENCE_DECK else "agent",
            "--output-root",
            str(output / "logs" / "verifier"),
        ]
    if family == "card":
        command = [
            sys.executable,
            str(POKEMON / "scripts" / "run_card_eval.py"),
            "--instance",
            instance,
            "--output-root",
            str(output / "logs" / "verifier"),
        ]
        if candidate is not None:
            command.extend(["--candidate", str(candidate)])
        return command
    if family == "set_engine":
        return [
            sys.executable,
            str(POKEMON / "scripts" / "run_set_engine_eval.py"),
            "--expansion",
            expansion,
            "--variant",
            variant,
            "--subject",
            "gold",
            "--suite",
            suite,
            "--output-root",
            str(output / "logs" / "verifier"),
        ]
    if family == "engine":
        return [
            sys.executable,
            str(POKEMON / "scripts" / "run_engine_parity.py"),
            "--output-root",
            str(output / "logs" / "verifier"),
        ]
    raise ValueError(f"{family} is scaffold-only")


def write_receipt(
    output: Path, family: str, command: str, agent_rc: int, verify_rc: int, variety: str
) -> None:
    result_path = output / "logs" / "verifier" / "result.json"
    if family == "engine":
        result_path = output / "logs" / "verifier" / "engine-check.json"
    verifier = json.loads(result_path.read_text()) if result_path.exists() else {}
    payload = {
        "schema_version": "cardbench.harbor.lane_receipt.v1",
        "task_id": f"cardbench/{variety}/{family}",
        "family": family,
        "variety": variety,
        "agent": "reference" if command == "verify" else "codex",
        "model": os.environ.get("CARDBENCH_HARBOR_MODEL", "openai/gpt-5.4-mini"),
        "effort": os.environ.get("CARDBENCH_HARBOR_EFFORT", "low"),
        "agent_rc": agent_rc,
        "verify_rc": verify_rc,
        "out_dir": str(output),
        "verifier": verifier,
        "reward": float(verifier.get("harbor_reward", 0.0)),
    }
    # Lift the Codex runner's Trace V5 refs onto the receipt. The evals matrix
    # reads trace evidence from lane-receipt.json and nowhere else, so a trace
    # that was captured and sealed still counts as absent without this.
    rollout_result_path = output / "rollout_result.json"
    if rollout_result_path.exists():
        rollout_result = json.loads(rollout_result_path.read_text())
        trace_v5 = (rollout_result.get("metadata") or {}).get("trace_v5")
        if trace_v5:
            payload["trace_v5"] = trace_v5

    scorecard_path = output / "logs" / "verifier" / "heldout_scorecard.json"
    if scorecard_path.exists():
        scorecard = json.loads(scorecard_path.read_text())
        payload["heldout"] = {
            "heldout_n": scorecard.get("heldout_n"),
            "roster_id": scorecard.get("roster_id"),
            "roster_sha256": scorecard.get("roster_sha256"),
            "baseline_score": scorecard.get("baseline_score"),
            "candidate_score": scorecard.get("candidate_score"),
            "delta": scorecard.get("delta"),
            "lift_accepted": bool(
                (scorecard.get("lift_verdict") or {}).get("accepted")
            ),
            "lift_verdict": scorecard.get("lift_verdict"),
            # The heldout opponents are lifted from a public upstream repo; the
            # seal covers the composition of the split, not the sources.
            "published_upstream_opponents": True,
        }

    for key in (
        "baseline_score",
        "best_score",
        "delta_vs_baseline",
        "best_candidate_id",
        "score_metric",
        "split",
        "roster_id",
        "roster_sha256",
        "cell_count",
        "lift_verdict",
        "instance_id",
        "expansion_id",
        "expansion",
        "variant",
        "suite",
        "tests_passed",
        "tests_total",
        "games_matched",
        "games_total",
        "authority",
        "suite_id",
        "suite_sha256",
        "compile_ok",
        "compile_passed",
        "cards_passed",
        "cards_credited",
        "cards_total",
        "printings_total",
        "coverage_complete",
        "full_reward",
        "pilot_reward",
        "pilot_card_passed",
        "qualification_status",
        "mutation_sensitivity",
        "qualification_gates",
        "sealed_authority",
        "starting_workspace_sha256",
        "worker_image",
        "rules_authority",
        "viewer_scope",
        "traces",
        "per_card_breakdown",
    ):
        if key in verifier:
            payload[key] = verifier[key]
    (output / "lane-receipt.json").write_text(json.dumps(payload, indent=2) + "\n")


def run_reference(
    family: str,
    output: Path,
    *,
    instance: str,
    expansion: str,
    variant: str,
    suite: str,
    variety: str,
) -> int:
    if variety == "magic" and family == "card":
        return run_magic_card_reference(output, instance)
    if variety == "magic" and family == "set_pvp":
        return run_magic_set_pvp_reference(output)
    candidate: Path | None = None
    if family == "code_policy":
        candidate = REFERENCE_POLICY
    elif family == "deck_opt":
        candidate = REFERENCE_DECK
    elif family == "card":
        data = json.loads((CARDS / "instances" / f"{instance}.json").read_text())
        module = Path(data["card_file"]).stem
        candidate = SEALED / "pokemon" / "card" / "implementations" / f"{module}.rs"
    if family == "code_policy":
        # Diagnostic first so a failed authority run still leaves the train
        # comparison behind to debug with.
        subprocess.run(
            score_command(
                family,
                candidate,
                output,
                instance=instance,
                expansion=expansion,
                variant=variant,
                suite=suite,
                variety=variety,
                split="train",
            )
        )
    command = score_command(
        family,
        candidate,
        output,
        instance=instance,
        expansion=expansion,
        variant=variant,
        suite=suite,
        variety=variety,
    )
    completed = subprocess.run(command)
    write_receipt(output, family, "verify", 0, completed.returncode, variety)
    print(f"receipt: {output / 'lane-receipt.json'}")
    return completed.returncode


def materialize_magic_workspace(workspace: Path, manifest: Path) -> None:
    subprocess.run(
        [
            sys.executable,
            str(MAGIC / "scripts" / "materialize_char_task.py"),
            "--workspace",
            str(workspace),
            "--manifest",
            str(manifest),
        ],
        check=True,
    )


def independent_review_is_current(task: str = "rav-char-pvp") -> bool:
    evidence_path = MAGIC / "tasks" / task / "independent_review.json"
    required_files = {
        "rav-char-pvp": {
            "adapters/harbor/bundles/set_pvp/README.md",
            "adapters/harbor/bundles/set_pvp/instruction.md",
            "adapters/harbor/bundles/set_pvp/tests/test.sh",
            "adapters/harbor/scripts/run_harbor.py",
            "varieties/magic/DATA_PROVENANCE.md",
            "varieties/magic/README.md",
            "varieties/magic/Cargo.lock",
            "varieties/magic/Cargo.toml",
            "varieties/magic/scripts/materialize_char_task.py",
            "varieties/magic/scripts/rav_card_task_workspace.py",
            "varieties/magic/scripts/score_char_pvp.py",
            "varieties/magic/session/Cargo.toml",
            "varieties/magic/session/src/project.rs",
            "varieties/magic/session/src/bin/rav_char_pvp.rs",
            "varieties/magic/engine/src/game.rs",
            "varieties/magic/engine/src/model.rs",
            "varieties/magic/sets/ravnica_city_of_guilds/src/cards/char.rs",
            "varieties/magic/sets/ravnica_city_of_guilds/src/cards/mod.rs",
            "varieties/magic/sets/ravnica_city_of_guilds/src/lib.rs",
            "varieties/magic/sets/ravnica_city_of_guilds/catalog/rav_main_set.tsv",
            "varieties/magic/tasks/rav-char-pvp/README.md",
            "varieties/magic/tasks/rav-char-pvp/features.toml",
            "varieties/magic/tasks/rav-set-pvp/RAV_PVP_SOURCE_CONTRACT.md",
        },
        "rav-lightning-helix-pvp": {
            "adapters/harbor/bundles/magic_card/README.md",
            "adapters/harbor/bundles/magic_card/instruction.md",
            "adapters/harbor/bundles/magic_card/tests/test.sh",
            "adapters/harbor/scripts/run_harbor.py",
            "varieties/magic/DATA_PROVENANCE.md",
            "varieties/magic/README.md",
            "varieties/magic/Cargo.lock",
            "varieties/magic/Cargo.toml",
            "varieties/magic/scripts/materialize_lightning_helix_task.py",
            "varieties/magic/scripts/score_lightning_helix_pvp.py",
            "varieties/magic/session/Cargo.toml",
            "varieties/magic/session/src/project.rs",
            "varieties/magic/session/src/bin/rav_lightning_helix_pvp.rs",
            "varieties/magic/engine/src/game.rs",
            "varieties/magic/engine/src/model.rs",
            "varieties/magic/sets/ravnica_city_of_guilds/src/cards/lightning_helix.rs",
            "varieties/magic/sets/ravnica_city_of_guilds/src/cards/mod.rs",
            "varieties/magic/sets/ravnica_city_of_guilds/src/lib.rs",
            "varieties/magic/sets/ravnica_city_of_guilds/catalog/rav_main_set.tsv",
            "varieties/magic/tasks/rav-lightning-helix-pvp/README.md",
            "varieties/magic/tasks/rav-lightning-helix-pvp/features.toml",
            "varieties/magic/tasks/rav-lightning-helix-pvp/harbor.toml",
        },
    }.get(task)
    expected_scope = "lightning helix" if task == "rav-lightning-helix-pvp" else "char"
    card_manifest = MAGIC / "tasks" / task / "harbor.toml"
    if task.startswith("rav-") and card_manifest.is_file():
        manifest_path = card_manifest
        try:
            if manifest_path.is_symlink() or not manifest_path.is_file():
                return False
            task_config = tomllib.loads(manifest_path.read_text(encoding="utf-8"))
            configured_files = task_config.get("review_files")
            configured_scope = task_config.get("review_scope")
            if (
                not isinstance(configured_files, list)
                or not configured_files
                or not all(isinstance(item, str) and item for item in configured_files)
                or len(set(configured_files)) != len(configured_files)
                or not isinstance(configured_scope, str)
                or not configured_scope.strip()
            ):
                return False
            required_files = set(configured_files)
            expected_scope = configured_scope
        except (OSError, tomllib.TOMLDecodeError, TypeError, ValueError):
            return False
    if required_files is None:
        return False
    try:
        if evidence_path.is_symlink() or not evidence_path.is_file():
            return False
        evidence = json.loads(evidence_path.read_text())
        if not isinstance(evidence, dict):
            return False
        if evidence.get("schema_version") != "cardbench.magic.independent-review.v1":
            return False
        if evidence.get("status") != "passed":
            return False
        reviewed_files = evidence.get("reviewed_files")
        review_scope = evidence.get("review_scope")
        if (
            not isinstance(reviewed_files, dict)
            or not required_files.issubset(reviewed_files)
            or not isinstance(review_scope, str)
            or expected_scope.lower() not in review_scope.lower()
        ):
            return False
        repo_root = REPO.resolve()
        for relative, expected_sha256 in reviewed_files.items():
            if not isinstance(relative, str) or not isinstance(expected_sha256, str):
                return False
            relative_path = Path(relative)
            if relative_path.is_absolute() or ".." in relative_path.parts:
                return False
            path = REPO / relative_path
            if path.is_symlink() or not path.is_file():
                return False
            path = path.resolve(strict=True)
            if not path.is_relative_to(repo_root):
                return False
            actual_sha256 = hashlib.sha256(path.read_bytes()).hexdigest()
            if actual_sha256 != expected_sha256:
                return False
        return True
    except (OSError, json.JSONDecodeError, TypeError, ValueError):
        return False


def run_magic_card_reference(output: Path, instance: str) -> int:
    """Qualify a configured card reference, replay, mutation, and stub."""
    config, task_dir = magic_card_task_config(instance)
    target_rel = Path(config["target_file"])
    source_file = REPO / target_rel
    materializer = REPO / config["materializer"]
    scorer = REPO / config["scorer"]
    mutation = config.get("mutation")
    if (
        not isinstance(mutation, dict)
        or not all(
            isinstance(mutation.get(key), str) and mutation[key]
            for key in ("find", "replace", "expected_diagnostic")
        )
    ):
        raise ValueError(f"card task has no complete source mutation contract: {task_dir}")
    if source_file.is_symlink() or not source_file.is_file():
        raise ValueError(f"card task target source is missing or unsafe: {target_rel}")

    def materialize(label: str) -> tuple[Path, Path]:
        workspace = output / f"{label}-workspace"
        manifest = output / f"{label}-materialization.json"
        subprocess.run(
            [
                sys.executable,
                str(materializer),
                "--workspace",
                str(workspace),
                "--manifest",
                str(manifest),
            ],
            check=True,
        )
        return workspace, manifest

    def score(
        label: str, workspace: Path, manifest: Path, candidate_kind: str
    ) -> tuple[subprocess.CompletedProcess, dict]:
        output_root = output / "logs" / label
        completed = subprocess.run(
            [
                sys.executable,
                str(scorer),
                "--workspace",
                str(workspace),
                "--manifest",
                str(manifest),
                "--output-root",
                str(output_root),
                "--candidate-kind",
                candidate_kind,
            ]
        )
        report_path = output_root / "result.json"
        report = json.loads(report_path.read_text()) if report_path.is_file() else {}
        return completed, report

    def reference_run(label: str) -> tuple[subprocess.CompletedProcess, dict, dict]:
        workspace, manifest = materialize(label)
        candidate_source = workspace / target_rel
        shutil.copyfile(source_file, candidate_source)
        completed, report = score(label, workspace, manifest, "reference")
        manifest_data = json.loads(manifest.read_text())
        return completed, report, manifest_data

    first_run, result, first_manifest = reference_run("verifier")
    replay_run, replay_result, replay_manifest = reference_run("reference-replay")
    trace_signature = lambda report: [
        (trace.get("id"), trace.get("digest"), trace.get("ordered_event_sequence"))
        for trace in report.get("traces", [])
    ]

    def report_has_clean_traces(report: dict) -> bool:
        traces = report.get("traces")
        return (
            isinstance(traces, list)
            and bool(traces)
            and all(
                isinstance(trace, dict)
                and trace.get("passed") is True
                and trace.get("terminal") in {"Winner(PlayerId(0))", "Draw"}
                and isinstance(trace.get("digest"), str)
                and bool(trace["digest"])
                for trace in traces
            )
        )

    card_definition = config.get("definition_id")
    first_candidate = result.get("candidate")
    replay_candidate = replay_result.get("candidate")
    expected_report = (
        result.get("task_id") == "cardbench/magic/card"
        and result.get("passed") is True
        and result.get("pilot_card_passed") is True
        and result.get("cards_passed") == 1
        and result.get("cards_credited") == 0
        and result.get("cards_total") == 291
        and result.get("printings_total") == 306
        and result.get("coverage_complete") is False
        and result.get("full_reward") is False
        and result.get("harbor_reward") == 0.0
        and isinstance(first_candidate, dict)
        and first_candidate.get("card_definition") == card_definition
        and first_candidate.get("kind") == "reference"
        and first_candidate.get("source_sha256")
        == hashlib.sha256(source_file.read_bytes()).hexdigest()
        and report_has_clean_traces(result)
    )
    two_clean_passes = (
        first_run.returncode == 0
        and replay_run.returncode == 0
        and expected_report
        and replay_result.get("passed") is True
        and replay_result.get("pilot_card_passed") is True
        and replay_result.get("cards_passed") == 1
        and replay_result.get("cards_credited") == 0
        and replay_result.get("cards_total") == 291
        and replay_result.get("printings_total") == 306
        and replay_result.get("task_id") == "cardbench/magic/card"
        and isinstance(replay_candidate, dict)
        and replay_candidate.get("kind") == "reference"
        and replay_candidate.get("card_definition") == card_definition
        and replay_candidate.get("source_sha256")
        == hashlib.sha256(source_file.read_bytes()).hexdigest()
        and report_has_clean_traces(replay_result)
        and first_manifest.get("workspace_sha256")
        == replay_manifest.get("workspace_sha256")
        and result.get("starting_workspace_sha256")
        == replay_result.get("starting_workspace_sha256")
        and result.get("worker_image") == replay_result.get("worker_image")
        and first_candidate.get("source_sha256")
        == replay_candidate.get("source_sha256")
        and trace_signature(result) == trace_signature(replay_result)
    )

    mutation_workspace, mutation_manifest = materialize("mutation")
    mutation_source = mutation_workspace / target_rel
    shutil.copyfile(source_file, mutation_source)
    source_text = mutation_source.read_text(encoding="utf-8")
    mutation_found = source_text.count(mutation["find"]) == 1
    mutation_detail = "mutation anchor must occur exactly once in the target source"
    mutation_ok = False
    if mutation_found:
        mutation_source.write_text(
            source_text.replace(mutation["find"], mutation["replace"], 1),
            encoding="utf-8",
        )
        mutation_run, mutation_result = score(
            "mutation", mutation_workspace, mutation_manifest, "agent"
        )
        mutation_diagnostics = mutation_result.get("diagnostics", "")
        mutation_stderr_path = output / "logs" / "mutation" / "worker.stderr"
        mutation_stderr = (
            mutation_stderr_path.read_text(errors="replace")
            if mutation_stderr_path.is_file()
            else ""
        )
        mutation_ok = (
            mutation_run.returncode != 0
            and mutation_result.get("passed") is False
            and "Finished `dev` profile" in mutation_stderr
            and mutation["expected_diagnostic"] in mutation_diagnostics
        )
        mutation_detail = mutation_diagnostics[-3000:]

    stub_workspace, stub_manifest = materialize("stub")
    stub_run, stub_result = score("stub", stub_workspace, stub_manifest, "agent")
    stub_diagnostics = stub_result.get("diagnostics", "")
    stub_stderr_path = output / "logs" / "stub" / "worker.stderr"
    stub_stderr = stub_stderr_path.read_text(errors="replace") if stub_stderr_path.is_file() else ""
    stub_expected = config.get("stub_expected_diagnostic", mutation["expected_diagnostic"])
    stub_rejected = (
        stub_run.returncode != 0
        and stub_result.get("passed") is False
        and "Finished `dev` profile" in stub_stderr
        and stub_expected in stub_diagnostics
    )

    independent_review_passed = independent_review_is_current(task_dir.name)
    result.setdefault("mutation_sensitivity", {}).update(
        {
            "compiled_source_mutation": f"{mutation['find']} -> {mutation['replace']}",
            "source_mutation_detected": mutation_ok,
            "compile_valid_stub_rejected": stub_rejected,
            "mutation_authority_outside_candidate_workspace": True,
            "mutation_diagnostics": mutation_detail,
        }
    )
    result["qualification_gates"] = {
        "two_clean_reference_passes": two_clean_passes,
        "compiled_source_mutation_caught": mutation_ok,
        "compile_valid_stub_rejected_semantically": stub_rejected,
        "independent_review": independent_review_passed,
        "real_agent_trial": False,
    }
    if independent_review_passed:
        result["qualification_status"] = "reference_pilot_pass; real agent trial remains"
        for card in result.get("per_card_breakdown", []):
            if card.get("definition_id") == card_definition:
                card["qualification"] = (
                    "pilot_trace_passed; independent_review_passed; candidate_trial_pending"
                )
        result["independent_review_evidence"] = str(
            (task_dir / "independent_review.json").relative_to(REPO)
        )
    else:
        result["qualification_status"] = (
            "reference_pilot_pass; independent review and real agent trial remain"
        )
    result["candidate"] = {
        "kind": "reference",
        "card_definition": card_definition,
        "source_sha256": hashlib.sha256(source_file.read_bytes()).hexdigest(),
    }
    result["starting_workspace_sha256"] = first_manifest.get("workspace_sha256")
    if not two_clean_passes or not mutation_ok or not stub_rejected:
        result["passed"] = False
        result["pilot_card_passed"] = False
        result["cards_passed"] = 0
        result["harbor_reward"] = 0.0
        result["full_reward"] = False
        result["error"] = "reference, mutation, or compile-valid stub qualification gate failed"
    result_path = output / "logs" / "verifier" / "result.json"
    result_path.parent.mkdir(parents=True, exist_ok=True)
    result_path.write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
    (result_path.parent / "reward.txt").write_text(
        f"{result.get('harbor_reward', 0.0):.8f}\n", encoding="utf-8"
    )
    write_receipt(
        output,
        "card",
        "verify",
        0,
        0 if result.get("passed") is True else 1,
        "magic",
    )
    receipt_path = output / "lane-receipt.json"
    receipt = json.loads(receipt_path.read_text())
    receipt.update(
        {
            "instance_id": instance,
            "expansion": config["expansion"],
            "variant": config["variant"],
            "task_manifest": str((task_dir / "harbor.toml").relative_to(REPO)),
        }
    )
    receipt_path.write_text(json.dumps(receipt, indent=2) + "\n")
    print(f"receipt: {receipt_path}")
    return 0 if result.get("passed") is True else 1


def run_magic_set_pvp_reference(output: Path) -> int:
    """Score the reference and a source mutant in isolated worker containers."""
    workspace = output / "workspace"
    materialization = output / "task-materialization.json"
    materialize_magic_workspace(workspace, materialization)

    source_rel = Path(
        "varieties/magic/sets/ravnica_city_of_guilds/src/cards/char.rs"
    )
    source_file = MAGIC / source_rel.relative_to("varieties/magic")
    reference_workspace = output / "reference-workspace"
    reference_manifest = output / "reference-materialization.json"
    materialize_magic_workspace(reference_workspace, reference_manifest)
    reference_source = reference_workspace / source_rel
    shutil.copyfile(source_file, reference_source)

    verifier_out = output / "logs" / "verifier"
    candidate_command = [
        sys.executable,
        str(MAGIC / "scripts" / "score_char_pvp.py"),
        "--workspace",
        str(reference_workspace),
        "--manifest",
        str(reference_manifest),
        "--output-root",
        str(verifier_out),
        "--candidate-kind",
        "reference",
    ]
    candidate_run = subprocess.run(candidate_command)
    result_path = verifier_out / "result.json"
    result = json.loads(result_path.read_text()) if result_path.exists() else {
        "schema_version": "cardbench.magic.set-pvp.check.v1",
        "task_id": "cardbench/magic/set_pvp",
        "passed": False,
        "cards_passed": 0,
        "cards_total": 291,
        "coverage_complete": False,
        "full_reward": False,
        "harbor_reward": 0.0,
        "error": "reference scorer did not emit result.json",
    }

    mutation_workspace = output / "mutation-workspace"
    mutation_manifest = output / "mutation-materialization.json"
    materialize_magic_workspace(mutation_workspace, mutation_manifest)
    mutation_source = mutation_workspace / source_rel
    shutil.copyfile(source_file, mutation_source)
    source_text = mutation_source.read_text()
    if source_text.count("amount: 4,") != 1:
        mutation_ok = False
        mutation_detail = "expected one Char 4-damage instruction before mutation"
    else:
        mutation_source.write_text(source_text.replace("amount: 4,", "amount: 3,", 1))
        mutation_out = output / "logs" / "mutation"
        mutation_run = subprocess.run(
            [
                sys.executable,
                str(MAGIC / "scripts" / "score_char_pvp.py"),
                "--workspace",
                str(mutation_workspace),
                "--manifest",
                str(mutation_manifest),
                "--output-root",
                str(mutation_out),
                "--candidate-kind",
                "agent",
            ]
        )
        mutation_result_path = mutation_out / "result.json"
        mutation_result = json.loads(mutation_result_path.read_text()) if mutation_result_path.exists() else {}
        diagnostics = mutation_result.get("diagnostics", "")
        mutation_ok = (
            mutation_run.returncode != 0
            and mutation_result.get("passed") is False
            and "first target did not receive the required four damage" in diagnostics
        )
        mutation_detail = diagnostics[-3000:]

    stub_out = output / "logs" / "stub"
    stub_run = subprocess.run(
        [
            sys.executable,
            str(MAGIC / "scripts" / "score_char_pvp.py"),
            "--workspace",
            str(workspace),
            "--manifest",
            str(materialization),
            "--output-root",
            str(stub_out),
            "--candidate-kind",
            "agent",
        ]
    )
    stub_result_path = stub_out / "result.json"
    stub_result = json.loads(stub_result_path.read_text()) if stub_result_path.exists() else {}
    stub_diagnostics = stub_result.get("diagnostics", "")
    stub_stderr = (stub_out / "worker.stderr").read_text(errors="replace") if (stub_out / "worker.stderr").exists() else ""
    stub_rejected = (
        stub_run.returncode != 0
        and stub_result.get("passed") is False
        and "Finished `dev` profile" in stub_stderr
        and "first target did not receive the required four damage" in stub_diagnostics
    )

    result.setdefault("mutation_sensitivity", {}).update(
        {
            "compiled_source_mutation": "Char first-target damage 4 -> 3",
            "source_mutation_detected": mutation_ok,
            "compile_valid_stub_rejected": stub_rejected,
            "mutation_authority_outside_candidate_workspace": True,
            "mutation_diagnostics": mutation_detail,
        }
    )
    independent_review_passed = independent_review_is_current()
    result["qualification_gates"] = {
        "deterministic_reference_replays": (
            candidate_run.returncode == 0 and result.get("passed") is True
        ),
        "compiled_source_mutation_caught": mutation_ok,
        "compile_valid_stub_rejected_semantically": stub_rejected,
        "independent_review": independent_review_passed,
        "real_agent_trial": False,
    }
    if independent_review_passed:
        result["qualification_status"] = "reference_pilot_pass; real agent trial remains"
        for card in result.get("per_card_breakdown", []):
            if card.get("definition_id") == "RAV-CHAR":
                card["qualification"] = (
                    "pilot_trace_passed; independent_review_passed; candidate_trial_pending"
                )
        result["independent_review_evidence"] = (
            "varieties/magic/tasks/rav-char-pvp/independent_review.json"
        )
    result["candidate"] = {
        "kind": "reference",
        "card_definition": "RAV-CHAR",
        "source_sha256": hashlib.sha256(source_file.read_bytes()).hexdigest(),
    }
    result["starting_workspace_sha256"] = json.loads(
        materialization.read_text()
    )["workspace_sha256"]
    if candidate_run.returncode != 0 or not mutation_ok or not stub_rejected:
        result["passed"] = False
        result["pilot_card_passed"] = False
        result["cards_passed"] = 0
        result["harbor_reward"] = 0.0
        result["full_reward"] = False
        result["error"] = "reference trace or compiled-source mutation gate failed"
        (verifier_out / "reward.txt").write_text("0.0\n")
    result_path.write_text(json.dumps(result, indent=2) + "\n")
    write_receipt(
        output,
        "set_pvp",
        "verify",
        0,
        0 if result.get("passed") else 1,
        "magic",
    )
    print(f"receipt: {output / 'lane-receipt.json'}")
    return 0 if result.get("passed") else 1


EXCLUDED_WORKSPACE_DIRS = {
    ".git",
    ".private",
    "artifacts",
    "target",
    ".cache",
    "__pycache__",
    # Heldout authority: decks, opponent policies and the cell manifest the
    # candidate is scored on. Never stage it into an agent workspace.
    ".sealed",
}


def copy_workspace(destination: Path) -> None:
    def ignore(_path: str, names: list[str]) -> set[str]:
        return {name for name in names if name in EXCLUDED_WORKSPACE_DIRS}

    shutil.copytree(REPO, destination, ignore=ignore)
    assert_no_sealed_leak(destination)


def cargo_cache_root() -> Path:
    """Shared host cache root for candidate builds.

    Honours an outer override so a matrix board can point every lane at one
    location; otherwise it is the checkout's own ``.cache``. Either way it sits
    outside the agent workspace.
    """
    override = os.environ.get("CARDBENCH_CARGO_CACHE", "").strip()
    return Path(override).expanduser().resolve() if override else REPO / ".cache"


def prune_workspace_cache(workspace: Path) -> None:
    """Drop the cargo target tree the agent built inside its own workspace.

    ``benchmark_ai`` roots its build cache at the repo it is invoked from, so an
    agent that compiles to test its policy leaves ~1GB under ``workspace/.cache``
    — per job. A ten-lane board would retain ~10GB of rebuildable object files
    in the results directory.
    """
    cache = workspace / ".cache"
    if cache.is_dir():
        shutil.rmtree(cache, ignore_errors=True)


def assert_no_sealed_leak(workspace: Path) -> None:
    """Fail closed if any sealed heldout asset reached the agent workspace."""
    leaked = sorted(
        str(path.relative_to(workspace))
        for path in workspace.rglob("*")
        if ".sealed" in path.parts
    )
    if leaked:
        raise RuntimeError(
            "sealed heldout assets leaked into the agent workspace: "
            + ", ".join(leaked[:10])
        )


def run_codex(
    family: str,
    output: Path,
    *,
    instance: str,
    expansion: str,
    variant: str,
    suite: str,
    variety: str,
) -> int:
    if variety == "magic" and family == "card":
        return run_magic_card_codex(output, instance)
    if variety == "magic" and family == "set_pvp":
        return run_magic_set_pvp_codex(output)
    if variety != "pokemon":
        raise ValueError(f"cardbench/magic/{family} has no Codex bundle")
    if family not in {"code_policy", "deck_opt"}:
        raise ValueError(f"Codex is not promoted for {family}")
    if not CODEX_RUNNER.is_file():
        raise FileNotFoundError(f"missing shared evals Harbor runner: {CODEX_RUNNER}")
    auth = Path(os.environ.get("CODEX_HOME", Path.home() / ".codex")) / "auth.json"
    if not auth.is_file():
        raise FileNotFoundError("Codex auth unavailable; run `codex login`")
    workspace = output / "workspace"
    output.mkdir(parents=True, exist_ok=True)
    copy_workspace(workspace)
    candidate_rel = Path("candidate") / ("policy.rs" if family == "code_policy" else "deck.json")

    # Per-run copy of the bundle. The shared Harbor runner derives the agent's
    # CODEX_HOME from the task root, so concurrent lanes pointed at the
    # in-repo bundle share one Codex state directory: the second agent then
    # reuses the first's capture-proxy URL, and its trace records zero calls
    # while the first records both lanes' traffic. A private task root also
    # keeps the runner's .codex/.cargo scratch out of the checkout.
    task_root = output / "task_root"
    shutil.copytree(REPO / "adapters" / "harbor" / "bundles" / family, task_root)
    payload = {
        "trace_correlation_id": f"cardbench-{family}-{output.name}",
        "deployment_name": f"cardbench_{family}",
        "codex_timeout_seconds": int(os.environ.get("CARDBENCH_HARBOR_TIMEOUT_SEC", "3600")),
        "harbor_agent": {
            "name": "codex",
            "model_name": os.environ.get("CARDBENCH_HARBOR_MODEL", "openai/gpt-5.4-mini"),
            "kwargs": {
                "reasoning_effort": os.environ.get("CARDBENCH_HARBOR_EFFORT", "low"),
                "codex_timeout_seconds": int(os.environ.get("CARDBENCH_HARBOR_TIMEOUT_SEC", "3600")),
            },
        },
        "task_metadata": {
            "workspace_dir": str(workspace),
            "task_id": f"cardbench/pokemon/{family}",
            "benchmark": "cardbench",
            "submission_path": str(candidate_rel),
        },
        "submission_path": str(candidate_rel),
        "codex_auth_json_b64": base64.b64encode(auth.read_bytes()).decode("ascii"),
        "codex_auth_source": "host_codex_auth_json",
        "env": {
            "CARDBENCH_WORKSPACE_ROOT": str(workspace),
            # The bundle verifier scores the sealed heldout split, which is
            # absent from the workspace by design, so it needs the host
            # checkout. It also writes straight to the authority directory the
            # receipt reads, which is why no second scoring pass is needed.
            "CARDBENCH_REPO_ROOT": str(REPO),
            "CARDBENCH_VERIFIER_OUT": str(output / "logs" / "verifier"),
            # Keep the agent's cargo target tree out of its workspace, which is
            # retained in the run's results directory. Without this each job
            # leaves ~1GB of rebuildable object files behind.
            "CARDBENCH_CARGO_CACHE": str(cargo_cache_root()),
        },
    }
    rollout = output / "rollout.json"
    rollout.write_text(json.dumps(payload, indent=2) + "\n")
    agent_result = output / "rollout_result.json"
    agent = subprocess.run(
        [
            sys.executable,
            str(CODEX_RUNNER),
            "--input",
            str(rollout),
            "--output",
            str(agent_result),
            "--task-root",
            str(task_root),
        ]
    )
    candidate = workspace / candidate_rel
    verifier_result = output / "logs" / "verifier" / "result.json"
    if candidate.exists():
        if family == "code_policy":
            subprocess.run(
                score_command(
                    family,
                    candidate,
                    output,
                    instance=instance,
                    expansion=expansion,
                    variant=variant,
                    suite=suite,
                    variety=variety,
                    split="train",
                )
            )
        if verifier_result.exists():
            # The bundle's tests/test.sh already scored the authority split into
            # this directory. Re-running it would cost another compile plus a
            # full heldout sweep for an identical answer.
            verify_rc = 0 if json.loads(verifier_result.read_text()).get("passed") else 1
        else:
            scored = subprocess.run(
                score_command(
                    family,
                    candidate,
                    output,
                    instance=instance,
                    expansion=expansion,
                    variant=variant,
                    suite=suite,
                    variety=variety,
                )
            )
            verify_rc = scored.returncode
        prune_workspace_cache(workspace)
    else:
        verify_rc = 1
        verifier_dir = output / "logs" / "verifier"
        verifier_dir.mkdir(parents=True, exist_ok=True)
        (verifier_dir / "result.json").write_text(
            json.dumps({"passed": False, "harbor_reward": 0.0, "error": f"missing {candidate_rel}"}, indent=2) + "\n"
        )
        (verifier_dir / "reward.txt").write_text("0.0\n")
    write_receipt(output, family, "codex", agent.returncode, verify_rc, variety)
    print(f"receipt: {output / 'lane-receipt.json'}")
    return 0 if agent.returncode == 0 and verify_rc == 0 else 1


def run_magic_set_pvp_codex(output: Path) -> int:
    """Run the one-card Magic task and score only inside its Rust worker."""
    if not CODEX_RUNNER.is_file():
        raise FileNotFoundError(f"missing shared evals Harbor runner: {CODEX_RUNNER}")
    assert_harbor_agent_cannot_read_host_checkout()
    auth = Path(os.environ.get("CODEX_HOME", Path.home() / ".codex")) / "auth.json"
    if not auth.is_file():
        raise FileNotFoundError("Codex auth unavailable; run `codex login`")

    workspace = output / "workspace"
    materialization = output / "task-materialization.json"
    if workspace.is_symlink():
        raise RuntimeError("candidate workspace must not be a symbolic link")
    if workspace.exists():
        if not workspace.is_dir():
            raise RuntimeError("candidate workspace path exists and is not a directory")
        if any(workspace.iterdir()):
            raise RuntimeError("candidate workspace must be fresh before Harbor launch")
        workspace.rmdir()
    if (workspace.parent / "evaluator").exists():
        raise RuntimeError("candidate workspace parent already has evaluator state")

    task_root = output / "task_root"
    shutil.copytree(REPO / "adapters" / "harbor" / "bundles" / "set_pvp", task_root)
    workspace_seed = task_root / "environment" / "workspace"
    subprocess.run(
        [
            sys.executable,
            str(MAGIC / "scripts" / "materialize_char_task.py"),
            "--workspace",
            str(workspace_seed),
            "--manifest",
            str(materialization),
        ],
        check=True,
    )
    candidate_rel = Path(
        "varieties/magic/sets/ravnica_city_of_guilds/src/cards/char.rs"
    )
    payload = {
        "trace_correlation_id": f"cardbench-set-pvp-{output.name}",
        "deployment_name": "cardbench_magic_set_pvp",
        "codex_timeout_seconds": int(os.environ.get("CARDBENCH_HARBOR_TIMEOUT_SEC", "3600")),
        "harbor_agent": {
            "name": "codex",
            "model_name": os.environ.get("CARDBENCH_HARBOR_MODEL", "openai/gpt-5.4-mini"),
            "kwargs": {
                "reasoning_effort": os.environ.get("CARDBENCH_HARBOR_EFFORT", "low"),
                "codex_timeout_seconds": int(os.environ.get("CARDBENCH_HARBOR_TIMEOUT_SEC", "3600")),
            },
        },
        "task_metadata": {
            "workspace_dir": str(workspace),
            "task_id": "cardbench/magic/set_pvp",
            "benchmark": "cardbench",
            "submission_path": str(candidate_rel),
            "require_fresh_absolute_workspace": True,
            "require_agent_filesystem_isolation": True,
        },
        "submission_path": str(candidate_rel),
        "codex_auth_json_b64": base64.b64encode(auth.read_bytes()).decode("ascii"),
        "codex_auth_source": "host_codex_auth_json",
        "env": {
            "CARDBENCH_REPO_ROOT": str(REPO),
            "CARDBENCH_WORKSPACE_ROOT": str(workspace),
            "CARDBENCH_VERIFIER_OUT": str(output / "logs" / "verifier"),
            "CARDBENCH_MAGIC_TASK_MANIFEST": str(materialization),
            "CARDBENCH_MAGIC_WORKER_IMAGE": os.environ.get(
                "CARDBENCH_MAGIC_WORKER_IMAGE",
                "rust@sha256:d7482085ff5b415f84dba5647ae71606650bdef00db7aeb69f4b3d170c3e4082",
            ),
        },
    }
    rollout = output / "rollout.json"
    rollout.write_text(json.dumps(payload, indent=2) + "\n")
    agent_result = output / "rollout_result.json"
    agent = subprocess.run(
        [
            sys.executable,
            str(CODEX_RUNNER),
            "--input",
            str(rollout),
            "--output",
            str(agent_result),
            "--task-root",
            str(task_root),
        ]
    )
    runner_result = {}
    if agent_result.is_file():
        try:
            runner_result = json.loads(agent_result.read_text())
        except (OSError, json.JSONDecodeError):
            runner_result = {}
    runner_result = runner_result if isinstance(runner_result, dict) else {}
    runner_metadata = runner_result.get("metadata")
    runner_metadata = runner_metadata if isinstance(runner_metadata, dict) else {}
    codex_returncode = runner_metadata.get("codex_returncode")
    task_smoke_returncode = runner_metadata.get("verifier_returncode")
    agent_completed = codex_returncode == 0 and task_smoke_returncode == 0

    remove_runner_staged_tests(workspace)

    candidate = workspace / candidate_rel
    if candidate.is_file():
        scored = subprocess.run(
            score_command(
                "set_pvp",
                candidate,
                output,
                instance="",
                expansion="RAV",
                variant="pilot",
                suite="shown",
                variety="magic",
            )
        )
        verify_rc = scored.returncode
    else:
        verify_rc = 1
        verifier_dir = output / "logs" / "verifier"
        verifier_dir.mkdir(parents=True, exist_ok=True)
        (verifier_dir / "result.json").write_text(
            json.dumps(
                {
                    "schema_version": "cardbench.magic.set-pvp.check.v1",
                    "task_id": "cardbench/magic/set_pvp",
                    "passed": False,
                    "cards_passed": 0,
                    "cards_total": 291,
                    "coverage_complete": False,
                    "full_reward": False,
                    "harbor_reward": 0.0,
                    "error": f"missing submission {candidate_rel}",
                },
                indent=2,
            )
            + "\n"
        )
        (verifier_dir / "reward.txt").write_text("0.0\n")
    write_receipt(
        output,
        "set_pvp",
        "codex",
        0 if agent_completed else agent.returncode,
        verify_rc,
        "magic",
    )
    receipt_path = output / "lane-receipt.json"
    receipt = json.loads(receipt_path.read_text())
    receipt.update(
        {
            "harbor_runner_rc": agent.returncode,
            "runner_success": bool(runner_result.get("success")),
            "codex_returncode": codex_returncode,
            "task_smoke_returncode": task_smoke_returncode,
            "agent_completed": agent_completed,
        }
    )
    receipt_path.write_text(json.dumps(receipt, indent=2) + "\n")
    print(f"receipt: {output / 'lane-receipt.json'}")
    return 0 if agent_completed and verify_rc == 0 else 1


def run_magic_card_codex(output: Path, instance: str) -> int:
    """Run the isolated single-card task described by its task-local manifest."""
    config, task_dir = magic_card_task_config(instance)
    target_rel = Path(config["target_file"])
    materializer = REPO / config["materializer"]
    if not CODEX_RUNNER.is_file():
        raise FileNotFoundError(f"missing shared evals Harbor runner: {CODEX_RUNNER}")
    assert_harbor_agent_cannot_read_host_checkout()
    auth = Path(os.environ.get("CODEX_HOME", Path.home() / ".codex")) / "auth.json"
    if not auth.is_file():
        raise FileNotFoundError("Codex auth unavailable; run `codex login`")

    workspace = output / "workspace"
    materialization = output / "task-materialization.json"
    if workspace.is_symlink():
        raise RuntimeError("candidate workspace must not be a symbolic link")
    if workspace.exists():
        if not workspace.is_dir():
            raise RuntimeError("candidate workspace path exists and is not a directory")
        if any(workspace.iterdir()):
            raise RuntimeError("candidate workspace must be fresh before Harbor launch")
        workspace.rmdir()
    if (workspace.parent / "evaluator").exists():
        raise RuntimeError("candidate workspace parent already has evaluator state")

    task_root = output / "task_root"
    shutil.copytree(
        REPO / "adapters" / "harbor" / "bundles" / config["bundle"], task_root
    )
    workspace_seed = task_root / "environment" / "workspace"
    subprocess.run(
        [
            sys.executable,
            str(materializer),
            "--workspace",
            str(workspace_seed),
            "--manifest",
            str(materialization),
        ],
        check=True,
    )
    candidate_rel = target_rel
    payload = {
        "trace_correlation_id": f"cardbench-magic-card-{instance}-{output.name}",
        "deployment_name": f"cardbench_magic_card_{instance.replace('-', '_')}",
        "codex_timeout_seconds": int(
            os.environ.get("CARDBENCH_HARBOR_TIMEOUT_SEC", "3600")
        ),
        "harbor_agent": {
            "name": "codex",
            "model_name": os.environ.get(
                "CARDBENCH_HARBOR_MODEL", "openai/gpt-5.4-mini"
            ),
            "kwargs": {
                "reasoning_effort": os.environ.get("CARDBENCH_HARBOR_EFFORT", "low"),
                "codex_timeout_seconds": int(
                    os.environ.get("CARDBENCH_HARBOR_TIMEOUT_SEC", "3600")
                ),
            },
        },
        "task_metadata": {
            "workspace_dir": str(workspace),
            "task_id": config["task_id"],
            "benchmark": "cardbench",
            "submission_path": str(candidate_rel),
            "require_fresh_absolute_workspace": True,
            "require_agent_filesystem_isolation": True,
        },
        "submission_path": str(candidate_rel),
        "codex_auth_json_b64": base64.b64encode(auth.read_bytes()).decode("ascii"),
        "codex_auth_source": "host_codex_auth_json",
        "env": {
            "CARDBENCH_REPO_ROOT": str(REPO),
            "CARDBENCH_WORKSPACE_ROOT": str(workspace),
            "CARDBENCH_VERIFIER_OUT": str(output / "logs" / "verifier"),
            "CARDBENCH_MAGIC_TASK_MANIFEST": str(materialization),
            "CARDBENCH_MAGIC_WORKER_IMAGE": os.environ.get(
                "CARDBENCH_MAGIC_WORKER_IMAGE",
                "rust@sha256:d7482085ff5b415f84dba5647ae71606650bdef00db7aeb69f4b3d170c3e4082",
            ),
        },
    }
    rollout = output / "rollout.json"
    rollout.write_text(json.dumps(payload, indent=2) + "\n")
    agent_result = output / "rollout_result.json"
    agent = subprocess.run(
        [
            sys.executable,
            str(CODEX_RUNNER),
            "--input",
            str(rollout),
            "--output",
            str(agent_result),
            "--task-root",
            str(task_root),
        ]
    )
    runner_result = {}
    if agent_result.is_file():
        try:
            runner_result = json.loads(agent_result.read_text())
        except (OSError, json.JSONDecodeError):
            runner_result = {}
    runner_result = runner_result if isinstance(runner_result, dict) else {}
    runner_metadata = runner_result.get("metadata")
    runner_metadata = runner_metadata if isinstance(runner_metadata, dict) else {}
    codex_returncode = runner_metadata.get("codex_returncode")
    task_smoke_returncode = runner_metadata.get("verifier_returncode")
    agent_completed = codex_returncode == 0 and task_smoke_returncode == 0

    remove_runner_staged_tests(workspace)

    candidate = workspace / candidate_rel
    if candidate.is_file():
        scored = subprocess.run(
            score_command(
                "card",
                candidate,
                output,
                instance=instance,
                expansion=config["expansion"],
                variant=config["variant"],
                suite="shown",
                variety="magic",
            )
        )
        verify_rc = scored.returncode
    else:
        verify_rc = 1
        verifier_dir = output / "logs" / "verifier"
        verifier_dir.mkdir(parents=True, exist_ok=True)
        (verifier_dir / "result.json").write_text(
            json.dumps(
                {
                    "schema_version": "cardbench.magic.card-pvp.check.v1",
                    "task_id": "cardbench/magic/card",
                    "passed": False,
                    "cards_passed": 0,
                    "cards_total": 291,
                    "coverage_complete": False,
                    "full_reward": False,
                    "harbor_reward": 0.0,
                    "error": f"missing submission {candidate_rel}",
                },
                indent=2,
            )
            + "\n"
        )
        (verifier_dir / "reward.txt").write_text("0.0\n")
    write_receipt(
        output,
        "card",
        "codex",
        0 if agent_completed else agent.returncode,
        verify_rc,
        "magic",
    )
    receipt_path = output / "lane-receipt.json"
    receipt = json.loads(receipt_path.read_text())
    receipt.update(
        {
            "instance_id": instance,
            "expansion": config["expansion"],
            "variant": config["variant"],
            "task_manifest": str((task_dir / "harbor.toml").relative_to(REPO)),
            "harbor_runner_rc": agent.returncode,
            "runner_success": bool(runner_result.get("success")),
            "codex_returncode": codex_returncode,
            "task_smoke_returncode": task_smoke_returncode,
            "agent_completed": agent_completed,
        }
    )
    receipt_path.write_text(json.dumps(receipt, indent=2) + "\n")
    print(f"receipt: {receipt_path}")
    return 0 if agent_completed and verify_rc == 0 else 1


def remove_runner_staged_tests(workspace: Path) -> None:
    """Remove the shared runner's temporary smoke-test copy before scoring."""
    staged = workspace / ".goex_verifier_tests"
    if staged.is_symlink():
        raise RuntimeError("runner-staged verifier tests must not be a symbolic link")
    if staged.exists():
        if not staged.is_dir():
            raise RuntimeError("runner-staged verifier tests path is not a directory")
        shutil.rmtree(staged)


def assert_harbor_agent_cannot_read_host_checkout() -> None:
    """Fail closed unless the runner's actual agent UID cannot read the repo."""
    runner_directory = str(CODEX_RUNNER.parent)
    if runner_directory not in sys.path:
        sys.path.insert(0, runner_directory)
    prior_bytecode_setting = sys.dont_write_bytecode
    sys.dont_write_bytecode = True
    try:
        runner = importlib.import_module("codex_harbor_runner")
    finally:
        sys.dont_write_bytecode = prior_bytecode_setting
    identity = runner.require_agent_execution_identity()

    protected_card_sources = [
        MAGIC / "sets" / "ravnica_city_of_guilds" / "src" / "cards" / "char.rs",
        MAGIC
        / "sets"
        / "ravnica_city_of_guilds"
        / "src"
        / "cards"
        / "lightning_helix.rs",
    ]
    git_common = subprocess.run(
        ["git", "rev-parse", "--git-common-dir"],
        cwd=REPO,
        check=True,
        capture_output=True,
        text=True,
    ).stdout.strip()
    git_common_dir = Path(git_common)
    if not git_common_dir.is_absolute():
        git_common_dir = REPO / git_common_dir

    protected_paths = (
        [(REPO, "varieties")]
        + [(path, None) for path in protected_card_sources]
        + [(REPO / ".git", "HEAD"), (git_common_dir, "HEAD")]
    )
    probe = r'''
import os,sys
path,kind,child=sys.argv[1],sys.argv[2],sys.argv[3]
try:
 if kind == 'directory':
  accessible=False
  try: os.listdir(path); accessible=True
  except PermissionError: pass
  if child:
   try: os.stat(os.path.join(path,child)); accessible=True
   except PermissionError: pass
  raise SystemExit(0 if accessible else 13)
 with open(path,'rb') as stream: stream.read(1)
except PermissionError: raise SystemExit(13)
except FileNotFoundError: raise SystemExit(14)
except OSError: raise SystemExit(15)
raise SystemExit(0)
'''
    for host_path, child in protected_paths:
        path = host_path.resolve()
        if not path.exists():
            raise RuntimeError(f"required protected host path is missing: {path}")
        kind = "directory" if path.is_dir() else "file"
        access = subprocess.run(
            [sys.executable, "-c", probe, str(path), kind, child or ""],
            env={"PATH": os.environ.get("PATH", "/usr/bin:/bin")},
            stdin=subprocess.DEVNULL,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
            check=False,
            user=identity.uid,
            group=identity.gid,
            extra_groups=[],
        )
        if access.returncode != 13:
            raise RuntimeError(
                "refusing Codex trial: Harbor agent identity can access a "
                f"protected CardBench host path ({path})"
            )


def main() -> int:
    parser = argparse.ArgumentParser(
        usage="./adapters/harbor/run.sh <family> <verify|codex|list> [pokemon]"
    )
    parser.add_argument("family", choices=sorted(FAMILIES))
    parser.add_argument("command", choices=["verify", "codex", "list"])
    parser.add_argument("variety", nargs="?", default="pokemon")
    parser.add_argument("--instance", default="df-097-rayquaza-ex")
    parser.add_argument("--expansion", default="crystal_guardians")
    parser.add_argument("--variant", default="0pct")
    parser.add_argument("--suite", choices=["train", "hidden"], default="train")
    args = parser.parse_args()
    family = FAMILIES[args.family]
    if args.variety not in {"pokemon", "magic"}:
        parser.error("variety must be pokemon or magic")
    if args.command == "list":
        runnable = (
            family in {"code_policy", "deck_opt", "card", "set_engine", "engine"}
            if args.variety == "pokemon"
            else family in {"card", "engine", "set_pvp"}
        )
        mode = (
            "reference-and-candidate-runnable"
            if args.variety == "magic" and family in {"card", "set_pvp"}
            else "reference-runnable"
            if runnable
            else "scaffold"
        )
        print(f"cardbench/{args.variety}/{family}\t{mode}")
        return 0
    runnable = (
        family in {"code_policy", "deck_opt", "card", "set_engine", "engine"}
        if args.variety == "pokemon"
        else family in {"card", "set_pvp"}
        or (family == "engine" and args.command == "verify")
    )
    if not runnable:
        print(f"cardbench/{args.variety}/{family} is scaffold-only", file=sys.stderr)
        return 2
    output = fresh_output(family, args.command)
    output.mkdir(parents=True, exist_ok=True)
    try:
        return (
            run_reference(
                family,
                output,
                instance=args.instance,
                expansion=args.expansion,
                variant=args.variant,
                suite=args.suite,
                variety=args.variety,
            )
            if args.command == "verify"
            else run_codex(
                family,
                output,
                instance=args.instance,
                expansion=args.expansion,
                variant=args.variant,
                suite=args.suite,
                variety=args.variety,
            )
        )
    except Exception as exc:
        print(f"CardBench Harbor failed: {exc}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
