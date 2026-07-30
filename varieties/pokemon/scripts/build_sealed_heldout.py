#!/usr/bin/env python3
"""Assemble the sealed heldout split for cardbench/pokemon/code_policy.

This script is committed; the assets it reads and writes are not. ``cardbench``
is a public repository, so the heldout decklists and opponent ladder live only
under ``varieties/pokemon/.sealed/`` (gitignored) and are described here purely
by shape. Nothing in this file reveals the split.

Inputs  (sealed, hand-authored):
    .sealed/code_policy/inputs/deck_specs.json
    .sealed/code_policy/inputs/opponent_manifest.json

Outputs (sealed, generated):
    .sealed/code_policy/decks/<deck_id>.json
    .sealed/code_policy/opponents/<opponent_id>.rs
    .sealed/code_policy/server_heldout.sqlite
    .sealed/code_policy/heldout_v1.json

The committed roster (``rosters/code_policy_v1.json``) carries only the sha256
of ``heldout_v1.json`` plus counts. ``--verify`` recomputes that digest and
checks it against the roster, which is what the Dock verdict gate pins.

Because the sealed tree is gitignored it is not recoverable from a clone. Back
it up privately; see .sealed/README.md.
"""

from __future__ import annotations

import argparse
import collections
import hashlib
import json
import re
import shutil
import sqlite3
import sys
from itertools import permutations
from pathlib import Path

POKEMON_ROOT = Path(__file__).resolve().parents[1]
SEALED_ROOT = POKEMON_ROOT / ".sealed" / "code_policy"
INPUTS = SEALED_ROOT / "inputs"
ROSTER = POKEMON_ROOT / "rosters" / "code_policy_v1.json"
CARDS_DB = POKEMON_ROOT / "policies" / "data" / "cards.sqlite"
SERVER_DB = POKEMON_ROOT / "policies" / "data" / "server.sqlite"
DEFAULT_LEGACY_ROOT = (
    Path.home()
    / "Documents"
    / "GitHub"
    / "engine-bench-tcg"
    / "auxiliary_tasks"
    / "algo_bench"
    / "results"
)

MANIFEST_SCHEMA = "cardbench.pokemon.code_policy_heldout.v1"


class SealError(RuntimeError):
    """Raised when the sealed split cannot be assembled or verified."""


# --------------------------------------------------------------------------
# validation
# --------------------------------------------------------------------------


def _train_card_ids() -> set[str]:
    """Non-Energy card ids reachable from any agent-visible published deck."""
    with sqlite3.connect(SERVER_DB) as connection:
        rows = connection.execute(
            "SELECT cards_json FROM decks WHERE is_public = 1"
        ).fetchall()
    ids: set[str] = set()
    for (cards_json,) in rows:
        for entry in json.loads(cards_json):
            card_id = str(entry["card_def_id"])
            if not card_id.upper().startswith("ENERGY-"):
                ids.add(card_id)
    return ids


def validate_deck(deck: dict, *, cards: sqlite3.Connection, train: set[str]) -> None:
    """Legality plus the heldout-specific disjointness requirement.

    Legality mirrors ``run_deck_eval.load_legal_deck``: exactly 60 cards, at
    least one basic Energy, at most 4 copies of any non-Energy card.
    """
    name = deck["name"]
    counts: collections.Counter[str] = collections.Counter()
    for entry in deck["cards"]:
        counts[str(entry["card_def_id"])] += int(entry["count"])

    total = sum(counts.values())
    if total != 60:
        raise SealError(f"{name}: deck must contain exactly 60 cards, found {total}")

    if not any(card.upper().startswith("ENERGY-") for card in counts):
        raise SealError(f"{name}: deck must contain at least one basic Energy")

    over = {
        card: count
        for card, count in counts.items()
        if not card.upper().startswith("ENERGY-") and count > 4
    }
    if over:
        raise SealError(f"{name}: non-Energy copy limit exceeded: {over}")

    missing = [
        card
        for card in counts
        if not cards.execute(
            "SELECT 1 FROM cards WHERE card_def_id = ?", (card,)
        ).fetchone()
    ]
    if missing:
        raise SealError(f"{name}: unknown card ids {sorted(missing)}")

    basics = 0
    for card, count in counts.items():
        row = cards.execute(
            "SELECT stage FROM cards WHERE card_def_id = ?", (card,)
        ).fetchone()
        if row and row[0] == "Basic":
            basics += count
    if basics < 8:
        raise SealError(
            f"{name}: only {basics} Basic Pokemon; a deck that cannot reliably "
            "open is not a usable heldout matchup"
        )

    overlap = sorted(
        card
        for card in counts
        if card in train and not card.upper().startswith("ENERGY-")
    )
    if overlap:
        raise SealError(
            f"{name}: shares non-Energy cards with the agent-visible train pool "
            f"({overlap}); heldout decks must come from an unseen card pool"
        )


# --------------------------------------------------------------------------
# assembly
# --------------------------------------------------------------------------


def write_decks(specs: dict, destination: Path) -> list[dict]:
    destination.mkdir(parents=True, exist_ok=True)
    written = []
    for deck in specs["decks"]:
        payload = {
            "name": deck["name"],
            "source": "cardbench sealed heldout split",
            "archetype": deck["archetype"],
            "cards": deck["cards"],
        }
        (destination / f"{deck['deck_id']}.json").write_text(
            json.dumps(payload, indent=2) + "\n"
        )
        written.append(deck)
    return written


_PROMPT_PATTERN = re.compile(r"(Prompt::\w+\s*\{)([^{}]*?)(\}\s*=>)", re.DOTALL)


def port_opponent_source(code: str) -> str:
    """Make a legacy policy compile against the current engine ABI.

    The lifted algo_bench policies were written against an older ``Prompt``
    enum. Several variants have gained fields since, so exhaustive struct
    patterns like ``Prompt::ChooseAttack { attacks }`` now fail to compile with
    E0027. Rewriting those match arms to ``{ attacks, .. }`` is behaviour
    preserving: the arm still matches the same variant and binds the same
    names.

    Applied here, in the builder, so the port is reproducible from the upstream
    sources rather than hand-edited into the sealed tree. Policies needing more
    than this (renamed fields, changed types) are not ported — they are dropped
    by the compile probe instead of being silently rewritten.
    """

    def add_rest(match: re.Match[str]) -> str:
        head, fields, tail = match.group(1), match.group(2), match.group(3)
        if ".." in fields:
            return match.group(0)
        stripped = fields.strip()
        if not stripped:
            return match.group(0)
        separator = "" if stripped.endswith(",") else ","
        return f"{head}{fields.rstrip()}{separator} .. {tail}"

    return _add_prompt_catch_all(_PROMPT_PATTERN.sub(add_rest, code))


def _add_prompt_catch_all(code: str) -> str:
    """Append ``_ => {}`` to exhaustive ``match prompt`` statements.

    The legacy policies enumerated every ``Prompt`` variant with no catch-all.
    The engine's enum has since gained variants, so those matches now fail with
    E0004. Adding a no-op arm restores compilation and preserves behaviour for
    every variant the policy already handled; prompts it never knew about
    simply go unanswered, exactly as they would have if the author had written
    a catch-all at the time.

    Only applied to ``match prompt`` in statement position, where a ``{}`` arm
    is type-correct.
    """
    needle = "match prompt {"
    out = code
    search_from = 0
    while True:
        start = out.find(needle, search_from)
        if start == -1:
            return out

        line_start = out.rfind("\n", 0, start) + 1
        if out[line_start:start].strip():
            # Not statement position (e.g. `let x = match prompt {`).
            search_from = start + len(needle)
            continue

        depth = 0
        index = start + len(needle) - 1
        end = -1
        while index < len(out):
            if out[index] == "{":
                depth += 1
            elif out[index] == "}":
                depth -= 1
                if depth == 0:
                    end = index
                    break
            index += 1
        if end == -1:
            return out

        body = out[start:end]
        if re.search(r"^\s*_\s*(if\b[^=]*)?=>", body, re.MULTILINE):
            search_from = end
            continue

        indent = " " * (start - line_start + 4)
        arm = f"{indent}_ => {{}}\n"
        close_line_start = out.rfind("\n", 0, end) + 1
        out = out[:close_line_start] + arm + out[close_line_start:]
        search_from = end + len(arm)


def write_opponents(
    manifest: dict, legacy_root: Path, destination: Path
) -> list[dict]:
    destination.mkdir(parents=True, exist_ok=True)
    written = []
    for opponent in manifest["opponents"]:
        source = legacy_root / opponent["source"]
        if not source.is_file():
            raise SealError(
                f"missing legacy opponent source: {source}. Pass --legacy-root "
                "if engine-bench-tcg lives elsewhere."
            )
        target = destination / f"{opponent['id']}.rs"
        original = source.read_text()
        ported = port_opponent_source(original)
        target.write_text(ported)
        written.append(
            {
                "id": opponent["id"],
                "source": str(target.relative_to(SEALED_ROOT)),
                "sha256": hashlib.sha256(target.read_bytes()).hexdigest(),
                "upstream_sha256": hashlib.sha256(original.encode()).hexdigest(),
                "ported": ported != original,
            }
        )
    return written


def build_server_db(decks: list[dict], destination: Path) -> None:
    """Heldout decks as ``is_public = 1`` rows the Rust harness can load.

    Schema matches ``policies/data/server.sqlite`` so the generated benchmark
    binary's ``load_public_deck_specs`` query works unchanged.
    """
    if destination.exists():
        destination.unlink()
    with sqlite3.connect(destination) as connection:
        connection.execute(
            """
            CREATE TABLE decks (
                deck_id TEXT PRIMARY KEY,
                user_id TEXT,
                name TEXT NOT NULL,
                cards_json TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL,
                is_public INTEGER NOT NULL DEFAULT 0
            )
            """
        )
        for deck in decks:
            connection.execute(
                "INSERT INTO decks VALUES (?, ?, ?, ?, ?, ?, ?)",
                (
                    deck["deck_id"],
                    None,
                    deck["name"],
                    json.dumps(deck["cards"], separators=(",", ":")),
                    0,
                    0,
                    1,
                ),
            )


def build_manifest(
    decks: list[dict], opponents: list[dict], seed_bases: list[int], matches: int
) -> dict:
    """Canonical cell ordering: ordered deck pair x opponent x side x seed.

    Pairs are ordered, not combinations: the candidate always pilots
    ``candidate_deck`` and the opponent pilots ``opponent_deck``, so unordered
    pairs would leave the second deck of each pair never piloted by the
    candidate and half the deck pool untested. ``side`` is the seat, which is
    an independent axis.

    Cell ids are the pairing key for the heldout scorecard, so the ordering
    here is authority — the evaluator must reproduce it exactly.
    """
    deck_names = [deck["name"] for deck in decks]
    cells = []
    for candidate_deck, opponent_deck in permutations(deck_names, 2):
        for opponent in opponents:
            for side in ("p1", "p2"):
                for seed in seed_bases:
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
        "schema_version": MANIFEST_SCHEMA,
        "split": "heldout",
        "decks": [
            {"deck_id": deck["deck_id"], "name": deck["name"], "archetype": deck["archetype"]}
            for deck in decks
        ],
        "opponents": opponents,
        "seed_bases": list(seed_bases),
        "matches_per_opponent_per_side": matches,
        "server_db": "server_heldout.sqlite",
        "cell_count": len(cells),
        "cells": cells,
    }


def manifest_digest(manifest: dict) -> str:
    return hashlib.sha256(
        json.dumps(manifest, sort_keys=True, separators=(",", ":")).encode()
    ).hexdigest()


def assemble(legacy_root: Path) -> tuple[dict, str]:
    for required in (INPUTS / "deck_specs.json", INPUTS / "opponent_manifest.json"):
        if not required.is_file():
            raise SealError(f"missing sealed input: {required}")

    specs = json.loads((INPUTS / "deck_specs.json").read_text())
    opponent_manifest = json.loads((INPUTS / "opponent_manifest.json").read_text())

    train = _train_card_ids()
    with sqlite3.connect(CARDS_DB) as cards:
        for deck in specs["decks"]:
            validate_deck(deck, cards=cards, train=train)

    decks = write_decks(specs, SEALED_ROOT / "decks")
    opponents = write_opponents(opponent_manifest, legacy_root, SEALED_ROOT / "opponents")
    build_server_db(decks, SEALED_ROOT / "server_heldout.sqlite")

    manifest = build_manifest(
        decks,
        opponents,
        opponent_manifest["seed_bases"],
        int(opponent_manifest["matches_per_opponent_per_side"]),
    )
    (SEALED_ROOT / "heldout_v1.json").write_text(json.dumps(manifest, indent=2) + "\n")
    return manifest, manifest_digest(manifest)


def verify(digest: str, manifest: dict) -> int:
    if not ROSTER.is_file():
        raise SealError(f"missing committed roster: {ROSTER}")
    roster = json.loads(ROSTER.read_text())
    heldout = roster["heldout"]
    problems = []
    if heldout.get("sha256") != digest:
        problems.append(
            f"roster sha256 {heldout.get('sha256')!r} != assembled {digest!r}"
        )
    for field, actual in (
        ("deck_count", len(manifest["decks"])),
        ("opponent_count", len(manifest["opponents"])),
        ("cell_count", manifest["cell_count"]),
    ):
        if int(heldout.get(field, -1)) != actual:
            problems.append(f"roster {field}={heldout.get(field)} != assembled {actual}")
    if problems:
        for problem in problems:
            print(f"sealed_heldout_mismatch: {problem}", file=sys.stderr)
        return 1
    print(f"sealed heldout verified: sha256={digest} cells={manifest['cell_count']}")
    return 0


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--legacy-root",
        type=Path,
        default=DEFAULT_LEGACY_ROOT,
        help="engine-bench-tcg algo_bench results directory",
    )
    parser.add_argument(
        "--verify",
        action="store_true",
        help="rebuild and check the digest against the committed roster",
    )
    parser.add_argument(
        "--print-roster-fields",
        action="store_true",
        help="emit the heldout block to paste into the committed roster",
    )
    args = parser.parse_args()

    try:
        manifest, digest = assemble(args.legacy_root.expanduser().resolve())
    except SealError as exc:
        print(f"sealed_heldout_error: {exc}", file=sys.stderr)
        return 1

    if args.print_roster_fields:
        print(
            json.dumps(
                {
                    "sha256": digest,
                    "deck_count": len(manifest["decks"]),
                    "opponent_count": len(manifest["opponents"]),
                    "cell_count": manifest["cell_count"],
                },
                indent=2,
            )
        )
        return 0

    if args.verify:
        return verify(digest, manifest)

    print(
        f"assembled sealed heldout split: {manifest['cell_count']} cells, "
        f"{len(manifest['decks'])} decks, {len(manifest['opponents'])} opponents"
    )
    print(f"sha256={digest}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
