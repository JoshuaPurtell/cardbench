#!/usr/bin/env python3
"""Package LLM 2HG / Commander / draft-tournament evidence into cardbench-magic.

Merges into the existing draft matrix:
  - full_draft_tournament_elo per seat (from tournament/summary.json)
  - cohort elo_2hg / elo_commander from arena receipts (WR → Elo vs 1500 code field)

Also writes format replay trees under v5/2hg, v5/commander, v5/tournament.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import re
from pathlib import Path
from typing import Any

DEFAULT_REGEN = Path(
    "/Users/joshuapurtell/Documents/Codex/2026-08-04/imp/work/magic-multiplayer"
    "/varieties/magic/artifacts/regen-2026-08-05"
)
DEFAULT_OUT = (
    Path(__file__).resolve().parents[1] / "artifacts" / "public-eval" / "cardbench-magic"
)
CHOICE_RE = re.compile(r'\{\s*"choice"\s*:\s*(\d+).*?\}', re.S)
REASON_RE = re.compile(r'"reasoning"\s*:\s*"((?:\\.|[^"\\])*)"')


def sha256_bytes(body: bytes) -> str:
    return hashlib.sha256(body).hexdigest()


def dump(path: Path, payload: Any) -> bytes:
    body = json.dumps(payload, indent=2, sort_keys=True).encode("utf-8") + b"\n"
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(body)
    return body


def wr_to_elo(win_rate: float, baseline: float = 1500.0) -> float:
    wr = min(max(win_rate, 0.02), 0.98)
    return round(baseline + 400.0 * math.log10(wr / (1.0 - wr)), 1)


def parse_reply(reply: str) -> tuple[int | None, str]:
    choice_match = CHOICE_RE.search(reply or "")
    reason_match = REASON_RE.search(reply or "")
    choice = int(choice_match.group(1)) if choice_match else None
    reasoning = ""
    if reason_match:
        reasoning = bytes(reason_match.group(1), "utf-8").decode("unicode_escape")
    return choice, reasoning


def model_slug(provider: str) -> str:
    return (
        provider.replace("google/", "")
        .replace("openai/", "")
        .replace(".", "-")
        .replace("_", "-")
    )


def decisions_to_replay(
    *,
    trace_id: str,
    model: str,
    arm: str,
    decisions: list[dict[str, Any]],
    receipt: dict[str, Any],
    source: str,
) -> dict[str, Any]:
    steps = []
    frames = []
    for index, record in enumerate(decisions, start=1):
        options = list(record.get("options") or [])
        if not options and not (record.get("reply") or "").strip():
            continue
        reply = str(record.get("reply") or "")
        parsed, reasoning = parse_reply(reply)
        chosen = record.get("chosen")
        if chosen is None:
            chosen = parsed
        turn = int(record.get("turn") or 0)
        step = str(record.get("step") or "")
        chosen_label = (
            options[chosen] if isinstance(chosen, int) and 0 <= chosen < len(options) else None
        )
        own_life = record.get("own_life")
        opp = record.get("opponent_life") or []
        opp_life = opp[0][1] if opp and isinstance(opp[0], (list, tuple)) else None
        observation = "\n".join(
            [
                f"Turn {turn} · {step}",
                f"Life {own_life} / opp {opp_life}",
                "",
                "Legal menu:",
                *[f"{i}. {label}" for i, label in enumerate(options)],
            ]
        )
        title = f"T{turn} {step}"
        if chosen_label:
            title = f"{title} · {chosen_label}"
        steps.append(
            {
                "id": f"decision-{index}",
                "index": index,
                "kind": "policy_call",
                "title": title,
                "content": {
                    "observation": observation,
                    "reasoning": reasoning or None,
                    "message": reply.strip() or None,
                },
                "action": {
                    "planned": [f"choice:{chosen}"] if chosen is not None else [],
                    "executed": (
                        [{"turn": turn, "name": chosen_label}] if chosen_label else []
                    ),
                    "rejected": [],
                },
                "raw": {
                    "turn": turn,
                    "step": step,
                    "own_life": own_life,
                    "opponent_life": opp_life,
                    "choice": chosen,
                    "note": record.get("note"),
                },
            }
        )
        frames.append(
            {
                "index": index,
                "turn": turn,
                "step": step,
                "own_life": own_life,
                "opponent_life": opp_life,
                "options": options,
                "chosen_index": chosen,
                "chosen_label": chosen_label,
                "reasoning": reasoning or None,
                "command_tax": record.get("command_tax"),
            }
        )
    return {
        "schema": f"cardbench_magic_{arm}_replay_v1",
        "id": trace_id,
        "model": model,
        "arm": arm,
        "success": bool(receipt.get("valid")),
        "score": round(float(receipt.get("win_rate") or 0.0) * 100.0, 1),
        "frames": frames,
        "receipt": receipt,
        "trace": {
            "schema": "eval_trace_view_v1",
            "source_schema": f"cardbench.magic.arena-{arm}-decision-record.v1",
            "trace_id": trace_id,
            "task": {
                "id": f"cardbench-magic-{arm}",
                "name": f"CardBench Magic · {arm}",
                "family": "cardbench-magic",
            },
            "run": {
                "model": model,
                "provider": "openrouter",
                "status": "completed",
                "win_rate": receipt.get("win_rate"),
                "agency_pct": None,
            },
            "integrity": {
                "status": "recorded",
                "content_digest": f"sha256:{sha256_bytes(json.dumps(decisions).encode())}",
                "source": source,
            },
            "steps": steps,
        },
    }


def receipt_win_rate(receipt: dict[str, Any]) -> float | None:
    overall = receipt.get("overall") or {}
    if isinstance(overall, dict) and overall.get("point") is not None:
        return float(overall["point"])
    if receipt.get("win_rate") is not None:
        return float(receipt["win_rate"])
    return None


def receipt_agency(receipt: dict[str, Any]) -> float:
    stats = receipt.get("stats_a") or receipt.get("stats_react") or {}
    open_n = int(stats.get("open") or 0)
    fallbacks = int(stats.get("fallbacks") or 0)
    if open_n <= 0:
        return 100.0
    return round(100.0 * (1.0 - fallbacks / open_n), 1)


def load_constructed_1v1_elo(
    regen: Path,
) -> dict[str, dict[str, dict[str, float | None]]]:
    """provider -> effort -> {elo_1v1, wr_1v1, agency_elo_1v1} from react-vs-code.

    Looks under play/1v1-constructed/{model_slug}-{effort}-pairs{N}/arena-receipt.json
    Prefer pairs3 over pairs1.
    """
    root = regen / "play" / "1v1-constructed"
    out: dict[str, dict[str, dict[str, float | None]]] = {}
    if not root.exists():
        return out

    # slug like gemini-2.5-flash-lite-medium-pairs3
    for receipt_path in sorted(root.glob("*/arena-receipt.json")):
        dirname = receipt_path.parent.name
        m = re.match(
            r"^(?P<slug>.+)-(?P<effort>none|low|medium|high)-pairs(?P<pairs>\d+)$",
            dirname,
        )
        if not m:
            continue
        slug = m.group("slug")
        effort = m.group("effort")
        pairs = int(m.group("pairs"))
        if "gemini-" in slug:
            provider = f"google/{slug}"
        elif slug.startswith("gpt-") or slug.startswith("o1") or slug.startswith("o3"):
            provider = f"openai/{slug}"
        elif "deepseek" in slug:
            provider = f"deepseek/{slug}"
        else:
            provider = slug

        receipt = json.loads(receipt_path.read_text())
        wr = receipt_win_rate(receipt)
        agency = receipt_agency(receipt)
        bucket = out.setdefault(provider, {}).setdefault(effort, {})
        prev_pairs = int(bucket.get("pairs") or 0)
        if prev_pairs > pairs:
            continue
        bucket["pairs"] = float(pairs)
        bucket["agency_elo_1v1"] = agency
        bucket["wr_1v1"] = wr
        # Gate: valid receipt + agency ≥90% (constructed menus can budget-hit).
        if receipt.get("valid") and agency >= 90.0 and wr is not None:
            bucket["elo_1v1"] = wr_to_elo(wr)
        else:
            bucket["elo_1v1"] = None
    return out


def load_format_elo(regen: Path) -> dict[str, dict[str, float | None]]:
    """model -> {elo_2hg, elo_commander, wr_*, agency_*} from mirrored 2HG + FFA mini-Commander."""
    out: dict[str, dict[str, float | None]] = {}
    for model in (
        "gemini-2.5-flash-lite",
        "gemini-3.1-flash-lite",
        "gemini-3.5-flash-lite",
    ):
        provider = f"google/{model}"
        row: dict[str, float | None] = {
            "elo_2hg": None,
            "elo_commander": None,
            "wr_2hg": None,
            "wr_commander": None,
        }

        # Mirrored precon 2HG (React seat0).
        for suffix in ("pairs3", "pairs1"):
            receipt_path = (
                regen / "play" / "2hg-llm-mirror" / f"{model}-{suffix}" / "arena-receipt.json"
            )
            if not receipt_path.exists():
                # Fall back to legacy asymmetric path only if mirror missing.
                receipt_path = (
                    regen / "play" / "2hg-llm" / f"{model}-{suffix}" / "arena-receipt.json"
                )
            if not receipt_path.exists():
                continue
            receipt = json.loads(receipt_path.read_text())
            stats = receipt.get("stats_react") or {}
            open_n = int(stats.get("open") or 0)
            fallbacks = int(stats.get("fallbacks") or 0)
            agency = round(100.0 * (1.0 - fallbacks / open_n), 1) if open_n else 100.0
            row["agency_elo_2hg"] = agency
            if receipt.get("valid") and agency >= 94.0:
                wr = float(receipt.get("win_rate") or 0.0)
                row["elo_2hg"] = wr_to_elo(wr)
                row["wr_2hg"] = wr
                break

        # Mini-Commander 40 FFA (same model ×4). Require seeds3 — omit models
        # that only have seed1 smoke / legacy react-vs-code pairs.
        receipt_path = (
            regen / "play" / "commander-ffa" / f"{model}-seeds3" / "arena-receipt.json"
        )
        if receipt_path.exists():
            receipt = json.loads(receipt_path.read_text())
            seats = receipt.get("seats") or []
            if seats:
                agencies = [float(seat.get("agency") or 0.0) * 100.0 for seat in seats]
                mean_agency = sum(agencies) / len(agencies) if agencies else 0.0
                row["agency_elo_commander"] = round(mean_agency, 1)
                # Mean FFA win rate across seats (expected ~0.25 under parity).
                wrs = [float(seat.get("win_rate") or 0.0) for seat in seats]
                mean_wr = sum(wrs) / len(wrs) if wrs else 0.5
                # Gate: valid + mean agency ≥94%.
                if receipt.get("valid") and mean_agency >= 94.0:
                    row["elo_commander"] = wr_to_elo(mean_wr)
                    row["wr_commander"] = mean_wr
                    row["deck_rules"] = receipt.get("deck_rules") or "mini_commander_40"

        out[provider] = row
    return out


def package_format_replays(regen: Path, out: Path) -> dict[str, Any]:
    featured: dict[str, str] = {}
    # 2HG mirrored LLM
    rollouts_2hg = []
    for model in (
        "gemini-2.5-flash-lite",
        "gemini-3.1-flash-lite",
        "gemini-3.5-flash-lite",
    ):
        for lane in ("2hg-llm-mirror", "2hg-llm"):
            for suffix in ("pairs3", "pairs1"):
                root = regen / "play" / lane / f"{model}-{suffix}"
                receipt_path = root / "arena-receipt.json"
                decisions_path = root / "decisions-seat-0.json"
                if not receipt_path.exists() or not decisions_path.exists():
                    continue
                receipt = json.loads(receipt_path.read_text())
                decisions = json.loads(decisions_path.read_text())
                provider = f"google/{model}"
                trace_id = f"2hg-{model_slug(provider)}-{suffix}"
                replay = decisions_to_replay(
                    trace_id=trace_id,
                    model=provider,
                    arm="2hg",
                    decisions=decisions,
                    receipt=receipt,
                    source=str(root),
                )
                frames_path = root / "frames.json"
                if frames_path.exists():
                    match_frames = json.loads(frames_path.read_text())
                    replay["match_frames"] = match_frames
                    scrub = []
                    for i, frame in enumerate(match_frames):
                        if not isinstance(frame, dict):
                            continue
                        scrub.append(
                            {
                                "index": i,
                                "turn": frame.get("turn"),
                                "note": frame.get("note"),
                                "team_life": frame.get("team_life"),
                                "active_seat": frame.get("active_seat"),
                                "priority_seat": frame.get("priority_seat"),
                                "last_seat": frame.get("last_seat"),
                                "last_action": frame.get("last_action"),
                                "seats_lost": frame.get("seats_lost"),
                            }
                        )
                    if scrub:
                        replay["frames"] = scrub
                        replay["llm"] = True
                        games = receipt.get("games") or []
                        if games:
                            replay["seed"] = games[0].get("seed")
                            replay["terminal_reason"] = games[0].get("termination")
                body = dump(out / "v5" / "2hg" / "replays" / f"{trace_id}.json", replay)
                rollouts_2hg.append(
                    {
                        "id": trace_id,
                        "sha256": sha256_bytes(body),
                        "model": provider,
                        "arm": "2hg",
                        "steps": len(replay["trace"]["steps"]),
                        "win_rate": receipt.get("win_rate"),
                        "valid": receipt.get("valid"),
                        "mirrored": lane == "2hg-llm-mirror",
                    }
                )
                featured.setdefault("2hg", trace_id)
                break
            if featured.get("2hg") == f"2hg-{model_slug(f'google/{model}')}-{suffix}":
                break
            if any(r["model"] == f"google/{model}" for r in rollouts_2hg):
                break
    if rollouts_2hg:
        dump(
            out / "v5" / "2hg" / "matrix.json",
            {
                "schema": "cardbench_magic_2hg_matrix_v1",
                "featured_rollout_id": featured.get("2hg"),
                "rollouts": rollouts_2hg,
                "coverage": {
                    "lane": "2hg",
                    "llm": "available",
                    "mirrored_precons": True,
                    "completed_rollouts": len(rollouts_2hg),
                },
            },
        )

    # Commander FFA mini-40 (seeds3 only — omit incomplete models / legacy pairs).
    rollouts_cmd = []
    cmd_replay_dir = out / "v5" / "commander" / "replays"
    if cmd_replay_dir.exists():
        for stale in cmd_replay_dir.glob("commander-*.json"):
            stale.unlink()
    for model in (
        "gemini-2.5-flash-lite",
        "gemini-3.1-flash-lite",
        "gemini-3.5-flash-lite",
    ):
        root = regen / "play" / "commander-ffa" / f"{model}-seeds3"
        receipt_path = root / "arena-receipt.json"
        decisions_path = root / "decisions-seat-0.json"
        if not receipt_path.exists() or not decisions_path.exists():
            continue
        receipt = json.loads(receipt_path.read_text())
        if not receipt.get("valid"):
            continue
        decisions = json.loads(decisions_path.read_text())
        provider = f"google/{model}"
        trace_id = f"commander-{model_slug(provider)}-seeds3"
        replay = decisions_to_replay(
            trace_id=trace_id,
            model=provider,
            arm="commander",
            decisions=decisions,
            receipt=receipt,
            source=str(root),
        )
        frames_path = root / "frames.json"
        if frames_path.exists():
            replay["match_frames"] = json.loads(frames_path.read_text())
            replay["llm"] = True
        replay["deck_rules"] = receipt.get("deck_rules") or "mini_commander_40"
        if (root / "decklists.json").exists():
            replay["decklists"] = json.loads((root / "decklists.json").read_text())
        body = dump(cmd_replay_dir / f"{trace_id}.json", replay)
        seats = receipt.get("seats") or []
        mean_wr = (
            sum(float(s.get("win_rate") or 0.0) for s in seats) / len(seats) if seats else None
        )
        rollouts_cmd.append(
            {
                "id": trace_id,
                "sha256": sha256_bytes(body),
                "model": provider,
                "arm": "commander",
                "steps": len(replay["trace"]["steps"]),
                "win_rate": mean_wr,
                "valid": receipt.get("valid"),
                "deck_rules": "mini_commander_40",
            }
        )
        featured.setdefault("commander", trace_id)
    if rollouts_cmd:
        dump(
            out / "v5" / "commander" / "matrix.json",
            {
                "schema": "cardbench_magic_commander_matrix_v1",
                "featured_rollout_id": featured.get("commander"),
                "rollouts": rollouts_cmd,
                "coverage": {
                    "lane": "commander",
                    "llm": "available",
                    "deck_rules": "mini_commander_40",
                    "ffa_seats": 4,
                    "completed_rollouts": len(rollouts_cmd),
                },
            },
        )
        meta_path = out / "v5" / "commander" / "meta.json"
        meta = {
            "schema": "cardbench_magic_commander_meta_v1",
            "status": "llm_available",
            "llm_trace_v5": "available",
            "protocol": "Absent",
            "deck_rules": "mini_commander_40",
            "note": "CardBench mini-Commander: 40-card RAV decks (not official 100). Four LLM seats FFA; Elo from mean seat win rate.",
            "command_tax": {
                "base": 0,
                "increment_per_cast": 2,
                "note": "Printed cost + 2 generic per prior command-zone cast.",
            },
            "starters": [],
        }
        if meta_path.exists():
            prev = json.loads(meta_path.read_text())
            meta["starters"] = prev.get("starters") or []
        dump(meta_path, meta)
    return featured


def package_tournaments(regen: Path, out: Path) -> dict[str, Any]:
    """Return provider -> effort -> {seed -> {seat -> elo}}."""
    tournament_dir = out / "v5" / "tournament"
    by_model_effort: dict[str, dict[str, dict[int, dict[int, float]]]] = {}
    specs = [
        ("gemini-2.5-flash-lite", "google", "low", ""),
        ("gemini-3.1-flash-lite", "google", "low", ""),
        ("gemini-3.5-flash-lite", "google", "low", ""),
        ("gpt-5.4-nano", "openai", "low", ""),
        ("gpt-5.6-luna", "openai", "low", ""),
        ("gpt-5.4-nano", "openai", "medium", "-medium"),
        ("gpt-5.6-luna", "openai", "medium", "-medium"),
        ("gpt-5.4-nano", "openai", "high", "-high"),
        ("gpt-5.6-luna", "openai", "high", "-high"),
    ]
    for model, vendor, effort, infix in specs:
        for seed in (73, 74, 75):
            summary_path = (
                regen / f"{model}{infix}-draft-{seed}" / "eval" / "tournament" / "summary.json"
            )
            if not summary_path.exists():
                continue
            summary = json.loads(summary_path.read_text())
            provider = f"{vendor}/{model}"
            body = dump(
                tournament_dir / f"{model}{infix}-seed-{seed}.json",
                summary,
            )
            seat_map = {
                int(row["seat"]): float(row["elo"]) for row in summary.get("seats") or []
            }
            by_model_effort.setdefault(provider, {}).setdefault(effort, {})[seed] = seat_map
            _ = body
    return by_model_effort


def patch_draft_matrix(
    out: Path,
    format_elo: dict[str, dict[str, float | None]],
    tournaments: dict[str, Any],
    constructed: dict[str, dict[str, dict[str, float | None]]] | None = None,
) -> None:
    path = out / "v5" / "matrix.json"
    if not path.exists():
        raise SystemExit(f"missing draft matrix at {path}")
    constructed = constructed or {}
    matrix = json.loads(path.read_text())
    for row in matrix.get("rollouts") or []:
        model = row.get("model")
        seed = int(row.get("seed") or 0)
        seat = int(row.get("seat") or 0)
        effort = str(row.get("effort") or "low")
        fmt = format_elo.get(model) or {}
        if fmt.get("elo_2hg") is not None:
            row["elo_2hg"] = fmt["elo_2hg"]
            row.setdefault("metric_status", {})["elo_2hg"] = "available"
            row.setdefault("metric_sources", {})["elo_2hg"] = [
                "llm_2hg_mirrored_precons",
                "wr_to_elo_vs_1500",
            ]
        else:
            row.pop("elo_2hg", None)
            if isinstance(row.get("metric_status"), dict):
                row["metric_status"]["elo_2hg"] = "pending"
        if fmt.get("elo_commander") is not None:
            row["elo_commander"] = fmt["elo_commander"]
            row.setdefault("metric_status", {})["elo_commander"] = "available"
            row.setdefault("metric_sources", {})["elo_commander"] = [
                "llm_mini_commander_40_ffa",
                "mean_seat_wr_to_elo",
            ]
        else:
            row.pop("elo_commander", None)
            if isinstance(row.get("metric_status"), dict):
                row["metric_status"]["elo_commander"] = "pending"
            if isinstance(row.get("metric_sources"), dict):
                row["metric_sources"].pop("elo_commander", None)

        # Constructed 1v1: prefer matching effort, else fall back to low.
        c1 = (constructed.get(model) or {}).get(effort) or (
            constructed.get(model) or {}
        ).get("low")
        if c1 and c1.get("elo_1v1") is not None:
            row["elo_1v1"] = c1["elo_1v1"]
            row.setdefault("metric_status", {})["elo_1v1"] = "available"
            row.setdefault("metric_sources", {})["elo_1v1"] = [
                "constructed_react_vs_code_v7",
                "wr_to_elo_vs_1500",
                f"pairs{int(c1.get('pairs') or 3)}",
            ]
        else:
            row.pop("elo_1v1", None)
            if isinstance(row.get("metric_status"), dict):
                row["metric_status"]["elo_1v1"] = "pending"
            if isinstance(row.get("metric_sources"), dict):
                row["metric_sources"]["elo_1v1"] = []

        seat_elos = (
            ((tournaments.get(model) or {}).get(row.get("effort") or "low") or {}).get(seed)
            or (tournaments.get(model) or {}).get(seed)
            or {}
        )
        if seat in seat_elos:
            row["full_draft_tournament_elo"] = seat_elos[seat]
            row.setdefault("metric_status", {})["full_draft_tournament_elo"] = "available"
            row.setdefault("metric_sources", {})["full_draft_tournament_elo"] = [
                "draft_pod_round_robin_catalog",
                "sequential_elo_k32",
            ]
        # Composite: drafting + limited + constructed 1v1 + 2hg + commander
        parts: list[float] = []
        if row.get("drafting_efficacy_elo") is not None:
            parts.append(float(row["drafting_efficacy_elo"]))
        if row.get("full_draft_tournament_elo") is not None:
            parts.append(float(row["full_draft_tournament_elo"]))
        elif row.get("full_draft_elo") is not None:
            parts.append(float(row["full_draft_elo"]))
        for key in ("elo_1v1", "elo_2hg", "elo_commander"):
            if row.get(key) is not None:
                parts.append(float(row[key]))
        if parts:
            row["composite"] = round(sum(parts) / len(parts), 1)

    coverage = dict(matrix.get("coverage") or {})
    y = dict(coverage.get("y_metrics") or {})
    if any(
        (eff.get("elo_1v1") is not None)
        for model_map in constructed.values()
        for eff in model_map.values()
    ):
        y["elo_1v1"] = "available"
    else:
        y["elo_1v1"] = "pending"
    if any(v.get("elo_2hg") is not None for v in format_elo.values()):
        y["elo_2hg"] = "available"
    if any(v.get("elo_commander") is not None for v in format_elo.values()):
        y["elo_commander"] = "available"
    y["full_draft_tournament_elo"] = "available" if tournaments else y.get(
        "full_draft_tournament_elo", "pending"
    )
    coverage["y_metrics"] = y
    coverage["play_frames"] = coverage.get("play_frames") or "menu_decisions_available"
    coverage["two_headed_giant"] = "llm_available"
    coverage["commander"] = "llm_available"
    coverage["draft_tournament"] = "available"
    coverage["constructed_1v1"] = (
        "available" if y.get("elo_1v1") == "available" else "pending"
    )
    matrix["coverage"] = coverage
    dump(path, matrix)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--regen", type=Path, default=DEFAULT_REGEN)
    parser.add_argument("--out", type=Path, default=DEFAULT_OUT)
    args = parser.parse_args()
    out: Path = args.out
    regen: Path = args.regen

    featured = package_format_replays(regen, out)
    tournaments = package_tournaments(regen, out)
    format_elo = load_format_elo(regen)
    constructed = load_constructed_1v1_elo(regen)
    patch_draft_matrix(out, format_elo, tournaments, constructed)

    dump(
        out / "meta" / "format_llm_sources.json",
        {
            "format_elo": format_elo,
            "constructed_1v1": constructed,
            "featured": featured,
            "tournament_models": list(tournaments.keys()),
        },
    )
    print(f"wrote {out}")
    print("format_elo", json.dumps(format_elo, indent=2))
    print("constructed_1v1", json.dumps(constructed, indent=2))
    print("featured", featured)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
