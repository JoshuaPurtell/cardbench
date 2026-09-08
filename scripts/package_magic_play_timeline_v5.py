#!/usr/bin/env python3
"""Package Craftax-style constructed play timelines into cardbench-magic.

Reads tagged decisions-seat-a.json + games.jsonl under
  artifacts/regen-.../play/1v1-constructed/{slug}-{effort}-pairs3/

Emits:
  v5/play/timeline.json
"""

from __future__ import annotations

import argparse
import json
import math
import re
from collections import Counter, defaultdict
from pathlib import Path
from typing import Any

DEFAULT_REGEN = Path(
    "/Users/joshuapurtell/Documents/Codex/2026-08-04/imp/work/magic-multiplayer"
    "/varieties/magic/artifacts/regen-2026-08-05/play/1v1-constructed"
)
DEFAULT_RAV_DEFS = Path(
    "/Users/joshuapurtell/Documents/Codex/2026-08-04/imp/work/magic-multiplayer"
    "/varieties/magic/sets/ravnica_city_of_guilds/src/lib.rs"
)
DEFAULT_OUT = (
    Path(__file__).resolve().parents[1] / "artifacts" / "public-eval" / "cardbench-magic"
)

CAST_RE = re.compile(r"^(?:cast|play)\s+(RAV-[\w-]+)", re.I)
LAND_RE = re.compile(r"^play land\s+(RAV-[\w-]+)", re.I)
ACT_RE = re.compile(r"^activate\s+", re.I)

COHORTS = [
    ("google/gemini-2.5-flash-lite", "gemini-2.5-flash-lite", "low"),
    ("google/gemini-3.1-flash-lite", "gemini-3.1-flash-lite", "low"),
    ("google/gemini-3.5-flash-lite", "gemini-3.5-flash-lite", "low"),
    ("openai/gpt-5.4-nano", "gpt-5.4-nano", "low"),
    ("openai/gpt-5.4-nano", "gpt-5.4-nano", "medium"),
    ("openai/gpt-5.4-nano", "gpt-5.4-nano", "high"),
    ("openai/gpt-5.6-luna", "gpt-5.6-luna", "low"),
    ("openai/gpt-5.6-luna", "gpt-5.6-luna", "medium"),
    ("openai/gpt-5.6-luna", "gpt-5.6-luna", "high"),
]


def dump(path: Path, payload: Any) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(json.dumps(payload, indent=2, sort_keys=True).encode("utf-8") + b"\n")


def load_card_catalog(path: Path) -> dict[str, list[str]]:
    if not path.exists():
        return {}
    text = path.read_text(encoding="utf-8")
    catalog: dict[str, list[str]] = {}
    for chunk in re.split(r"CardDefinition\s*\{", text)[1:]:
        window = chunk[:2200]
        id_match = re.search(r'id:\s*"([^"]+)"', window)
        if not id_match:
            continue
        types = re.findall(r"CardType::(\w+)", window)
        catalog[id_match.group(1)] = types
    return catalog


def parse_action(options: list[str], chosen: Any) -> tuple[str, str | None]:
    if not isinstance(chosen, int) or chosen < 0 or chosen >= len(options):
        return "unknown", None
    label = options[chosen]
    if label.startswith("pass"):
        return "pass", None
    land = LAND_RE.match(label)
    if land:
        return "land", land.group(1)
    cast = CAST_RE.match(label)
    if cast:
        return "cast", cast.group(1)
    if ACT_RE.match(label):
        return "activate", None
    return "other", None


def percentile(values: list[float], ratio: float) -> float:
    if not values:
        return 0.0
    ordered = sorted(values)
    if len(ordered) == 1:
        return ordered[0]
    pos = (len(ordered) - 1) * ratio
    lo = math.floor(pos)
    hi = math.ceil(pos)
    if lo == hi:
        return ordered[lo]
    weight = pos - lo
    return ordered[lo] * (1.0 - weight) + ordered[hi] * weight


def mean(values: list[float]) -> float:
    return sum(values) / len(values) if values else 0.0


def segment_games(decisions: list[dict[str, Any]]) -> dict[str, list[dict[str, Any]]]:
    """Group by game_id when present; otherwise refuse (interleaved logs)."""
    by_game: dict[str, list[dict[str, Any]]] = defaultdict(list)
    for row in decisions:
        game_id = row.get("game_id")
        if not game_id:
            continue
        by_game[str(game_id)].append(row)
    return dict(by_game)


def life_at_turn(game: list[dict[str, Any]]) -> dict[int, tuple[float, float]]:
    """First observation of own/opp life on each turn for one game."""
    out: dict[int, tuple[float, float]] = {}
    for row in sorted(game, key=lambda r: (int(r.get("turn") or 0), r.get("step") or "")):
        turn = int(row.get("turn") or 0)
        if turn in out or turn <= 0:
            continue
        own = row.get("own_life")
        opp = row.get("opponent_life") or []
        opp_life = opp[0][1] if opp and isinstance(opp[0], (list, tuple)) else None
        if own is None or opp_life is None:
            continue
        out[turn] = (float(own), float(opp_life))
    return out


def package_cohort(
    *,
    model: str,
    effort: str,
    root: Path,
    catalog: dict[str, list[str]],
) -> dict[str, Any] | None:
    decisions_path = root / "decisions-seat-a.json"
    if not decisions_path.exists():
        return None
    decisions = json.loads(decisions_path.read_text())
    games = segment_games(decisions)
    if not games:
        return None

    games_meta: dict[str, dict[str, Any]] = {}
    games_path = root / "games.jsonl"
    if games_path.exists():
        for line in games_path.read_text().splitlines():
            if not line.strip():
                continue
            row = json.loads(line)
            games_meta[str(row.get("game_id"))] = row

    cast_counts: Counter[str] = Counter()
    type_counts: Counter[str] = Counter()
    action_counts: Counter[str] = Counter()
    end_turns: list[int] = []
    # turn -> list of (own, opp, lead) across games still alive
    life_samples: dict[int, list[tuple[float, float, float]]] = defaultdict(list)
    # per game: cumulative casts by turn
    cast_curves: list[dict[int, int]] = []
    type_curves: list[dict[int, dict[str, int]]] = []

    for game_id, rows in games.items():
        meta = games_meta.get(game_id) or {}
        end_turn = int(meta.get("turns") or max((int(r.get("turn") or 0) for r in rows), default=0))
        if end_turn:
            end_turns.append(end_turn)

        life_map = life_at_turn(rows)
        for turn, (own, opp) in life_map.items():
            life_samples[turn].append((own, opp, own - opp))

        cum = 0
        cum_types: Counter[str] = Counter()
        by_turn_casts: dict[int, int] = {}
        by_turn_types: dict[int, dict[str, int]] = {}
        for row in sorted(rows, key=lambda r: (int(r.get("turn") or 0), r.get("step") or "")):
            turn = int(row.get("turn") or 0)
            kind, card = parse_action(list(row.get("options") or []), row.get("chosen"))
            action_counts[kind] += 1
            if kind == "land" and card:
                type_counts["Land"] += 1
            elif kind == "cast" and card:
                cum += 1
                cast_counts[card] += 1
                for ty in catalog.get(card) or ["Unknown"]:
                    type_counts[ty] += 1
                    cum_types[ty] += 1
            by_turn_casts[turn] = cum
            by_turn_types[turn] = dict(cum_types)
        cast_curves.append(by_turn_casts)
        type_curves.append(by_turn_types)

    max_turn = max(life_samples.keys() | {t for curve in cast_curves for t in curve}, default=1)
    # Forward-fill cast curves so every turn has a value for averaging.
    filled_cast: list[list[float]] = []
    for curve in cast_curves:
        series = []
        last = 0
        for turn in range(1, max_turn + 1):
            if turn in curve:
                last = curve[turn]
            series.append(float(last))
        filled_cast.append(series)

    life_by_turn = []
    casts_by_turn = []
    for turn in range(1, max_turn + 1):
        samples = life_samples.get(turn) or []
        owns = [s[0] for s in samples]
        opps = [s[1] for s in samples]
        leads = [s[2] for s in samples]
        life_by_turn.append(
            {
                "turn": turn,
                "n_games_alive": len(samples),
                "own_mean": round(mean(owns), 2) if owns else None,
                "own_p25": round(percentile(owns, 0.25), 2) if owns else None,
                "own_p75": round(percentile(owns, 0.75), 2) if owns else None,
                "opp_mean": round(mean(opps), 2) if opps else None,
                "lead_mean": round(mean(leads), 2) if leads else None,
                "lead_p25": round(percentile(leads, 0.25), 2) if leads else None,
                "lead_p75": round(percentile(leads, 0.75), 2) if leads else None,
            }
        )
        cast_vals = [series[turn - 1] for series in filled_cast if len(series) >= turn]
        # type breakdown at this turn (mean cumulative)
        type_means: dict[str, float] = defaultdict(float)
        for curve in type_curves:
            last: dict[str, int] = {}
            for t in range(1, turn + 1):
                if t in curve:
                    last = curve[t]
            for ty, count in last.items():
                type_means[ty] += count
        n_curves = max(len(type_curves), 1)
        casts_by_turn.append(
            {
                "turn": turn,
                "casts_mean": round(mean(cast_vals), 3) if cast_vals else 0.0,
                "by_type": {
                    ty: round(val / n_curves, 3) for ty, val in sorted(type_means.items())
                },
            }
        )

    end_hist: Counter[int] = Counter(end_turns)
    n_games = len(games)
    return {
        "model": model,
        "effort": effort,
        "n_games": n_games,
        "n_decisions": len(decisions),
        "source": str(root),
        "life_by_turn": life_by_turn,
        "casts_by_turn": casts_by_turn,
        "action_mix": {
            "pass": action_counts.get("pass", 0),
            "land": action_counts.get("land", 0),
            "cast": action_counts.get("cast", 0),
            "activate": action_counts.get("activate", 0),
            "other": action_counts.get("other", 0) + action_counts.get("unknown", 0),
            "pass_rate": round(action_counts.get("pass", 0) / max(sum(action_counts.values()), 1), 3),
        },
        "type_mix": dict(type_counts),
        "type_mix_per_game": {
            ty: round(count / max(n_games, 1), 2) for ty, count in type_counts.items()
        },
        "top_casts": [
            {"card": card, "count": count, "per_game": round(count / max(n_games, 1), 2)}
            for card, count in cast_counts.most_common(12)
        ],
        "end_turn_hist": [
            {"turn": turn, "count": count} for turn, count in sorted(end_hist.items())
        ],
        "mean_end_turn": round(mean([float(t) for t in end_turns]), 2) if end_turns else None,
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--regen", type=Path, default=DEFAULT_REGEN)
    parser.add_argument("--out", type=Path, default=DEFAULT_OUT)
    parser.add_argument("--defs", type=Path, default=DEFAULT_RAV_DEFS)
    args = parser.parse_args()

    catalog = load_card_catalog(args.defs)
    cohorts = []
    for model, slug, effort in COHORTS:
        root = args.regen / f"{slug}-{effort}-pairs3"
        # Prefer freshly tagged timeline dirs if present.
        timeline_root = args.regen / f"{slug}-{effort}-timeline-pairs3"
        if (timeline_root / "decisions-seat-a.json").exists():
            # Prefer timeline dir when it has game_id tags.
            sample = json.loads((timeline_root / "decisions-seat-a.json").read_text())
            if sample and sample[0].get("game_id"):
                root = timeline_root
        packaged = package_cohort(model=model, effort=effort, root=root, catalog=catalog)
        if packaged:
            cohorts.append(packaged)
            print(f"ok {model} [{effort}] games={packaged['n_games']} casts={packaged['action_mix']['cast']}")
        else:
            print(f"skip {model} [{effort}] missing tagged decisions at {root}")

    payload = {
        "schema": "cardbench_magic_play_timeline_v1",
        "lane": "constructed_1v1",
        "baseline": "v7",
        "pairs": 3,
        "cohorts": cohorts,
        "coverage": {
            "cohorts": len(cohorts),
            "expected": len(COHORTS),
            "catalog_cards": len(catalog),
        },
    }
    out_path = args.out / "v5" / "play" / "timeline.json"
    dump(out_path, payload)
    print(f"wrote {out_path} cohorts={len(cohorts)}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
