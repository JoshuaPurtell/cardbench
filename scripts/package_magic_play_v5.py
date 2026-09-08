#!/usr/bin/env python3
"""Package Magic play / 2HG / commander format assets into cardbench-magic.

Writes (without clobbering draft rollouts):
  v5/play/matrix.json
  v5/play/replays/<id>.json
  v5/2hg/matrix.json
  v5/2hg/replays/<id>.json
  v5/commander/meta.json

Also patches the draft v5/matrix.json coverage flags so the page can advertise
play Trace V5 without inventing Play Elo columns.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import re
from pathlib import Path
from typing import Any

CHOICE_RE = re.compile(r'\{\s*"choice"\s*:\s*(\d+).*?\}', re.S)
REASON_RE = re.compile(r'"reasoning"\s*:\s*"((?:\\.|[^"\\])*)"')

DEFAULT_PLAY = Path(
    "/Users/joshuapurtell/Documents/Codex/2026-08-04/imp/work/magic-multiplayer"
    "/varieties/magic/artifacts/regen-2026-08-05/play/3.5-vs-v7-recorded"
)
DEFAULT_2HG = Path(
    "/Users/joshuapurtell/Documents/Codex/2026-08-04/imp/work/magic-multiplayer"
    "/varieties/magic/artifacts/regen-2026-08-05/play/2hg-code-seed-73"
)
DEFAULT_OUT = (
    Path(__file__).resolve().parents[1] / "artifacts" / "public-eval" / "cardbench-magic"
)


def sha256_bytes(body: bytes) -> str:
    return hashlib.sha256(body).hexdigest()


def sha256_file(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def dump(path: Path, payload: Any) -> bytes:
    body = json.dumps(payload, indent=2, sort_keys=True).encode("utf-8") + b"\n"
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(body)
    return body


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
        .replace("deepseek/", "")
        .replace(".", "-")
        .replace("_", "-")
    )


def package_play(play_root: Path, out: Path) -> dict[str, Any]:
    receipt = json.loads((play_root / "arena-receipt.json").read_text())
    decisions = json.loads((play_root / "decisions-seat-a.json").read_text())
    seat_a = str(receipt.get("seat_a") or "")
    model = "google/gemini-3.5-flash-lite"
    if "google/" in seat_a:
        # react:google/...(...) → google/...
        start = seat_a.find("google/")
        end = seat_a.find("(", start)
        model = seat_a[start : end if end > start else None].strip()
    slug = model_slug(model)
    pairs = int(receipt.get("seed_pairs") or 1)
    trace_id = f"play-{slug}-vs-v7-pairs-{pairs}"
    source_digest = sha256_file(play_root / "decisions-seat-a.json")

    steps: list[dict[str, Any]] = []
    frames: list[dict[str, Any]] = []
    actions: list[dict[str, Any]] = []
    open_index = 0
    for record in decisions:
        options = list(record.get("options") or [])
        # Skip pure fallback markers with empty menus — they are not scrub frames.
        if not options and not (record.get("reply") or "").strip():
            continue
        open_index += 1
        reply = str(record.get("reply") or "")
        parsed_choice, reasoning = parse_reply(reply)
        chosen = record.get("chosen")
        if chosen is None:
            chosen = parsed_choice
        note = record.get("note")
        outcome = "accepted" if chosen is not None and not note else "fallback"
        if note and chosen is None:
            outcome = "fallback"
        elif note:
            outcome = "rejected"
        own_life = record.get("own_life")
        opp = record.get("opponent_life") or []
        opp_life = opp[0][1] if opp and isinstance(opp[0], (list, tuple)) else None
        turn = int(record.get("turn") or 0)
        step = str(record.get("step") or "")
        chosen_label = (
            options[chosen] if isinstance(chosen, int) and 0 <= chosen < len(options) else None
        )
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
                "id": f"decision-{open_index}",
                "index": open_index,
                "kind": "policy_call",
                "title": title,
                "timestamp": None,
                "turn_start": turn,
                "turn_end": turn,
                "tokens": {},
                "content": {
                    "observation": observation,
                    "reasoning": reasoning or None,
                    "message": reply.strip() or None,
                },
                "action": {
                    "planned": [f"choice:{chosen}"] if chosen is not None else [],
                    "executed": (
                        [{"turn": turn, "name": chosen_label}]
                        if chosen_label and outcome == "accepted"
                        else []
                    ),
                    "rejected": (
                        [
                            {
                                "turn": turn,
                                "name": chosen_label or f"choice:{chosen}",
                                "reason": note or outcome,
                            }
                        ]
                        if outcome != "accepted"
                        else []
                    ),
                },
                "reward": 1.0 if outcome == "accepted" else 0.0,
                "achievements": [],
                "state_delta": {},
                "raw": {
                    "turn": turn,
                    "step": step,
                    "own_life": own_life,
                    "opponent_life": opp_life,
                    "choice": chosen,
                    "note": note,
                    "outcome": outcome,
                },
            }
        )
        frames.append(
            {
                "index": open_index,
                "turn": turn,
                "step": step,
                "own_life": own_life,
                "opponent_life": opp_life,
                "options": options,
                "chosen_index": chosen,
                "chosen_label": chosen_label,
                "outcome": outcome,
                "note": note,
                "reasoning": reasoning or None,
            }
        )
        if chosen_label:
            actions.append(
                {
                    "turn": turn,
                    "action": chosen_label,
                    "outcome": outcome,
                }
            )

    stats = receipt.get("stats_a") or {}
    open_n = int(stats.get("open") or 0)
    fallbacks = int(stats.get("fallbacks") or 0)
    agency = round(100.0 * (1.0 - (fallbacks / open_n)), 1) if open_n else 0.0
    overall = receipt.get("overall") or {}
    win_rate = float(overall.get("point") or 0.0)

    replay = {
        "schema": "cardbench_magic_play_replay_v1",
        "id": trace_id,
        "model": model,
        "effort": "low",
        "arm": "play-1v1",
        "agent_count": 1,
        "scenario": "ravnica_constructed_react_vs_code",
        "seed_pairs": pairs,
        "opponent": str(receipt.get("seat_b") or "v7"),
        "success": bool(receipt.get("valid")) and agency >= 95.0,
        "terminal_reason": "arena_complete",
        "score": round(win_rate * 100.0, 1),
        "total_reward": win_rate,
        "actions": actions,
        "frames": frames,
        "environment": "cardbench-magic-play",
        "protocol": "react-vs-code-recorded",
        "coverage_label": "llm-menu-decisions",
        "receipt": {
            "valid": receipt.get("valid"),
            "overall": overall,
            "stats_a": stats,
            "per_deck": receipt.get("per_deck"),
        },
        "trace": {
            "schema": "eval_trace_view_v1",
            "source_schema": "cardbench.magic.arena-decision-record.v1",
            "trace_id": trace_id,
            "task": {
                "id": "cardbench-magic-play-1v1",
                "name": "CardBench Magic · Constructed 1v1",
                "family": "cardbench-magic",
            },
            "run": {
                "model": model,
                "provider": "openrouter",
                "effort": "low",
                "seed_pairs": pairs,
                "status": "completed",
                "opponent": receipt.get("seat_b"),
                "win_rate": win_rate,
                "agency_pct": agency,
            },
            "integrity": {
                "status": "recorded",
                "content_digest": f"sha256:{source_digest}",
                "source": play_root.name,
            },
            "system_prompt": None,
            "steps": steps,
        },
    }
    body = dump(out / "v5" / "play" / "replays" / f"{trace_id}.json", replay)
    digest = sha256_bytes(body)
    rollout = {
        "id": trace_id,
        "sha256": digest,
        "model": model,
        "effort": "low",
        "arm": "play-1v1",
        "agent_count": 1,
        "scenario": "ravnica_constructed_react_vs_code",
        "seed_pairs": pairs,
        "opponent": receipt.get("seat_b"),
        "success": replay["success"],
        "terminal_reason": "arena_complete",
        "score": replay["score"],
        "total_reward": replay["total_reward"],
        "steps": len(steps),
        "open_decisions": open_n,
        "agency_pct": agency,
        "win_rate": win_rate,
        "environment": "cardbench-magic-play",
        "protocol": "react-vs-code-recorded",
        "coverage_label": "llm-menu-decisions",
    }
    matrix = {
        "schema": "cardbench_magic_play_matrix_v1",
        "benchmark_id": "cardbench/magic",
        "environment": "cardbench-magic-play",
        "status": "pilot",
        "coverage": {
            "lane": "play-1v1",
            "completed_rollouts": 1,
            "board_frames": "pending",
            "menu_decisions": "available",
            "source_artifact": str(play_root),
        },
        "featured_rollout_id": trace_id,
        "rollouts": [rollout],
        "source_runs": [
            {
                "id": play_root.name,
                "digest": f"sha256:{source_digest}",
                "receipt_digest": f"sha256:{sha256_file(play_root / 'arena-receipt.json')}",
            }
        ],
    }
    dump(out / "v5" / "play" / "matrix.json", matrix)
    return {"trace_id": trace_id, "steps": len(steps), "agency": agency, "win_rate": win_rate}


def package_2hg(match_root: Path, out: Path) -> dict[str, Any]:
    match_path = match_root / "2hg-match.json"
    match = json.loads(match_path.read_text())
    seed = int(match.get("seed") or 73)
    trace_id = f"2hg-code-policy-seed-{seed}"
    source_digest = sha256_file(match_path)
    frames_in = list(match.get("frames") or [])
    seats = list(match.get("seats") or [])
    steps: list[dict[str, Any]] = []
    frames: list[dict[str, Any]] = []
    for index, frame in enumerate(frames_in, start=1):
        turn = int(frame.get("turn") or 0)
        note = str(frame.get("note") or "")
        team_life = frame.get("team_life") or [0, 0]
        title = f"T{turn} · {note}"
        if frame.get("last_action"):
            title = f"{title} · seat {frame.get('last_seat')} {frame.get('last_action')}"
        observation = "\n".join(
            [
                f"Turn {turn}",
                f"Team 1 life {team_life[0]} · Team 2 life {team_life[1]}",
                f"Active seat {frame.get('active_seat')} · priority seat {frame.get('priority_seat')}",
                f"Note: {note}",
                f"Last: seat {frame.get('last_seat')} → {frame.get('last_action')}",
            ]
        )
        steps.append(
            {
                "id": f"frame-{index}",
                "index": index,
                "kind": "checkpoint",
                "title": title,
                "timestamp": None,
                "turn_start": turn,
                "turn_end": turn,
                "tokens": {},
                "content": {
                    "observation": observation,
                    "reasoning": None,
                    "message": None,
                },
                "action": {
                    "planned": [],
                    "executed": (
                        [
                            {
                                "turn": turn,
                                "name": f"seat{frame.get('last_seat')}:{frame.get('last_action')}",
                            }
                        ]
                        if frame.get("last_action")
                        else []
                    ),
                    "rejected": [],
                },
                "reward": None,
                "achievements": [],
                "state_delta": {},
                "raw": frame,
            }
        )
        frames.append(
            {
                "index": index,
                "turn": turn,
                "note": note,
                "team_life": team_life,
                "active_seat": frame.get("active_seat"),
                "priority_seat": frame.get("priority_seat"),
                "last_seat": frame.get("last_seat"),
                "last_action": frame.get("last_action"),
                "seats_lost": frame.get("seats_lost"),
                "seats": seats,
            }
        )

    replay = {
        "schema": "cardbench_magic_2hg_replay_v1",
        "id": trace_id,
        "model": None,
        "policy": "code-policy",
        "llm": False,
        "arm": "2hg",
        "agent_count": 4,
        "scenario": "ravnica_two_headed_giant_code",
        "seed": seed,
        "success": str(match.get("termination") or "").startswith("winner="),
        "terminal_reason": match.get("termination"),
        "score": None,
        "total_reward": None,
        "actions": [],
        "frames": frames,
        "environment": "cardbench-magic-2hg",
        "protocol": "code-policy-2hg",
        "coverage_label": "code-policy",
        "match": {
            "team_life": match.get("team_life"),
            "turns": match.get("turns"),
            "accepted_policy_moves": match.get("accepted_policy_moves"),
            "event_digest": match.get("event_digest"),
            "seats": seats,
        },
        "trace": {
            "schema": "eval_trace_view_v1",
            "source_schema": "cardbench.magic.two-headed-giant-policy-match.v1",
            "trace_id": trace_id,
            "task": {
                "id": "cardbench-magic-2hg",
                "name": "CardBench Magic · Two-Headed Giant",
                "family": "cardbench-magic",
            },
            "run": {
                "model": None,
                "provider": "code-policy",
                "effort": None,
                "seed": seed,
                "status": "completed",
                "llm": False,
                "termination": match.get("termination"),
            },
            "integrity": {
                "status": "recorded",
                "content_digest": f"sha256:{source_digest}",
                "source": match_root.name,
            },
            "system_prompt": None,
            "steps": steps,
        },
    }
    body = dump(out / "v5" / "2hg" / "replays" / f"{trace_id}.json", replay)
    digest = sha256_bytes(body)
    matrix = {
        "schema": "cardbench_magic_2hg_matrix_v1",
        "benchmark_id": "cardbench/magic",
        "environment": "cardbench-magic-2hg",
        "status": "pilot",
        "coverage": {
            "lane": "2hg",
            "policy": "code-policy",
            "llm": "pending",
            "completed_rollouts": 1,
            "source_artifact": str(match_root),
        },
        "featured_rollout_id": trace_id,
        "rollouts": [
            {
                "id": trace_id,
                "sha256": digest,
                "model": None,
                "policy": "code-policy",
                "llm": False,
                "arm": "2hg",
                "seed": seed,
                "success": replay["success"],
                "terminal_reason": match.get("termination"),
                "steps": len(steps),
                "environment": "cardbench-magic-2hg",
                "protocol": "code-policy-2hg",
                "coverage_label": "code-policy",
            }
        ],
        "source_runs": [{"id": match_root.name, "digest": f"sha256:{source_digest}"}],
    }
    dump(out / "v5" / "2hg" / "matrix.json", matrix)
    return {"trace_id": trace_id, "frames": len(frames), "termination": match.get("termination")}


def package_commander(out: Path) -> dict[str, Any]:
    meta = {
        "schema": "cardbench_magic_commander_meta_v1",
        "status": "harness_pending",
        "llm_trace_v5": "pending",
        "protocol": "Absent",
        "note": (
            "Validated Ravnica starter commanders only. No LLM Trace V5 campaign bin exists yet; "
            "do not invent Commander Elo."
        ),
        "starters": [
            {
                "id": "RAV-TOLSIMIR-WOLFBLOOD",
                "name": "Tolsimir Wolfblood",
                "identity": ["Green", "White"],
                "identity_pips": ["G", "W"],
                "mana": "4GW",
                "power": 3,
                "toughness": 4,
                "role": "human_default",
                "validated": True,
            },
            {
                "id": "RAV-SZADEK",
                "name": "Szadek, Lord of Secrets",
                "identity": ["Blue", "Black"],
                "identity_pips": ["U", "B"],
                "mana": "3UUBB",
                "power": 5,
                "toughness": 5,
                "role": "ai_default",
                "validated": True,
            },
        ],
        "command_tax": {
            "base": 0,
            "increment_per_cast": 2,
            "note": "Tax shown for UI shell only; no recorded cast sequence in this cut.",
        },
    }
    dump(out / "v5" / "commander" / "meta.json", meta)
    return {"starters": len(meta["starters"])}


def patch_draft_matrix(out: Path, play: dict[str, Any], twog: dict[str, Any]) -> None:
    path = out / "v5" / "matrix.json"
    if not path.exists():
        return
    matrix = json.loads(path.read_text())
    coverage = dict(matrix.get("coverage") or {})
    coverage["play_frames"] = "menu_decisions_available"
    coverage["play_featured_rollout_id"] = play["trace_id"]
    coverage["two_headed_giant"] = "code_policy_available"
    coverage["two_headed_giant_featured_rollout_id"] = twog["trace_id"]
    coverage["commander"] = "meta_only"
    y_metrics = dict(coverage.get("y_metrics") or {})
    y_metrics.setdefault("elo_2hg", "pending")
    y_metrics.setdefault("elo_commander", "pending")
    coverage["y_metrics"] = y_metrics
    matrix["coverage"] = coverage
    excluded = list(matrix.get("excluded_lanes") or [])
    # Keep Elo lanes excluded; advertise format assets separately.
    for lane in ("play-h2h", "play-2hg-llm", "play-commander-llm"):
        if lane not in excluded:
            excluded.append(lane)
    matrix["excluded_lanes"] = excluded
    dump(path, matrix)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--play-artifact", type=Path, default=DEFAULT_PLAY)
    parser.add_argument("--twohg-artifact", type=Path, default=DEFAULT_2HG)
    parser.add_argument("--out", type=Path, default=DEFAULT_OUT)
    args = parser.parse_args()
    out: Path = args.out
    out.mkdir(parents=True, exist_ok=True)

    play = package_play(args.play_artifact, out)
    twog = package_2hg(args.twohg_artifact, out)
    commander = package_commander(out)
    patch_draft_matrix(out, play, twog)

    dump(
        out / "meta" / "format_sources.json",
        {
            "play": {
                "artifact": str(args.play_artifact),
                "trace_id": play["trace_id"],
                "steps": play["steps"],
                "agency_pct": play["agency"],
                "win_rate": play["win_rate"],
            },
            "two_headed_giant": {
                "artifact": str(args.twohg_artifact),
                "trace_id": twog["trace_id"],
                "frames": twog["frames"],
                "termination": twog["termination"],
            },
            "commander": commander,
        },
    )
    print(f"wrote {out}")
    print(
        f"play={play['trace_id']} steps={play['steps']} "
        f"2hg={twog['trace_id']} frames={twog['frames']} "
        f"commander_starters={commander['starters']}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
