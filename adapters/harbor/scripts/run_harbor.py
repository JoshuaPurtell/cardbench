#!/usr/bin/env python3
"""Unified CardBench Harbor entry point for reference and Codex runs."""

from __future__ import annotations

import argparse
import base64
import datetime as dt
import json
import os
import shutil
import subprocess
import sys
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


EXCLUDED_WORKSPACE_DIRS = {
    ".git",
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
    if variety != "pokemon":
        raise ValueError("cardbench/magic/engine has no Codex bundle yet")
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
            else family == "engine"
        )
        print(f"cardbench/{args.variety}/{family}\t{'reference-runnable' if runnable else 'scaffold'}")
        return 0
    runnable = (
        family in {"code_policy", "deck_opt", "card", "set_engine", "engine"}
        if args.variety == "pokemon"
        else family == "engine"
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
