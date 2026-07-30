#!/usr/bin/env python3
"""Choose-from-pool deck evaluation under a frozen reference code policy."""

from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
import tempfile
from collections import Counter
from pathlib import Path

POKEMON_ROOT = Path(__file__).resolve().parents[1]
REPO_ROOT = POKEMON_ROOT.parents[1]
BENCHMARK = POKEMON_ROOT / "policies" / "benchmark_ai.py"
POLICY = POKEMON_ROOT / "candidates" / "reference" / "simple_heuristic_ai.rs"
DEFAULT_SUITE = POKEMON_ROOT / "defaults" / "deck_choose_v1.json"
DECK_POOL = POKEMON_ROOT / "decks"
sys.path.insert(0, str(POKEMON_ROOT / "viz"))
from render_frame import render_frame  # noqa: E402


def load_legal_deck(path: Path) -> tuple[str, dict]:
    raw = json.loads(path.read_text())
    name = raw.get("name")
    cards = raw.get("cards")
    if not isinstance(name, str) or not isinstance(cards, list) or not cards:
        raise ValueError("deck must contain name and non-empty cards")
    counts: Counter[str] = Counter()
    for entry in cards:
        card_id = entry.get("card_def_id")
        count = entry.get("count")
        if not isinstance(card_id, str) or not isinstance(count, int) or count <= 0:
            raise ValueError("each card entry requires card_def_id and positive integer count")
        counts[card_id] += count
    if sum(counts.values()) != 60:
        raise ValueError("Pokemon deck must contain exactly 60 cards")
    if sum(count for card, count in counts.items() if card.upper().startswith("ENERGY-")) < 1:
        raise ValueError("deck must contain at least one basic Energy")
    illegal = {card: count for card, count in counts.items() if not card.upper().startswith("ENERGY-") and count > 4}
    if illegal:
        raise ValueError(f"non-Energy copy limit exceeded: {illegal}")
    pool = {json.loads(item.read_text()).get("name"): item for item in DECK_POOL.glob("*.json")}
    if name not in pool:
        raise ValueError(f"deck {name!r} is not in the shown choose-from-pool set")
    if json.loads(pool[name].read_text()).get("cards") != cards:
        raise ValueError("candidate deck contents do not match the selected shown-pool deck")
    return name, raw


def struct_name(path: Path) -> str:
    return re.search(r"\bpub\s+struct\s+(\w+)", path.read_text()).group(1)


def parse_metrics(text: str) -> dict:
    marker = "BENCHMARK_METRICS_JSON: "
    lines = [line for line in text.splitlines() if line.startswith(marker)]
    if not lines:
        raise RuntimeError("fixed-policy benchmark produced no metrics")
    metrics = json.loads(lines[-1][len(marker) :])
    if metrics.get("total_games", 0) <= 0:
        raise RuntimeError("fixed-policy benchmark completed zero games")
    return metrics


def run_command(command: list[str]) -> str:
    completed = subprocess.run(command, text=True, capture_output=True)
    combined = completed.stdout + "\n" + completed.stderr
    if completed.returncode != 0:
        raise RuntimeError(combined[-5000:])
    return combined


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--candidate", type=Path, required=True)
    parser.add_argument("--candidate-id", default="candidate-deck")
    parser.add_argument("--suite", type=Path, default=DEFAULT_SUITE)
    parser.add_argument("--output-root", type=Path, default=REPO_ROOT / "artifacts" / "deck-smoke")
    args = parser.parse_args()
    output = args.output_root.resolve()
    output.mkdir(parents=True, exist_ok=True)
    result_path = output / "result.json"
    try:
        selected_name, _ = load_legal_deck(args.candidate.resolve())
        suite = json.loads(args.suite.read_text())
        with tempfile.TemporaryDirectory() as temp:
            manifest = Path(temp) / "manifest.json"
            run_command(
                [
                    sys.executable,
                    str(BENCHMARK),
                    "--mode",
                    "build",
                    "--ai-code-file",
                    str(POLICY),
                    "--name",
                    struct_name(POLICY),
                    "--work-dir",
                    str(Path(temp) / "build"),
                    "--manifest-file",
                    str(manifest),
                ]
            )
            binary = json.loads(manifest.read_text())["binary_path"]

            def score(deck_name: str) -> tuple[float, int]:
                values = []
                games = 0
                for seed in suite["seed_bases"]:
                    text = run_command(
                        [
                            sys.executable,
                            str(BENCHMARK),
                            "--mode",
                            "run-built",
                            "--name",
                            struct_name(POLICY),
                            "--binary-path",
                            binary,
                            "--matches",
                            str(suite["matches_per_opponent_per_side"]),
                            "--seed-base",
                            str(seed),
                            "--deck-name",
                            deck_name,
                        ]
                    )
                    metrics = parse_metrics(text)
                    values.append(float(metrics["overall_win_rate"]))
                    games += int(metrics["total_games"])
                return sum(values) / len(values), games

            baseline, _ = score(suite["baseline_deck"])
            candidate_score, games = score(selected_name)
        delta = candidate_score - baseline
        passed = delta >= float(suite["minimum_delta"])
        leaderboard = {
            "schema_version": "cardbench.leaderboard.v1",
            "task_id": "cardbench/pokemon/deck_opt",
            "score_metric": "fixed_policy_blended_win_rate",
            "scoring_policy_id": suite["scoring_policy_id"],
            "rows": [
                {"candidate_id": suite["baseline_deck"], "role": "baseline", "score": baseline},
                {"candidate_id": args.candidate_id, "deck_name": selected_name, "role": "candidate", "score": candidate_score},
            ],
        }
        (output / "leaderboard.json").write_text(json.dumps(leaderboard, indent=2) + "\n")
        state = {
            "player": {"active": {"hp_fraction": max(0.1, candidate_score), "energy": 3}, "bench": [{}, {}], "prizes": 3, "deck": 30, "discard": 9},
            "opponent": {"active": {"hp_fraction": max(0.1, 1 - candidate_score), "energy": 2}, "bench": [{}], "prizes": 4, "deck": 27, "discard": 12},
        }
        (output / "eventlog.jsonl").write_text(json.dumps({"type": "deck_match_summary", "state": state}) + "\n")
        render_frame(state, output / "viz" / "match-summary.png")
        result = {
            "schema_version": "cardbench.harbor.result.v1",
            "benchmark_family": "cardbench.pokemon.deck_opt",
            "task_id": "cardbench/pokemon/deck_opt",
            "score_metric": "fixed_policy_blended_win_rate",
            "scoring_policy_id": suite["scoring_policy_id"],
            "baseline_score": baseline,
            "best_score": candidate_score,
            "best_candidate_id": args.candidate_id,
            "selected_deck": selected_name,
            "delta_vs_baseline": delta,
            "total_games": games,
            "leaderboard_path": str(output / "leaderboard.json"),
            "passed": passed,
            "harbor_reward": max(0.0, min(1.0, delta)),
        }
    except Exception as exc:
        result = {
            "schema_version": "cardbench.harbor.result.v1",
            "benchmark_family": "cardbench.pokemon.deck_opt",
            "task_id": "cardbench/pokemon/deck_opt",
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
