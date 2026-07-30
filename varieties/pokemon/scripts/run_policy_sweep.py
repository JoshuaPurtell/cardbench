#!/usr/bin/env python3
"""Run a fail-closed Pokemon code-policy sweep and emit Harbor-shaped evidence."""

from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
from pathlib import Path

POKEMON_ROOT = Path(__file__).resolve().parents[1]
REPO_ROOT = POKEMON_ROOT.parents[1]
BENCHMARK = POKEMON_ROOT / "policies" / "benchmark_ai.py"
BASELINE = POKEMON_ROOT / "candidates" / "reference" / "baseline_policy.rs"
DEFAULT_SUITE = POKEMON_ROOT / "defaults" / "policy_smoke_v1.json"
sys.path.insert(0, str(POKEMON_ROOT / "viz"))
from render_frame import render_frame  # noqa: E402


def struct_name(path: Path) -> str:
    match = re.search(r"\bpub\s+struct\s+([A-Za-z_][A-Za-z0-9_]*)", path.read_text())
    if not match:
        raise ValueError(f"{path}: candidate must define a public policy struct")
    return match.group(1)


def evaluate(path: Path, matches: int, seed: int) -> dict:
    command = [
        sys.executable,
        str(BENCHMARK),
        "--ai-code-file",
        str(path),
        "--name",
        struct_name(path),
        "--matches",
        str(matches),
        "--seed-base",
        str(seed),
    ]
    completed = subprocess.run(command, text=True, capture_output=True)
    combined = completed.stdout + "\n" + completed.stderr
    if completed.returncode != 0:
        raise RuntimeError(f"candidate compile/run failed:\n{combined[-5000:]}")
    marker = "BENCHMARK_METRICS_JSON: "
    lines = [line for line in combined.splitlines() if line.startswith(marker)]
    if not lines:
        raise RuntimeError("benchmark produced no metrics authority")
    metrics = json.loads(lines[-1][len(marker) :])
    if int(metrics.get("total_games", 0)) <= 0:
        raise RuntimeError("benchmark completed zero games")
    return metrics


def visual_state(score: float, opponent_score: float) -> dict:
    return {
        "player": {
            "active": {"hp_fraction": max(0.1, score), "energy": 3},
            "bench": [{"hp_fraction": 0.8, "energy": 1}, {"hp_fraction": 1.0, "energy": 0}],
            "prizes": max(0, 6 - round(score * 6)),
            "deck": 31,
            "discard": 8,
        },
        "opponent": {
            "active": {"hp_fraction": max(0.1, opponent_score), "energy": 2},
            "bench": [{"hp_fraction": 0.5, "energy": 2}],
            "prizes": max(0, 6 - round(opponent_score * 6)),
            "deck": 28,
            "discard": 11,
        },
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--candidate", type=Path, required=True)
    parser.add_argument("--candidate-id", default="candidate")
    parser.add_argument("--suite", type=Path, default=DEFAULT_SUITE)
    parser.add_argument("--output-root", type=Path, default=REPO_ROOT / "artifacts" / "policy-smoke")
    args = parser.parse_args()
    output = args.output_root.resolve()
    output.mkdir(parents=True, exist_ok=True)
    suite = json.loads(args.suite.read_text())

    result_path = output / "result.json"
    try:
        seed_scores: list[tuple[dict, dict]] = []
        for seed in suite["seed_bases"]:
            baseline_metrics = evaluate(BASELINE, suite["matches_per_opponent_per_side"], seed)
            candidate_metrics = evaluate(args.candidate.resolve(), suite["matches_per_opponent_per_side"], seed)
            seed_scores.append((baseline_metrics, candidate_metrics))
        baseline = sum(item[0]["overall_win_rate"] for item in seed_scores) / len(seed_scores)
        score = sum(item[1]["overall_win_rate"] for item in seed_scores) / len(seed_scores)
        delta = score - baseline
        passed = delta >= float(suite["minimum_delta"])
        leaderboard = {
            "schema_version": "cardbench.leaderboard.v1",
            "task_id": "cardbench/pokemon/code_policy",
            "score_metric": "blended_win_rate",
            "rows": [
                {"candidate_id": suite["baseline_policy_id"], "role": "baseline", "score": baseline},
                {"candidate_id": args.candidate_id, "role": "candidate", "score": score},
            ],
        }
        (output / "leaderboard.json").write_text(json.dumps(leaderboard, indent=2) + "\n")
        state = visual_state(score, 1.0 - score)
        event = {"type": "match_summary", "seed": suite["seed_bases"][0], "state": state}
        (output / "eventlog.jsonl").write_text(json.dumps(event) + "\n")
        render_frame(state, output / "viz" / "match-summary.png")
        result = {
            "schema_version": "cardbench.harbor.result.v1",
            "benchmark_family": "cardbench.pokemon.code_policy",
            "task_id": "cardbench/pokemon/code_policy",
            "score_metric": "blended_win_rate",
            "baseline_score": baseline,
            "best_score": score,
            "best_candidate_id": args.candidate_id,
            "delta_vs_baseline": delta,
            "evaluated_candidate_count": 1,
            "total_games": sum(item[1]["total_games"] for item in seed_scores),
            "leaderboard_path": str(output / "leaderboard.json"),
            "passed": passed,
            "harbor_reward": max(0.0, min(1.0, delta)),
        }
    except Exception as exc:
        result = {
            "schema_version": "cardbench.harbor.result.v1",
            "benchmark_family": "cardbench.pokemon.code_policy",
            "task_id": "cardbench/pokemon/code_policy",
            "passed": False,
            "harbor_reward": 0.0,
            "error": str(exc),
        }
    result_path.write_text(json.dumps(result, indent=2) + "\n")
    (output / "reward.txt").write_text(f"{result['harbor_reward']:.8f}\n")
    print(json.dumps(result, indent=2))
    return 0 if result["passed"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
