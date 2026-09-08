#!/usr/bin/env python3
"""Package a Harbor code_policy verify artifact into a Craftax-shaped public matrix.

Writes matrix.json, leaderboard.json, featured cell digests, board frames, and a
Gemini 2.5 Flash Lite (low) ReAct focus stub for the first planned play model.
"""

from __future__ import annotations

import argparse
import json
import shutil
import statistics
import sys
from collections import defaultdict
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
VIZ = ROOT / "varieties" / "pokemon" / "viz"
sys.path.insert(0, str(VIZ))
from render_frame import render_frame  # noqa: E402


def load_jsonl(path: Path) -> list[dict]:
    return [json.loads(line) for line in path.read_text().splitlines() if line.strip()]


def summarize_split(name: str, cells: list[dict], leaderboard: dict, eventlog_rows: list[dict], out: Path) -> dict:
    cand_wins = cand_games = base_wins = base_games = 0
    turns: list[int] = []
    damage: list[int] = []
    deck_matchups: dict[str, dict] = defaultdict(
        lambda: {
            "n": 0,
            "cand_score_sum": 0.0,
            "base_score_sum": 0.0,
            "delta_sum": 0.0,
        }
    )
    for cell in cells:
        key = f"{cell['candidate_deck']} vs {cell['opponent_deck']}"
        matchup = deck_matchups[key]
        matchup["n"] += 1
        matchup["cand_score_sum"] += float(cell["candidate_score"])
        matchup["base_score_sum"] += float(cell["baseline_score"])
        matchup["delta_sum"] += float(cell["delta"])
        matchup["candidate_deck"] = cell["candidate_deck"]
        matchup["opponent_deck"] = cell["opponent_deck"]
        matchup["opponent_id"] = cell["opponent_id"]
        for tel in cell.get("candidate_event_telemetry") or []:
            cand_games += 1
            if tel.get("won"):
                cand_wins += 1
            if "turns" in tel:
                turns.append(int(tel["turns"]))
            if "total_damage_dealt" in tel:
                damage.append(int(tel["total_damage_dealt"]))
        for tel in cell.get("baseline_event_telemetry") or []:
            base_games += 1
            if tel.get("won"):
                base_wins += 1

    featured = sorted(cells, key=lambda cell: abs(float(cell["delta"])), reverse=True)[:12]
    matchups = []
    for key, matchup in sorted(
        deck_matchups.items(),
        key=lambda item: -abs(item[1]["delta_sum"] / max(item[1]["n"], 1)),
    ):
        n = matchup["n"]
        matchups.append(
            {
                "id": key,
                "candidate_deck": matchup["candidate_deck"],
                "opponent_deck": matchup["opponent_deck"],
                "opponent_id": matchup["opponent_id"],
                "n": n,
                "candidate_score": matchup["cand_score_sum"] / n,
                "baseline_score": matchup["base_score_sum"] / n,
                "delta": matchup["delta_sum"] / n,
            }
        )

    frames = []
    for index, row in enumerate(eventlog_rows):
        state = row.get("state")
        if not isinstance(state, dict):
            continue
        path = out / "frames" / f"{name}_summary_{index}.png"
        render_frame(state, path)
        frames.append(
            {
                "id": f"{name}_summary_{index}",
                "path": f"v0/frames/{name}_summary_{index}.png",
                "split": name,
                "source": row.get("type", "match_summary"),
                "width": 256,
                "height": 160,
            }
        )

    return {
        "split": name,
        "cell_count": len(cells),
        "leaderboard": leaderboard,
        "candidate_game_win_rate": (cand_wins / cand_games) if cand_games else None,
        "baseline_game_win_rate": (base_wins / base_games) if base_games else None,
        "candidate_games": cand_games,
        "baseline_games": base_games,
        "turns": {
            "mean": statistics.mean(turns) if turns else None,
            "median": statistics.median(turns) if turns else None,
            "n": len(turns),
        },
        "damage": {
            "mean": statistics.mean(damage) if damage else None,
            "median": statistics.median(damage) if damage else None,
            "n": len(damage),
        },
        "matchups": matchups[:40],
        "featured_cells": [
            {
                "cell_id": cell["cell_id"],
                "candidate_deck": cell["candidate_deck"],
                "opponent_deck": cell["opponent_deck"],
                "opponent_id": cell["opponent_id"],
                "side": cell["side"],
                "seed_base": cell["seed_base"],
                "candidate_score": cell["candidate_score"],
                "baseline_score": cell["baseline_score"],
                "delta": cell["delta"],
            }
            for cell in featured
        ],
        "frames": frames,
        "_featured_full": featured,
    }


def package(artifact_dir: Path, out: Path) -> dict:
    out.mkdir(parents=True, exist_ok=True)
    (out / "cells").mkdir(exist_ok=True)
    (out / "frames").mkdir(exist_ok=True)
    (out / "viz").mkdir(exist_ok=True)

    receipt = json.loads((artifact_dir / "lane-receipt.json").read_text())
    heldout_cells = load_jsonl(artifact_dir / "logs/verifier/per_cell.jsonl")
    train_cells = load_jsonl(artifact_dir / "logs/verifier/train/per_cell.jsonl")
    heldout_lb = json.loads((artifact_dir / "logs/verifier/leaderboard.json").read_text())
    train_lb = json.loads((artifact_dir / "logs/verifier/train/leaderboard.json").read_text())
    eventlog = load_jsonl(artifact_dir / "logs/verifier/eventlog.jsonl")
    train_eventlog = load_jsonl(artifact_dir / "logs/verifier/train/eventlog.jsonl")

    heldout = summarize_split("heldout", heldout_cells, heldout_lb, eventlog, out)
    train = summarize_split("train", train_cells, train_lb, train_eventlog, out)

    for cell in heldout["_featured_full"]:
        full = next(item for item in heldout_cells if item["cell_id"] == cell["cell_id"])
        digest = {
            "schema": "cardbench.cell_digest.v1",
            "family": "code_policy",
            "variety": "pokemon",
            **full,
        }
        safe = cell["cell_id"].replace("|", "__").replace(" ", "_")
        (out / "cells" / f"{safe}.json").write_text(json.dumps(digest, indent=2, sort_keys=True) + "\n")

    for src, name in [
        (artifact_dir / "logs/verifier/viz/match-summary.png", "heldout_match_summary.png"),
        (artifact_dir / "logs/verifier/train/viz/match-summary.png", "train_match_summary.png"),
    ]:
        if src.exists():
            shutil.copy2(src, out / "viz" / name)

    gemini_react = {
        "model": "google/gemini-2.5-flash-lite",
        "display_name": "Gemini 2.5 Flash Lite",
        "effort": "low",
        "family": "react",
        "variety": "pokemon",
        "status": "pending",
        "reason": "react_family_p2_and_awaiting_provider_backed_smoke",
        "planned_opponent": "reference code_policy baseline",
        "planned_smoke": {"seeds": [1, 2, 3, 4, 5, 6, 7, 8], "agency_gate": 0.95},
        "n": 0,
        "score": None,
    }
    code_policy_cohort = {
        "model": receipt.get("model", "openai/gpt-5.4-mini"),
        "display_name": "Harbor · gpt-5.4-mini",
        "effort": receipt.get("effort", "low"),
        "family": "code_policy",
        "variety": "pokemon",
        "status": "complete",
        "agent": receipt.get("agent"),
        "candidate_id": receipt.get("best_candidate_id"),
        "n": receipt.get("cell_count"),
        "score": receipt.get("best_score"),
        "baseline_score": receipt.get("baseline_score"),
        "delta_vs_baseline": receipt.get("delta_vs_baseline"),
        "lift_verdict": receipt.get("lift_verdict"),
        "score_metric": receipt.get("score_metric"),
    }

    def public_split(summary: dict) -> dict:
        return {key: value for key, value in summary.items() if not key.startswith("_")}

    matrix = {
        "schema": "cardbench.public_matrix.v0",
        "benchmark_id": "cardbench/pokemon",
        "status": "pilot",
        "variety": "pokemon",
        "coverage": {
            "families": {
                "code_policy": "complete",
                "deck_opt": "not_packaged",
                "react": "pending_first_model",
                "magic": "reserved",
            },
            "expected_react_models": 1,
            "completed_react_models": 0,
            "code_policy_cells": {
                "heldout": heldout["cell_count"],
                "train": train["cell_count"],
            },
        },
        "source_runs": [
            {
                "run_id": artifact_dir.name,
                "task_id": receipt.get("task_id"),
                "family": "code_policy",
                "agent": receipt.get("agent"),
                "model": receipt.get("model"),
                "effort": receipt.get("effort"),
                "passed": receipt.get("verifier", {}).get("passed"),
            }
        ],
        "cohorts": [code_policy_cohort, gemini_react],
        "splits": {
            "heldout": public_split(heldout),
            "train": public_split(train),
        },
        "trace_status": {
            "board_frames": "partial",
            "eventlog_summaries": "available",
            "react_decision_series": "pending",
            "play_export": "partial",
        },
        "first_model_focus": {
            "model": "google/gemini-2.5-flash-lite",
            "effort": "low",
            "family": "react",
            "note": (
                "Initialize the public CardBench surface with Gemini 2.5 Flash Lite "
                "low as the first ReAct play model. Packaged evidence today is the "
                "code_policy ladder ReAct will sit on."
            ),
        },
    }

    (out / "matrix.json").write_text(json.dumps(matrix, indent=2, sort_keys=True) + "\n")
    (out / "leaderboard.json").write_text(
        json.dumps(
            {
                "schema": "cardbench.public_leaderboard.v0",
                "heldout": heldout_lb,
                "train": train_lb,
            },
            indent=2,
            sort_keys=True,
        )
        + "\n"
    )
    (out / "gemini_25_flash_lite_low.json").write_text(
        json.dumps(
            {
                "schema": "cardbench.model_focus.v0",
                "model": "google/gemini-2.5-flash-lite",
                "effort": "low",
                "family": "react",
                "status": "pending",
                "depends_on": "code_policy reference opponents",
                "code_policy_ladder": {
                    "heldout_candidate_score": receipt.get("best_score"),
                    "heldout_baseline_score": receipt.get("baseline_score"),
                    "delta": receipt.get("delta_vs_baseline"),
                    "lift_accepted": receipt.get("lift_verdict", {}).get("accepted"),
                    "n": receipt.get("cell_count"),
                },
                "next_steps": [
                    "Set provider credentials and run react smoke seeds 1–8",
                    "Agency-gate ≥95% legal actions before publishing win rate",
                    "Emit Trace V5 decision series + board frames per seed",
                    "Replace pending cohort in matrix.json",
                ],
            },
            indent=2,
            sort_keys=True,
        )
        + "\n"
    )
    return {
        "out": str(out),
        "heldout_cells": heldout["cell_count"],
        "train_cells": train["cell_count"],
        "frames": len(list((out / "frames").glob("*.png"))),
        "cell_digests": len(list((out / "cells").glob("*.json"))),
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--artifact",
        type=Path,
        default=ROOT
        / "artifacts/harbor/code_policy-verify-20260801T182535Z-67269",
    )
    parser.add_argument("--out", type=Path, required=True)
    args = parser.parse_args()
    if not args.artifact.exists():
        raise SystemExit(f"missing artifact dir: {args.artifact}")
    print(json.dumps(package(args.artifact, args.out), indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
