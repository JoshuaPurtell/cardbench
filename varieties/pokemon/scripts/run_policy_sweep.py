#!/usr/bin/env python3
"""Score a Pokemon code policy over a fixed, cell-addressed evaluation surface.

Two splits, both driven by ``rosters/code_policy_v1.json``:

``train``    decks and opponents visible in the agent workspace. Feedback only.
``heldout``  the sealed split (``.sealed/code_policy/heldout_v1.json``).
             Authority: this is what the Dock verdict and Harbor receipt gate on.

A *cell* is one (candidate deck, opponent deck, opponent, seat, seed) coordinate.
Baseline and candidate are scored over exactly the same cells and compared
pairwise, so a lift has to survive a paired bootstrap rather than a difference of
two blended win rates. Coverage is fail-closed: if the sweep does not reproduce
the roster's cell set exactly, the run scores zero rather than reporting a
partial result.
"""

from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
import tempfile
from collections import defaultdict
from pathlib import Path

POKEMON_ROOT = Path(__file__).resolve().parents[1]
REPO_ROOT = POKEMON_ROOT.parents[1]
BENCHMARK = POKEMON_ROOT / "policies" / "benchmark_ai.py"
BASELINE = POKEMON_ROOT / "candidates" / "reference" / "baseline_policy.rs"
ROSTER = POKEMON_ROOT / "rosters" / "code_policy_v1.json"
SEALED = POKEMON_ROOT / ".sealed" / "code_policy"
TRAIN_SERVER_DB = POKEMON_ROOT / "policies" / "data" / "server.sqlite"
CARDS_DB = POKEMON_ROOT / "policies" / "data" / "cards.sqlite"

sys.path.insert(0, str(POKEMON_ROOT / "viz"))
sys.path.insert(0, str(POKEMON_ROOT / "scripts"))
from render_frame import render_frame  # noqa: E402
from stats_gate import PairedScores, significance_gate  # noqa: E402

TASK_ID = "cardbench/pokemon/code_policy"
FAMILY = "cardbench.pokemon.code_policy"
METRICS_MARKER = "BENCHMARK_METRICS_JSON: "

# A heldout lift is only credible if most games actually resolved. Stalled games
# score as non-wins, so a high stall rate would quietly flatten every cell.
MAX_STALL_FRACTION = 0.25


class SweepError(RuntimeError):
    """Raised when the sweep cannot produce a trustworthy score."""


def struct_name(path: Path) -> str:
    match = re.search(r"\bpub\s+struct\s+([A-Za-z_][A-Za-z0-9_]*)", path.read_text())
    if not match:
        raise SweepError(f"{path}: candidate must define a public policy struct")
    return match.group(1)


# --------------------------------------------------------------------------
# split resolution
# --------------------------------------------------------------------------


def load_split(split: str) -> dict:
    """Resolve a split into the concrete surface the sweep must cover."""
    roster = json.loads(ROSTER.read_text())

    if split == "train":
        decks = list(roster["train"]["decks"])
        opponents = list(roster["train"]["opponents"])
        cells = []
        for candidate_deck in decks:
            for opponent_deck in decks:
                if candidate_deck == opponent_deck:
                    continue
                for opponent in opponents:
                    for side in ("p1", "p2"):
                        for seed in roster["train"]["seed_bases"]:
                            cells.append(
                                {
                                    "cell_id": (
                                        f"{candidate_deck}|{opponent_deck}|"
                                        f"{opponent['id']}|{side}|{seed}"
                                    ),
                                    "candidate_deck": candidate_deck,
                                    "opponent_deck": opponent_deck,
                                    "opponent_id": opponent["id"],
                                    "side": side,
                                    "seed_base": seed,
                                }
                            )
        return {
            "split": "train",
            "roster_id": roster["roster_id"],
            "roster_sha256": None,
            "server_db": TRAIN_SERVER_DB,
            "opponents": opponents,
            "cells": cells,
            "matches": int(roster["train"]["matches_per_opponent_per_side"]),
            "seed_bases": list(roster["train"]["seed_bases"]),
        }

    manifest_path = SEALED / "heldout_v1.json"
    if not manifest_path.is_file():
        raise SweepError(
            f"sealed heldout manifest missing: {manifest_path}. Run "
            "varieties/pokemon/scripts/build_sealed_heldout.py first."
        )
    manifest = json.loads(manifest_path.read_text())

    # The committed roster pins the sealed split; a mismatch means the heldout
    # surface moved under us and no score from it is comparable.
    import hashlib

    digest = hashlib.sha256(
        json.dumps(manifest, sort_keys=True, separators=(",", ":")).encode()
    ).hexdigest()
    expected = roster["heldout"]["sha256"]
    if digest != expected:
        raise SweepError(
            f"heldout manifest sha256 {digest} does not match the committed "
            f"roster pin {expected}"
        )

    return {
        "split": "heldout",
        "roster_id": roster["roster_id"],
        "roster_sha256": digest,
        "server_db": SEALED / manifest["server_db"],
        "opponents": [
            {"id": opponent["id"], "source": str(SEALED / opponent["source"])}
            for opponent in manifest["opponents"]
        ],
        "cells": manifest["cells"],
        "matches": int(manifest["matches_per_opponent_per_side"]),
        "seed_bases": list(manifest["seed_bases"]),
    }


# --------------------------------------------------------------------------
# sweep
# --------------------------------------------------------------------------


def build_binary(policy: Path, name: str, roster_path: Path, work_dir: Path) -> Path:
    manifest = work_dir / "manifest.json"
    completed = subprocess.run(
        [
            sys.executable,
            str(BENCHMARK),
            "--mode",
            "build",
            "--ai-code-file",
            str(policy),
            "--name",
            name,
            "--opponent-roster",
            str(roster_path),
            "--work-dir",
            str(work_dir),
            "--manifest-file",
            str(manifest),
        ],
        text=True,
        capture_output=True,
    )
    if completed.returncode != 0:
        combined = completed.stdout + "\n" + completed.stderr
        raise SweepError(f"policy compile failed:\n{combined[-5000:]}")
    return Path(json.loads(manifest.read_text())["binary_path"])


def sweep_policy(
    policy: Path, name: str, surface: dict, roster_path: Path, work_dir: Path
) -> dict[str, dict]:
    """Return ``cell_id -> cell metrics`` for one policy over the whole surface."""
    binary = build_binary(policy, name, roster_path, work_dir)

    pairs = sorted(
        {(cell["candidate_deck"], cell["opponent_deck"]) for cell in surface["cells"]}
    )
    seeds = surface["seed_bases"]

    cells: dict[str, dict] = {}
    for candidate_deck, opponent_deck in pairs:
        for seed in seeds:
            completed = subprocess.run(
                [
                    sys.executable,
                    str(BENCHMARK),
                    "--mode",
                    "run-built",
                    "--name",
                    name,
                    "--binary-path",
                    str(binary),
                    "--server-db",
                    str(surface["server_db"]),
                    "--cards-db",
                    str(CARDS_DB),
                    "--opponent-roster",
                    str(roster_path),
                    "--matches",
                    str(surface["matches"]),
                    "--seed-base",
                    str(seed),
                    "--deck-pair",
                    f"{candidate_deck},{opponent_deck}",
                ],
                text=True,
                capture_output=True,
            )
            combined = completed.stdout + "\n" + completed.stderr
            if completed.returncode != 0:
                raise SweepError(
                    f"sweep failed on {candidate_deck} vs {opponent_deck} "
                    f"seed={seed}:\n{combined[-4000:]}"
                )
            lines = [
                line for line in combined.splitlines() if line.startswith(METRICS_MARKER)
            ]
            if not lines:
                raise SweepError(
                    f"no metrics authority for {candidate_deck} vs {opponent_deck} "
                    f"seed={seed}"
                )
            metrics = json.loads(lines[-1][len(METRICS_MARKER) :])
            for cell in metrics["per_cell"]:
                cell_id = cell["cell_id"]
                if cell_id in cells:
                    raise SweepError(f"duplicate cell in sweep output: {cell_id}")
                cells[cell_id] = cell
    return cells


def assert_covers(cells: dict[str, dict], surface: dict, label: str) -> None:
    """Fail closed unless the sweep reproduced the roster cell set exactly."""
    expected = {cell["cell_id"] for cell in surface["cells"]}
    actual = set(cells)
    missing = sorted(expected - actual)
    extra = sorted(actual - expected)
    if missing or extra:
        raise SweepError(
            f"{label} sweep did not cover the {surface['split']} surface: "
            f"{len(missing)} missing (e.g. {missing[:3]}), "
            f"{len(extra)} unexpected (e.g. {extra[:3]})"
        )

    attempted = sum(int(cell["matches"]) for cell in cells.values())
    stalled = sum(int(cell.get("stalled", 0)) for cell in cells.values())
    if attempted <= 0:
        raise SweepError(f"{label} sweep attempted zero games")
    if stalled / attempted > MAX_STALL_FRACTION:
        raise SweepError(
            f"{label} sweep stalled on {stalled}/{attempted} games "
            f"(> {MAX_STALL_FRACTION:.0%}); scores are not trustworthy"
        )


def write_opponent_roster(surface: dict, path: Path) -> Path:
    path.write_text(json.dumps({"opponents": surface["opponents"]}, indent=2) + "\n")
    return path


# --------------------------------------------------------------------------
# scoring
# --------------------------------------------------------------------------


def score_surface(
    baseline_cells: dict[str, dict], candidate_cells: dict[str, dict], surface: dict
) -> dict:
    ordered = [cell["cell_id"] for cell in surface["cells"]]
    paired = PairedScores(
        unit_ids=tuple(ordered),
        candidate=tuple(float(candidate_cells[cid]["win_rate"]) for cid in ordered),
        champion=tuple(float(baseline_cells[cid]["win_rate"]) for cid in ordered),
    )
    verdict = significance_gate(paired, min_n=len(ordered))
    baseline_score = sum(paired.champion) / len(ordered)
    candidate_score = sum(paired.candidate) / len(ordered)
    return {
        "cell_ids": ordered,
        "baseline_score": baseline_score,
        "candidate_score": candidate_score,
        "delta": candidate_score - baseline_score,
        "verdict": verdict,
        "paired": paired,
    }


def per_opponent_breakdown(cells: dict[str, dict]) -> list[dict]:
    grouped: dict[str, list[float]] = defaultdict(list)
    for cell in cells.values():
        grouped[cell["opponent_id"]].append(float(cell["win_rate"]))
    return [
        {"opponent_id": opponent, "win_rate": sum(rates) / len(rates), "cells": len(rates)}
        for opponent, rates in sorted(grouped.items())
    ]


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


def write_artifacts(
    output: Path,
    *,
    split: str,
    surface: dict,
    scored: dict,
    baseline_cells: dict[str, dict],
    candidate_cells: dict[str, dict],
    candidate_id: str,
) -> None:
    verdict = scored["verdict"]

    leaderboard = {
        "schema_version": "cardbench.leaderboard.v1",
        "task_id": TASK_ID,
        "split": split,
        "score_metric": "cell_win_rate",
        "rows": [
            {
                "candidate_id": "baseline_policy",
                "role": "baseline",
                "score": scored["baseline_score"],
            },
            {
                "candidate_id": candidate_id,
                "role": "candidate",
                "score": scored["candidate_score"],
            },
        ],
    }
    (output / "leaderboard.json").write_text(json.dumps(leaderboard, indent=2) + "\n")

    with (output / "per_cell.jsonl").open("w") as handle:
        for cell_id in scored["cell_ids"]:
            handle.write(
                json.dumps(
                    {
                        "cell_id": cell_id,
                        "baseline_score": baseline_cells[cell_id]["win_rate"],
                        "candidate_score": candidate_cells[cell_id]["win_rate"],
                        "delta": candidate_cells[cell_id]["win_rate"]
                        - baseline_cells[cell_id]["win_rate"],
                        "opponent_id": candidate_cells[cell_id]["opponent_id"],
                        "candidate_deck": candidate_cells[cell_id]["candidate_deck"],
                        "opponent_deck": candidate_cells[cell_id]["opponent_deck"],
                        "side": candidate_cells[cell_id]["side"],
                        "seed_base": candidate_cells[cell_id]["seed_base"],
                    }
                )
                + "\n"
            )

    state = visual_state(scored["candidate_score"], 1.0 - scored["candidate_score"])
    (output / "eventlog.jsonl").write_text(
        json.dumps({"type": "match_summary", "split": split, "state": state}) + "\n"
    )
    render_frame(state, output / "viz" / "match-summary.png")

    if split == "heldout":
        scorecard = {
            "schema_version": "cardbench.pokemon.code_policy.heldout_scorecard.v1",
            "measurement_mode": "heldout",
            "task_id": TASK_ID,
            "roster_id": surface["roster_id"],
            "roster_sha256": surface["roster_sha256"],
            "metric": "cell_win_rate_delta",
            "heldout_n": len(scored["cell_ids"]),
            "matches_per_opponent_per_side": surface["matches"],
            "baseline_score": scored["baseline_score"],
            "candidate_score": scored["candidate_score"],
            "delta": scored["delta"],
            "lift_verdict": verdict.to_payload(),
            "per_opponent": per_opponent_breakdown(candidate_cells),
            "per_cell": [
                {
                    "cell_id": cell_id,
                    "baseline_score": baseline_cells[cell_id]["win_rate"],
                    "candidate_score": candidate_cells[cell_id]["win_rate"],
                    "delta": candidate_cells[cell_id]["win_rate"]
                    - baseline_cells[cell_id]["win_rate"],
                }
                for cell_id in scored["cell_ids"]
            ],
        }
        (output / "heldout_scorecard.json").write_text(
            json.dumps(scorecard, indent=2) + "\n"
        )


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--candidate", type=Path, required=True)
    parser.add_argument("--candidate-id", default="candidate")
    parser.add_argument("--split", choices=["train", "heldout"], default="heldout")
    parser.add_argument(
        "--output-root", type=Path, default=REPO_ROOT / "artifacts" / "policy-sweep"
    )
    args = parser.parse_args()

    output = args.output_root.resolve()
    output.mkdir(parents=True, exist_ok=True)
    result_path = output / "result.json"

    try:
        surface = load_split(args.split)
        candidate = args.candidate.resolve()
        if not candidate.is_file():
            raise SweepError(f"missing candidate policy: {candidate}")

        with tempfile.TemporaryDirectory() as temp:
            temp_root = Path(temp)
            roster_path = write_opponent_roster(
                surface, temp_root / "opponent_roster.json"
            )

            baseline_cells = sweep_policy(
                BASELINE,
                struct_name(BASELINE),
                surface,
                roster_path,
                temp_root / "baseline",
            )
            assert_covers(baseline_cells, surface, "baseline")

            candidate_cells = sweep_policy(
                candidate,
                struct_name(candidate),
                surface,
                roster_path,
                temp_root / "candidate",
            )
            assert_covers(candidate_cells, surface, "candidate")

        scored = score_surface(baseline_cells, candidate_cells, surface)
        verdict = scored["verdict"]
        # Heldout authority requires the paired CI to clear zero. Train is a
        # feedback signal, so a positive mean delta is enough there.
        passed = verdict.accepted if args.split == "heldout" else scored["delta"] > 0.0

        write_artifacts(
            output,
            split=args.split,
            surface=surface,
            scored=scored,
            baseline_cells=baseline_cells,
            candidate_cells=candidate_cells,
            candidate_id=args.candidate_id,
        )

        result = {
            "schema_version": "cardbench.harbor.result.v1",
            "benchmark_family": FAMILY,
            "task_id": TASK_ID,
            "split": args.split,
            "roster_id": surface["roster_id"],
            "roster_sha256": surface["roster_sha256"],
            "score_metric": "cell_win_rate",
            "baseline_score": scored["baseline_score"],
            "best_score": scored["candidate_score"],
            "best_candidate_id": args.candidate_id,
            "delta_vs_baseline": scored["delta"],
            "evaluated_candidate_count": 1,
            "cell_count": len(scored["cell_ids"]),
            "lift_verdict": verdict.to_payload(),
            "leaderboard_path": str(output / "leaderboard.json"),
            "passed": passed,
            "harbor_reward": max(0.0, min(1.0, scored["delta"])) if passed else 0.0,
        }
    except Exception as exc:  # noqa: BLE001 - every failure must still emit authority
        result = {
            "schema_version": "cardbench.harbor.result.v1",
            "benchmark_family": FAMILY,
            "task_id": TASK_ID,
            "split": args.split,
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
