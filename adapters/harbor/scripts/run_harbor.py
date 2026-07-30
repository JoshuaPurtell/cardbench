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
EVALS = Path(os.environ.get("CARDBENCH_EVALS_ROOT", Path.home() / "Documents" / "GitHub" / "evals"))
CODEX_RUNNER = EVALS / "core" / "harbor" / "runner" / "codex_harbor_runner.py"
REFERENCE_POLICY = POKEMON / "candidates" / "reference" / "simple_heuristic_ai.rs"
REFERENCE_DECK = POKEMON / "decks" / "gardevoir_delta_control.json"

FAMILIES = {
    "code-policy": "code_policy",
    "code_policy": "code_policy",
    "deck-opt": "deck_opt",
    "deck_opt": "deck_opt",
    "engine": "engine",
    "react": "react",
    "cybernetic": "cybernetic",
}


def fresh_output(family: str, command: str) -> Path:
    configured = os.environ.get("CARDBENCH_HARBOR_OUT")
    if configured:
        return Path(configured).expanduser().resolve()
    stamp = dt.datetime.now(dt.UTC).strftime("%Y%m%dT%H%M%SZ")
    return REPO / "artifacts" / "harbor" / f"{family}-{command}-{stamp}-{os.getpid()}"


def score_command(family: str, candidate: Path, output: Path) -> list[str]:
    if family == "code_policy":
        return [
            sys.executable,
            str(POKEMON / "scripts" / "run_policy_sweep.py"),
            "--candidate",
            str(candidate),
            "--candidate-id",
            "reference" if candidate == REFERENCE_POLICY else "agent",
            "--output-root",
            str(output / "logs" / "verifier"),
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
    if family == "engine":
        return [
            sys.executable,
            str(POKEMON / "scripts" / "run_engine_parity.py"),
            "--output-root",
            str(output / "logs" / "verifier"),
        ]
    raise ValueError(f"{family} is scaffold-only")


def write_receipt(output: Path, family: str, command: str, agent_rc: int, verify_rc: int) -> None:
    result_path = output / "logs" / "verifier" / "result.json"
    if family == "engine":
        result_path = output / "logs" / "verifier" / "engine-check.json"
    verifier = json.loads(result_path.read_text()) if result_path.exists() else {}
    payload = {
        "schema_version": "cardbench.harbor.lane_receipt.v1",
        "task_id": f"cardbench/pokemon/{family}",
        "family": family,
        "variety": "pokemon",
        "agent": "reference" if command == "verify" else "codex",
        "model": os.environ.get("CARDBENCH_HARBOR_MODEL", "openai/gpt-5.4-mini"),
        "effort": os.environ.get("CARDBENCH_HARBOR_EFFORT", "low"),
        "agent_rc": agent_rc,
        "verify_rc": verify_rc,
        "out_dir": str(output),
        "verifier": verifier,
        "reward": float(verifier.get("harbor_reward", 0.0)),
    }
    for key in ("baseline_score", "best_score", "delta_vs_baseline", "best_candidate_id", "score_metric"):
        if key in verifier:
            payload[key] = verifier[key]
    (output / "lane-receipt.json").write_text(json.dumps(payload, indent=2) + "\n")


def run_reference(family: str, output: Path) -> int:
    candidate = REFERENCE_POLICY if family == "code_policy" else REFERENCE_DECK
    command = score_command(family, candidate, output)
    completed = subprocess.run(command)
    write_receipt(output, family, "verify", 0, completed.returncode)
    print(f"receipt: {output / 'lane-receipt.json'}")
    return completed.returncode


def copy_workspace(destination: Path) -> None:
    def ignore(_path: str, names: list[str]) -> set[str]:
        return {name for name in names if name in {".git", "artifacts", "target", ".cache", "__pycache__"}}

    shutil.copytree(REPO, destination, ignore=ignore)


def run_codex(family: str, output: Path) -> int:
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
    task_root = REPO / "adapters" / "harbor" / "bundles" / family
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
        "env": {"CARDBENCH_WORKSPACE_ROOT": str(workspace)},
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
    if candidate.exists():
        scored = subprocess.run(score_command(family, candidate, output))
        verify_rc = scored.returncode
    else:
        verify_rc = 1
        verifier_dir = output / "logs" / "verifier"
        verifier_dir.mkdir(parents=True, exist_ok=True)
        (verifier_dir / "result.json").write_text(
            json.dumps({"passed": False, "harbor_reward": 0.0, "error": f"missing {candidate_rel}"}, indent=2) + "\n"
        )
        (verifier_dir / "reward.txt").write_text("0.0\n")
    write_receipt(output, family, "codex", agent.returncode, verify_rc)
    print(f"receipt: {output / 'lane-receipt.json'}")
    return 0 if agent.returncode == 0 and verify_rc == 0 else 1


def main() -> int:
    parser = argparse.ArgumentParser(
        usage="./adapters/harbor/run.sh <family> <verify|codex|list> [pokemon]"
    )
    parser.add_argument("family", choices=sorted(FAMILIES))
    parser.add_argument("command", choices=["verify", "codex", "list"])
    parser.add_argument("variety", nargs="?", default="pokemon")
    args = parser.parse_args()
    family = FAMILIES[args.family]
    if args.variety != "pokemon":
        parser.error("only the pokemon variety is runnable; magic is reserved")
    if args.command == "list":
        print(f"cardbench/pokemon/{family}\t{'runnable' if family in {'code_policy', 'deck_opt', 'engine'} else 'scaffold'}")
        return 0
    if family in {"react", "cybernetic"}:
        print(f"cardbench/pokemon/{family} is scaffold-only", file=sys.stderr)
        return 2
    output = fresh_output(family, args.command)
    output.mkdir(parents=True, exist_ok=True)
    try:
        return run_reference(family, output) if args.command == "verify" else run_codex(family, output)
    except Exception as exc:
        print(f"CardBench Harbor failed: {exc}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
