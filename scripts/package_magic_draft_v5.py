#!/usr/bin/env python3
"""Package Magic Draft seed-73 into a Wasabi-ready cardbench-magic v5 tree.

Reads the on-disk rav-llm-draft-eval artifact and emits:
  v5/matrix.json
  v5/replays/<trace_id>.json
  meta/source.json

No Scryfall art / collector frames — pack state is structured for chip renderers.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import re
from pathlib import Path
from typing import Any

PACK_LINE = re.compile(
    r"^(\d+)\.\s+(.+?)\s+\((RAV-[A-Z0-9-]+),\s*ManaCost\s*\{(.+)\}\)\s*$"
)
CHOICE_RE = re.compile(r'\{\s*"choice"\s*:\s*(\d+).*?\}', re.S)
REASON_RE = re.compile(r'"reasoning"\s*:\s*"((?:\\.|[^"\\])*)"')
PACK_BLOCK = re.compile(
    r"=== CURRENT PACK ===\n(.*?)(?:\n=== |\nYour pool|\Z)", re.S
)
POOL_BLOCK = re.compile(r"=== YOUR POOL ===\n(.*?)(?:\n=== |\Z)", re.S)
PICK_HEADER = re.compile(
    r"Pack\s+(\d+)\s+pick\s+(\d+)\.\s+Your pool has\s+(\d+)\s+cards?", re.I
)
COLOR_PIP = {
    "White": "W",
    "Blue": "U",
    "Black": "B",
    "Red": "R",
    "Green": "G",
}
DEFAULT_RAV_DEFS = Path(
    "/Users/joshuapurtell/Documents/Codex/2026-08-04/imp/work/magic-multiplayer"
    "/varieties/magic/sets/ravnica_city_of_guilds/src/lib.rs"
)


def sha256_bytes(body: bytes) -> str:
    return hashlib.sha256(body).hexdigest()


def sha256_file(path: Path) -> str:
    return sha256_bytes(path.read_bytes())


def load_card_catalog(path: Path) -> dict[str, dict[str, Any]]:
    """Pull type / P/T / mana-value facts from executable CardDefinition blocks (no art)."""
    if not path.exists():
        return {}
    text = path.read_text(encoding="utf-8")
    catalog: dict[str, dict[str, Any]] = {}
    for chunk in re.split(r"CardDefinition\s*\{", text)[1:]:
        window = chunk[:2200]
        id_match = re.search(r'id:\s*"([^"]+)"', window)
        if not id_match:
            continue
        types = re.findall(r"CardType::(\w+)", window)
        power_match = re.search(r"power:\s*Some\((-?\d+)\)", window)
        tough_match = re.search(r"toughness:\s*Some\((-?\d+)\)", window)
        mana_value: int | None = None
        mana_match = re.search(
            r"mana_cost:\s*ManaCost::(\w+)\((.*?)\)\s*,",
            window,
            re.S,
        )
        if mana_match:
            kind, args = mana_match.group(1), mana_match.group(2)
            generic_match = re.match(r"\s*(\d+)", args)
            generic = int(generic_match.group(1)) if generic_match else 0
            colored = re.findall(r"Color::(\w+)", args)
            hybrids = re.findall(
                r"HybridManaSymbol\s*\{\s*first:\s*(\w+),\s*second:\s*(\w+)\s*\}",
                args,
            )
            if kind == "new":
                mana_value = generic
            else:
                # Each colored pip and each hybrid symbol counts as 1 toward MV.
                mana_value = generic + len(colored) + len(hybrids)
        catalog[id_match.group(1)] = {
            "types": types,
            "power": int(power_match.group(1)) if power_match else None,
            "toughness": int(tough_match.group(1)) if tough_match else None,
            "mana_value": mana_value,
        }
    return catalog


BASIC_LAND_IDS = {
    "RAV-PLAINS",
    "RAV-ISLAND",
    "RAV-SWAMP",
    "RAV-MOUNTAIN",
    "RAV-FOREST",
}

# OpenRouter list prices used for published_token_rate_estimate (USD / token).
OPENROUTER_RATES_USD_PER_TOKEN = {
    "google/gemini-2.5-flash-lite": {"input": 0.10 / 1_000_000, "output": 0.40 / 1_000_000},
    "google/gemini-3.1-flash-lite": {"input": 0.10 / 1_000_000, "output": 0.40 / 1_000_000},
    "google/gemini-3.5-flash-lite": {"input": 0.30 / 1_000_000, "output": 2.50 / 1_000_000},
    "openai/gpt-5.4-nano": {"input": 0.20 / 1_000_000, "output": 1.25 / 1_000_000},
    "openai/gpt-5.6-luna": {"input": 0.10 / 1_000_000, "output": 0.60 / 1_000_000},
}
# OpenRouter median TTFT-ish latency proxy × calls.
OPENROUTER_MEDIAN_LATENCY_S = {
    "google/gemini-2.5-flash-lite": 0.50,
    "google/gemini-3.1-flash-lite": 0.50,
    "google/gemini-3.5-flash-lite": 0.50,
    "openai/gpt-5.4-nano": 0.60,
    "openai/gpt-5.6-luna": 0.60,
}


def estimate_seat_economics(
    *,
    provider: str,
    events: list[dict[str, Any]],
    score_path: Path | None,
) -> dict[str, Any]:
    """Token-rate cost estimate + provider-latency proxy from recorded transcript text."""
    rates = OPENROUTER_RATES_USD_PER_TOKEN.get(
        provider,
        {"input": 0.10 / 1_000_000, "output": 0.40 / 1_000_000},
    )
    latency_s = OPENROUTER_MEDIAN_LATENCY_S.get(provider, 0.50)
    input_chars = 0
    output_chars = 0
    for event in events:
        input_chars += len(event.get("prompt") or "")
        output_chars += len(event.get("reply") or "")
    # Rough tokenizer stand-in when provider usage is not recorded on disk.
    input_tokens = input_chars / 4.0
    output_tokens = output_chars / 4.0
    model_calls = len(events)
    cost_usd = input_tokens * rates["input"] + output_tokens * rates["output"]
    est_latency_s = model_calls * latency_s
    mean_turns = None
    if score_path and score_path.exists():
        score = json.loads(score_path.read_text())
        turns = [int(match.get("turns") or 0) for match in score.get("matches") or []]
        if turns:
            mean_turns = sum(turns) / len(turns)
    return {
        "model_calls": model_calls,
        "input_tokens_est": round(input_tokens),
        "output_tokens_est": round(output_tokens),
        "cost_usd": round(cost_usd, 6),
        "cost_sources": ["published_token_rate_estimate", "char/4_token_proxy"],
        "est_latency_s": round(est_latency_s, 3),
        "latency_sources": ["openrouter_median_latency_x_calls"],
        "mean_limited_turns": mean_turns,
    }


def match_won(match: dict[str, Any]) -> bool | None:
    """Return True/False/None for candidate win from a limited-deck-score match row."""
    termination = str(match.get("termination") or "")
    if "Winner(PlayerId(" not in termination:
        return None
    try:
        winner = int(termination.split("Winner(PlayerId(")[1].split(")")[0])
    except (IndexError, ValueError):
        return None
    candidate = int(match.get("candidate_seat") if match.get("candidate_seat") is not None else 0)
    return winner == candidate


def elo_from_limited_matches(
    matches: list[dict[str, Any]],
    *,
    initial: float = 1500.0,
    opponent: float = 1500.0,
    k: float = 32.0,
) -> float | None:
    """Sequential Elo vs fixed sealed-reference opponents (assumed 1500)."""
    if not matches:
        return None
    rating = initial
    decided = 0
    for match in matches:
        won = match_won(match)
        if won is None:
            continue
        decided += 1
        expected = 1.0 / (1.0 + 10.0 ** ((opponent - rating) / 400.0))
        rating += k * ((1.0 if won else 0.0) - expected)
    if decided == 0:
        return None
    return round(rating, 1)


def drafting_efficacy_as_elo(points: float, *, max_points: float = 18.0) -> float:
    """Map Limited points / 18 onto a 1000–2000 Elo-like scale for composites."""
    frac = max(0.0, min(1.0, points / max_points))
    return round(1000.0 + frac * 1000.0, 1)


def seat_performance_metrics(
    *,
    points: int,
    score_path: Path | None,
) -> dict[str, Any]:
    """
    Publish Y-axis metrics for the results atlas.

    Limited matches ground drafting efficacy + full-draft Elo. Constructed 1v1
    Elo is filled later by package_magic_format_llm_v5 from react-vs-code
    receipts (not the Limited sealed-reference series).
    """
    matches: list[dict[str, Any]] = []
    if score_path and score_path.exists():
        score = json.loads(score_path.read_text())
        matches = list(score.get("matches") or [])
    limited_elo = elo_from_limited_matches(matches)
    drafting = float(points)
    sources = {
        "drafting_efficacy": ["limited_points_per_18"],
        "full_draft_elo": ["limited_1v1_vs_sealed_reference", "draft_deckbuild_play"],
        "elo_1v1": [],
        "elo_2hg": [],
        "elo_commander": [],
    }
    return {
        "drafting_efficacy": drafting,
        "full_draft_elo": limited_elo,
        "elo_1v1": None,
        "elo_2hg": None,
        "elo_commander": None,
        "drafting_efficacy_elo": drafting_efficacy_as_elo(drafting),
        "metric_sources": sources,
        "metric_status": {
            "drafting_efficacy": "available",
            "full_draft_elo": "available" if limited_elo is not None else "pending",
            "elo_1v1": "pending",
            "elo_2hg": "pending",
            "elo_commander": "pending",
        },
        "limited_matches": len(matches),
    }


def is_land_card(card_id: str, facts: dict[str, Any] | None) -> bool:
    if card_id in BASIC_LAND_IDS:
        return True
    if facts and "Land" in (facts.get("types") or []):
        return True
    name = card_id
    return any(
        token in name
        for token in (
            "AQUEDUCT",
            "FOUNTAIN",
            "FOUNDRY",
            "GUILDGATE",
            "TOMB",
            "PROMENADE",
            "BASTION",
            "BOILERWORKS",
            "SANCTUARY",
        )
    )


def mana_curve_from_decklist(
    decklist: dict[str, Any],
    catalog: dict[str, dict[str, Any]],
) -> dict[str, Any]:
    """Non-land mana-value histogram for a registered 40-card Limited deck.

    Buckets are 1..6 and 7+ (keyed 7). Lands are excluded from the histogram.
    """
    buckets = {mv: 0 for mv in range(1, 8)}
    lands = 0
    spells = 0
    unknown = 0
    for entry in decklist.get("mainboard") or []:
        card_id = str(entry.get("card") or "")
        count = int(entry.get("count") or 0)
        if count <= 0 or not card_id:
            continue
        facts = catalog.get(card_id)
        if is_land_card(card_id, facts):
            lands += count
            continue
        mv = facts.get("mana_value") if facts else None
        if mv is None:
            unknown += count
            continue
        spells += count
        bucket = 7 if mv >= 7 else max(1, int(mv))
        buckets[bucket] += count
    return {
        "buckets": [{"mv": mv, "count": buckets[mv]} for mv in range(1, 8)],
        "lands": lands,
        "spells": spells,
        "unknown": unknown,
        "total": lands + spells + unknown,
    }


def parse_color_list(raw: str) -> list[str]:
    return [part.strip() for part in raw.split(",") if part.strip()]


def parse_mana_cost_body(body: str) -> tuple[int, list[str], list[str], list[str]]:
    """Return generic, colored identity, mana pip tokens, hybrid labels."""
    generic_match = re.search(r"generic:\s*(\d+)", body)
    generic = int(generic_match.group(1)) if generic_match else 0
    colored_match = re.search(r"colored:\s*\[([^\]]*)\]", body)
    colors = parse_color_list(colored_match.group(1) if colored_match else "")
    hybrids: list[str] = []
    for first, second in re.findall(
        r"HybridManaSymbol\s*\{\s*first:\s*(\w+),\s*second:\s*(\w+)\s*\}",
        body,
    ):
        hybrids.append(f"{COLOR_PIP.get(first, first[0])}/{COLOR_PIP.get(second, second[0])}")
    pips: list[str] = []
    if generic:
        pips.append(str(generic))
    pips.extend(COLOR_PIP.get(color, color[:1].upper()) for color in colors)
    pips.extend(hybrids)
    if not pips:
        pips.append("0")
    return generic, colors, pips, hybrids


def type_line(types: list[str]) -> str:
    if not types:
        return ""
    # Engine uses CardType::* names; join like a short Oracle type line.
    return " ".join(types)


def enrich_card(card: dict[str, Any], catalog: dict[str, dict[str, Any]]) -> dict[str, Any]:
    facts = catalog.get(card["id"]) or {}
    types = list(facts.get("types") or card.get("types") or [])
    land_name = card["name"] in {
        "Plains",
        "Island",
        "Swamp",
        "Mountain",
        "Forest",
    } or any(
        token in card["name"]
        for token in (
            "Aqueduct",
            "Fountain",
            "Foundry",
            "Guildgate",
            "Tomb of",
            "Promenade",
            "Bastion",
            "Boilerworks",
            "Sanctuary",
        )
    )
    if not types and land_name:
        types = ["Land"]
    card["types"] = types
    card["type_line"] = type_line(types)
    card["power"] = facts.get("power")
    card["toughness"] = facts.get("toughness")
    return card


def parse_card_line(
    line: str, catalog: dict[str, dict[str, Any]]
) -> dict[str, Any] | None:
    match = PACK_LINE.match(line.strip())
    if not match:
        return None
    index, name, card_id, cost_body = match.groups()
    generic, colors, pips, hybrids = parse_mana_cost_body(cost_body)
    card = {
        "index": int(index),
        "name": name.strip(),
        "id": card_id.strip(),
        "mana": "".join(pips),
        "mana_pips": pips,
        "generic": generic,
        "colors": colors,
        "hybrids": hybrids,
        "power": None,
        "toughness": None,
        "types": [],
        "type_line": "",
    }
    return enrich_card(card, catalog)


def parse_card_block(
    block: str | None, catalog: dict[str, dict[str, Any]]
) -> list[dict[str, Any]]:
    if not block:
        return []
    cards: list[dict[str, Any]] = []
    for line in block.splitlines():
        card = parse_card_line(line, catalog)
        if card:
            cards.append(card)
    return cards


def parse_reply(reply: str) -> tuple[int | None, str]:
    choice_match = CHOICE_RE.search(reply or "")
    reason_match = REASON_RE.search(reply or "")
    choice = int(choice_match.group(1)) if choice_match else None
    reasoning = ""
    if reason_match:
        reasoning = bytes(reason_match.group(1), "utf-8").decode("unicode_escape")
    return choice, reasoning


def dump(path: Path, payload: Any) -> bytes:
    body = json.dumps(payload, indent=2, sort_keys=True).encode("utf-8") + b"\n"
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(body)
    return body


def model_slug(provider: str) -> str:
    return (
        provider.replace("google/", "")
        .replace("openai/", "")
        .replace("deepseek/", "")
        .replace(".", "-")
        .replace("_", "-")
    )


def build_replay(
    *,
    seat_zero: int,
    seat_meta: dict[str, Any],
    events: list[dict[str, Any]],
    provider: str,
    seed: int,
    source_digest: str,
    catalog: dict[str, dict[str, Any]],
    mana_curve: dict[str, Any] | None = None,
    effort: str = "low",
) -> tuple[str, dict[str, Any]]:
    seat = seat_zero + 1
    slug = model_slug(provider)
    effort_tag = "" if effort == "low" else f"-{effort}"
    trace_id = f"draft-{slug}{effort_tag}-seat-{seat}-seed-{seed}"
    steps: list[dict[str, Any]] = []
    frames: list[dict[str, Any]] = []
    actions: list[dict[str, Any]] = []

    for index, event in enumerate(events, start=1):
        prompt = event.get("prompt") or ""
        reply = event.get("reply") or ""
        accepted = bool(event.get("accepted"))
        fallback = event.get("fallback_reason")
        choice, reasoning = parse_reply(reply)
        pack = parse_card_block(
            PACK_BLOCK.search(prompt).group(1) if PACK_BLOCK.search(prompt) else None,
            catalog,
        )
        pool = parse_card_block(
            POOL_BLOCK.search(prompt).group(1) if POOL_BLOCK.search(prompt) else None,
            catalog,
        )
        header = PICK_HEADER.search(prompt)
        pack_n = int(header.group(1)) if header else None
        pick_n = int(header.group(2)) if header else index
        chosen = next((c for c in pack if c["index"] == choice), None)
        outcome = "accepted" if accepted else "rejected"
        if fallback:
            outcome = "fallback"

        title = f"Pack {pack_n} pick {pick_n}" if pack_n else f"Pick {pick_n}"
        if chosen:
            title = f"{title} · {chosen['name']}"

        steps.append(
            {
                "id": f"pick-{index}",
                "index": index,
                "kind": "policy_call",
                "title": title,
                "timestamp": None,
                "turn_start": pick_n,
                "turn_end": pick_n,
                "tokens": {},
                "content": {
                    "observation": prompt,
                    "reasoning": reasoning or None,
                    "message": reply.strip() or None,
                },
                "action": {
                    "planned": [f"choice:{choice}"] if choice is not None else [],
                    "executed": (
                        [{"turn": pick_n, "name": chosen["name"]}]
                        if chosen and accepted
                        else []
                    ),
                    "rejected": (
                        [{"turn": pick_n, "name": chosen["name"] if chosen else f"choice:{choice}", "reason": fallback or outcome}]
                        if not accepted
                        else []
                    ),
                },
                "reward": 1.0 if accepted else 0.0,
                "achievements": [],
                "state_delta": {},
                "raw": {
                    "pack_n": pack_n,
                    "pick_n": pick_n,
                    "choice": choice,
                    "accepted": accepted,
                    "fallback_reason": fallback,
                    "outcome": outcome,
                },
            }
        )
        frames.append(
            {
                "turn": pick_n,
                "pick_index": index,
                "pack_n": pack_n,
                "pick_n": pick_n,
                "chosen_index": choice,
                "outcome": outcome,
                "pack_cards": pack,
                "pool_cards": pool,
                "chosen_card": chosen,
                "width": 720,
                "height": 420,
            }
        )
        if chosen:
            actions.append(
                {
                    "turn": pick_n,
                    "action": f"draft_pick:{chosen['name']}",
                    "outcome": outcome,
                }
            )

    points = int(seat_meta.get("points") or 0)
    wins = int(seat_meta.get("wins") or 0)
    agency = float(seat_meta.get("agency_pct") or 0.0)
    replay = {
        "schema": "cardbench_magic_draft_replay_v1",
        "id": trace_id,
        "model": provider,
        "effort": effort,
        "arm": "draft",
        "agent_count": 1,
        "scenario": "ravnica_llm_draft",
        "seed": seed,
        "seat": seat,
        "success": agency >= 95.0,
        "terminal_reason": "draft_complete",
        "score": points,
        "total_reward": float(wins),
        "actions": actions,
        "frames": frames,
        "environment": "cardbench-magic-draft",
        "protocol": "llm-draft-seed-73",
        "mana_curve": mana_curve,
        "trace": {
            "schema": "eval_trace_view_v1",
            "source_schema": "synth.visual-input.trace-v5-decision-series.v1",
            "trace_id": trace_id,
            "task": {
                "id": "cardbench-magic-draft",
                "name": "CardBench Magic · Ravnica Limited draft",
                "family": "cardbench-magic",
            },
            "run": {
                "model": provider,
                "provider": "openrouter",
                "effort": effort,
                "seed": seed,
                "status": "completed",
                "seat": seat,
                "points": points,
                "wins": wins,
                "agency_pct": agency,
            },
            "integrity": {
                "status": "recorded",
                "content_digest": f"sha256:{source_digest}",
                "source": "openrouter-gemini-flash-lite-draft-73",
            },
            "system_prompt": None,
            "steps": steps,
        },
    }
    return trace_id, replay


def package_one_artifact(
    *,
    artifact: Path,
    out: Path,
    catalog: dict[str, dict[str, Any]],
) -> dict[str, Any]:
    """Package one draft+eval artifact into rollouts/replays; return matrix slice."""
    draft_path = artifact / "llm-draft.json"
    summary_path = artifact / "eval" / "summary.json"
    series_path = artifact / "eval" / "traces" / "draft" / "decision-series.v1.json"
    if not draft_path.exists():
        raise FileNotFoundError(f"missing draft artifact: {draft_path}")
    if not summary_path.exists():
        raise FileNotFoundError(f"missing eval summary: {summary_path}")

    draft = json.loads(draft_path.read_text())
    summary = json.loads(summary_path.read_text())
    provider = str(draft.get("provider") or summary.get("provider"))
    seed = int(draft.get("seed") or summary.get("seed") or 73)
    effort = str(draft.get("reasoning_effort") or draft.get("effort") or "low")
    if effort in ("", "none", "null"):
        effort = "low"
    source_digest = sha256_file(draft_path)
    slug = model_slug(provider)

    by_seat: dict[int, list[dict[str, Any]]] = {i: [] for i in range(8)}
    for event in draft.get("transcript") or []:
        seat = int(event["seat"])
        by_seat[seat].append(event)

    seat_meta = {int(row["seat"]): row for row in summary.get("seats") or []}
    rollouts: list[dict[str, Any]] = []

    for seat_zero in range(8):
        events = by_seat[seat_zero]
        meta = seat_meta.get(seat_zero + 1) or {}
        decklist_path = artifact / "eval" / "decklists" / f"seat-{seat_zero + 1}.json"
        score_path = artifact / "eval" / "scores" / f"seat-{seat_zero + 1}.json"
        mana_curve = None
        if decklist_path.exists():
            mana_curve = mana_curve_from_decklist(
                json.loads(decklist_path.read_text()),
                catalog,
            )
        score_exists = score_path.exists()
        economics = estimate_seat_economics(
            provider=provider,
            events=events,
            score_path=score_path if score_exists else None,
        )
        points = int(meta.get("points") or 0)
        performance = seat_performance_metrics(
            points=points,
            score_path=score_path if score_exists else None,
        )
        composite_parts: list[float] = [float(performance["drafting_efficacy_elo"])]
        # Limited full-draft Elo only here; constructed elo_1v1 is merged by
        # package_magic_format_llm_v5 from react-vs-code receipts.
        if performance["full_draft_elo"] is not None:
            composite_parts.append(float(performance["full_draft_elo"]))
        for key in ("elo_2hg", "elo_commander"):
            value = performance[key]
            if value is not None:
                composite_parts.append(float(value))
        composite = round(sum(composite_parts) / len(composite_parts), 1)
        trace_id, replay = build_replay(
            seat_zero=seat_zero,
            seat_meta=meta,
            events=events,
            provider=provider,
            seed=seed,
            source_digest=source_digest,
            catalog=catalog,
            mana_curve=mana_curve,
            effort=effort,
        )
        replay["cost_usd"] = economics["cost_usd"]
        replay["est_latency_s"] = economics["est_latency_s"]
        replay["model_calls"] = economics["model_calls"]
        replay["input_tokens_est"] = economics["input_tokens_est"]
        replay["output_tokens_est"] = economics["output_tokens_est"]
        replay["drafting_efficacy"] = performance["drafting_efficacy"]
        replay["full_draft_elo"] = performance["full_draft_elo"]
        replay["elo_1v1"] = performance["elo_1v1"]
        replay["composite"] = composite
        body = dump(out / "v5" / "replays" / f"{trace_id}.json", replay)
        digest = sha256_bytes(body)
        rollouts.append(
            {
                "id": trace_id,
                "sha256": digest,
                "model": provider,
                "effort": effort,
                "arm": "draft",
                "agent_count": 1,
                "scenario": "ravnica_llm_draft",
                "seed": seed,
                "seat": seat_zero + 1,
                "success": bool(replay["success"]),
                "terminal_reason": "draft_complete",
                "score": replay["score"],
                "total_reward": replay["total_reward"],
                "achievements": [],
                "invalid_actions": 0,
                "steps": len(replay["trace"]["steps"]),
                "cost_usd": economics["cost_usd"],
                "cost_sources": economics["cost_sources"],
                "est_latency_s": economics["est_latency_s"],
                "latency_sources": economics["latency_sources"],
                "model_calls": economics["model_calls"],
                "input_tokens_est": economics["input_tokens_est"],
                "output_tokens_est": economics["output_tokens_est"],
                "mean_limited_turns": economics["mean_limited_turns"],
                "environment": "cardbench-magic-draft",
                "protocol": f"llm-draft-seed-{seed}",
                "agency_pct": float(meta.get("agency_pct") or 0.0),
                "wins": int(meta.get("wins") or 0),
                "losses": int(meta.get("losses") or 0),
                "points": points,
                "drafting_efficacy": performance["drafting_efficacy"],
                "full_draft_elo": performance["full_draft_elo"],
                "elo_1v1": performance["elo_1v1"],
                "elo_2hg": performance["elo_2hg"],
                "elo_commander": performance["elo_commander"],
                "drafting_efficacy_elo": performance["drafting_efficacy_elo"],
                "composite": composite,
                "metric_sources": performance["metric_sources"],
                "metric_status": performance["metric_status"],
                "limited_matches": performance["limited_matches"],
                "mana_curve": mana_curve,
            }
        )

    return {
        "artifact": str(artifact),
        "provider": provider,
        "slug": slug,
        "seed": seed,
        "effort": effort,
        "source_digest": source_digest,
        "summary_digest": sha256_file(summary_path),
        "decision_series_digest": sha256_file(series_path) if series_path.exists() else None,
        "rollouts": rollouts,
        "cohort": {
            "id": f"{slug}__{effort}__draft",
            "model": provider,
            "effort": effort,
            "arm": "draft",
            "protocol": f"llm-draft-seed-{seed}",
            "rollout_ids": [row["id"] for row in rollouts],
        },
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--artifact",
        type=Path,
        action="append",
        dest="artifacts",
        help="Draft+eval artifact root (repeatable). Defaults to the Gemini 2.5 seed-73 pod.",
    )
    parser.add_argument(
        "--out",
        type=Path,
        default=Path(__file__).resolve().parents[1]
        / "artifacts"
        / "public-eval"
        / "cardbench-magic",
    )
    args = parser.parse_args()
    artifacts: list[Path] = args.artifacts or [
        Path(
            "/Users/joshuapurtell/Documents/Codex/2026-08-04/imp/work/magic-multiplayer"
            "/varieties/magic/artifacts/openrouter-gemini-flash-lite-draft-73"
        )
    ]
    out: Path = args.out
    catalog = load_card_catalog(DEFAULT_RAV_DEFS)
    out.mkdir(parents=True, exist_ok=True)

    # Clear prior replays so multi-model packages do not retain stale seats.
    replay_dir = out / "v5" / "replays"
    if replay_dir.exists():
        for stale in replay_dir.glob("*.json"):
            stale.unlink()

    packaged = [package_one_artifact(artifact=path, out=out, catalog=catalog) for path in artifacts]
    rollouts = [row for pack in packaged for row in pack["rollouts"]]
    if not rollouts:
        raise SystemExit("no rollouts packaged")

    featured = sorted(
        rollouts,
        key=lambda row: (row["points"], row["agency_pct"], -row["seat"]),
        reverse=True,
    )[0]

    # Merge pods that share model×effort into one cohort (multi-seed sample growth).
    cohort_map: dict[str, dict[str, Any]] = {}
    for pack in packaged:
        effort = pack.get("effort") or "low"
        key = f"{pack['provider']}__{effort}__draft"
        if key not in cohort_map:
            cohort_map[key] = {
                "id": f"{pack['slug']}__{effort}__draft",
                "model": pack["provider"],
                "effort": effort,
                "arm": "draft",
                "protocol": "llm-draft-multi-seed",
                "seeds": [],
                "rollout_ids": [],
            }
        cohort_map[key]["seeds"].append(pack["seed"])
        cohort_map[key]["rollout_ids"].extend(row["id"] for row in pack["rollouts"])

    models = sorted({pack["provider"] for pack in packaged})
    matrix = {
        "schema": "cardbench_magic_matrix_v1",
        "benchmark_id": "cardbench/magic",
        "environment": "cardbench-magic-draft",
        "status": "pilot",
        "coverage": {
            "lane": "draft",
            "completed_rollouts": len(rollouts),
            "expected_rollouts": 8 * len(packaged),
            "models": models,
            "seeds": sorted({pack["seed"] for pack in packaged}),
            "play_frames": "pending",
            "source_artifacts": [pack["artifact"] for pack in packaged],
            "y_metrics": {
                "drafting_efficacy": "available",
                "full_draft_elo": "available",
                "elo_1v1": "pending",
                "elo_2hg": "pending",
                "elo_commander": "pending",
            },
        },
        "protocols": sorted({f"llm-draft-seed-{pack['seed']}" for pack in packaged}),
        "source_runs": [
            {
                "id": Path(pack["artifact"]).name,
                "model": pack["provider"],
                "seed": pack["seed"],
                "digest": f"sha256:{pack['source_digest']}",
            }
            for pack in packaged
        ],
        "excluded_lanes": ["play-react-vs-code", "play-h2h", "play-2hg", "play-commander"],
        "cohorts": list(cohort_map.values()),
        "featured_rollout_id": featured["id"],
        "rollouts": rollouts,
        "decision_series_digests": {
            f"{pack['provider']}__seed-{pack['seed']}": (
                f"sha256:{pack['decision_series_digest']}"
                if pack["decision_series_digest"]
                else None
            )
            for pack in packaged
        },
    }
    dump(out / "v5" / "matrix.json", matrix)
    dump(
        out / "meta" / "source.json",
        {
            "artifacts": [
                {
                    "artifact": pack["artifact"],
                    "provider": pack["provider"],
                    "seed": pack["seed"],
                    "draft_digest": f"sha256:{pack['source_digest']}",
                    "summary_digest": f"sha256:{pack['summary_digest']}",
                    "decision_series_digest": (
                        f"sha256:{pack['decision_series_digest']}"
                        if pack["decision_series_digest"]
                        else None
                    ),
                }
                for pack in packaged
            ],
            "featured_rollout_id": featured["id"],
        },
    )

    print(f"wrote {out}")
    print(
        f"models={len(packaged)} rollouts={len(rollouts)} "
        f"featured={featured['id']} points={featured['points']}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
