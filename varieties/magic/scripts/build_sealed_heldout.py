#!/usr/bin/env python3
"""Assemble the sealed heldout split for cardbench/magic/code_policy.

This script is committed; the assets it reads and writes are not. ``cardbench``
is a public repository, so the heldout opponent composition lives only under
``varieties/magic/.sealed/`` (gitignored) and is described here purely by shape.
Nothing in this file reveals the split.

Inputs  (sealed, hand-authored -- the irreplaceable part):
    .sealed/code_policy/inputs/opponent_manifest.json

Outputs (sealed, generated):
    .sealed/code_policy/heldout_v1.json

The committed roster (``rosters/code_policy_v1.json``) carries only the sha256 of
``heldout_v1.json`` plus counts. ``--verify`` recomputes that digest and checks it
against the roster; the Rust sweep recomputes the same digest before it will
score a heldout run, so a heldout surface that moved cannot be scored by either
lane.

Canonical form is ``json.dumps(manifest, sort_keys=True, separators=(",", ":"))``
and the manifest is required to be pure ASCII. Both constraints exist so
``policies/src/code_policy/sha256.rs`` -- which hashes ``serde_json``'s compact,
key-ordered rendering -- produces byte-identical input. ``--verify`` proves the
two agree rather than assuming it.

Because the sealed tree is gitignored it is not recoverable from a clone. Back up
``inputs/`` privately; see .sealed/README.md.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import subprocess
import sys
import tomllib
from pathlib import Path

MAGIC_ROOT = Path(__file__).resolve().parents[1]
SEALED_ROOT = MAGIC_ROOT / ".sealed" / "code_policy"
INPUTS = SEALED_ROOT / "inputs"
ROSTER = MAGIC_ROOT / "rosters" / "code_policy_v1.json"
DECK_INDEXES = (
    MAGIC_ROOT / "sets" / "ravnica_city_of_guilds" / "decks" / "constructed_decks.toml",
    MAGIC_ROOT / "sets" / "ravnica_city_of_guilds" / "decks" / "hillclimb_decks.toml",
)
ARCHETYPES_MOD = MAGIC_ROOT / "policies" / "src" / "archetypes" / "mod.rs"

MANIFEST_SCHEMA = "cardbench.magic.code_policy_heldout.v1"


class SealError(RuntimeError):
    """Raised when the sealed split cannot be assembled or verified."""


# --------------------------------------------------------------------------
# the surface the manifest is validated against
# --------------------------------------------------------------------------


def playable_decks() -> dict[str, str]:
    """Deck id -> archetype, over the two indexes a roster may name.

    Read from the same two files ``roster.rs::playable_fixtures`` reads, and
    deliberately not from ``reference_decks.toml``: those fixtures run 44 to 52
    lands in a sixty-card list to exercise the rules engine, and an opponent
    surface built from them would measure who can beat a deck that cannot
    function.
    """
    decks: dict[str, str] = {}
    for index in DECK_INDEXES:
        if not index.is_file():
            raise SealError(f"missing deck index: {index}")
        for entry in tomllib.loads(index.read_text())["deck"]:
            archetype = entry.get("archetype", "")
            if not archetype:
                raise SealError(f"deck {entry['id']} declares no archetype")
            decks[entry["id"]] = archetype
    return decks


def known_pilots() -> set[str]:
    """Pilot generation ids, read out of the Rust enum rather than duplicated."""
    source = ARCHETYPES_MOD.read_text()
    ids = set(re.findall(r'Self::V\d+ => "(v\d+)"', source))
    if not ids:
        raise SealError(f"could not read pilot ids from {ARCHETYPES_MOD}")
    return ids


def committed_roster() -> dict:
    if not ROSTER.is_file():
        raise SealError(f"missing committed roster: {ROSTER}")
    return json.loads(ROSTER.read_text())


# --------------------------------------------------------------------------
# validation
# --------------------------------------------------------------------------


def validate(manifest_input: dict, roster: dict) -> None:
    """Everything that has to hold before a split is worth sealing."""
    decks = playable_decks()
    pilots = known_pilots()
    opponents = manifest_input["opponents"]

    if not opponents:
        raise SealError("heldout split declares no opponents")

    ids = [opponent["id"] for opponent in opponents]
    if len(set(ids)) != len(ids):
        raise SealError(f"duplicate heldout opponent ids: {ids}")

    for opponent in opponents:
        if opponent["pilot"] not in pilots:
            raise SealError(
                f"{opponent['id']}: unknown pilot {opponent['pilot']!r}; "
                f"known: {sorted(pilots)}"
            )
        if opponent["deck"] not in decks:
            raise SealError(f"{opponent['id']}: unknown deck {opponent['deck']!r}")

    # Disjointness is the whole point, and the committed roster publishes the
    # strong form of it: "no pilot generation and no deck fixture appears in
    # both splits". Enforce exactly that claim, on each axis independently.
    #
    # A pair-level check (reject only when a heldout opponent reuses a train
    # pilot *and* a train deck) is too weak to defend the published sentence: it
    # would admit a heldout opponent flying an already-seen pilot on a fresh
    # deck, or a fresh pilot on an already-seen deck. Either one leaks half the
    # surface the agent is supposed to be blind to, and the train-to-heldout
    # shrinkage — the whole reason the split exists — stops measuring
    # generalisation. The roster sentence would then be false and nothing would
    # catch it, because this script is the only thing that ever regenerates the
    # sealed manifest.
    #
    # The strong rule is satisfiable and is what the current seal already
    # satisfies: train spends 5 of 8 pilot generations and 4 of 10 playable
    # decks, leaving 3 generations and 6 decks to draw a 5-opponent split from.
    train = roster["train"]["opponents"]
    train_pilots = {opponent["pilot"] for opponent in train}
    train_decks = {opponent["deck"] for opponent in train}
    for opponent in opponents:
        if opponent["pilot"] in train_pilots:
            raise SealError(
                f"{opponent['id']}: pilot {opponent['pilot']} is already a "
                f"visible train pilot; the roster claims no pilot generation "
                f"appears in both splits"
            )
        if opponent["deck"] in train_decks:
            raise SealError(
                f"{opponent['id']}: deck {opponent['deck']} is already a "
                f"visible train deck; the roster claims no deck fixture "
                f"appears in both splits"
            )

    # The reference is the origin, not an opponent. Seating it on both sides of
    # the comparison would guarantee a 0.0 delta cell and silently dilute the
    # reward toward zero.
    reference = roster["reference"]
    for opponent in opponents:
        if (
            opponent["pilot"] == reference["pilot"]
            and opponent["deck"] == reference["deck"]
        ):
            raise SealError(
                f"{opponent['id']} is the frozen reference itself; the origin "
                "cannot also be an opponent"
            )

    if int(manifest_input["seeds"]) <= 0:
        raise SealError("a split with zero seeds measures nothing")


# --------------------------------------------------------------------------
# assembly
# --------------------------------------------------------------------------


def build_manifest(manifest_input: dict) -> dict:
    """Canonical cell ordering: opponent x seat x seed.

    This ordering is authority. ``roster.rs::Surface::cells`` enumerates in
    exactly this order and the sweep's coverage check compares against it, so a
    consumer that enumerates differently is scoring a different surface.

    The candidate's own deck and pilot are *not* an axis here: they are the
    submission, so they vary per arm rather than per cell. The shared coordinate
    -- opponent, opponent deck, seat, seed -- is what the two arms pair on.
    """
    decks = playable_decks()
    seeds = int(manifest_input["seeds"])
    opponents = [
        {
            "id": opponent["id"],
            "pilot": opponent["pilot"],
            "deck": opponent["deck"],
            "archetype": decks[opponent["deck"]],
        }
        for opponent in manifest_input["opponents"]
    ]
    cells = []
    for opponent in opponents:
        for seat in ("p0", "p1"):
            for seed in range(seeds):
                cells.append(
                    {
                        "pair_key": f"{opponent['id']}|{opponent['deck']}|{seat}|{seed}",
                        "opponent_id": opponent["id"],
                        "opponent_deck": opponent["deck"],
                        "seat": seat,
                        "seed": seed,
                    }
                )
    return {
        "schema_version": MANIFEST_SCHEMA,
        "split": "heldout",
        "opponents": opponents,
        "seeds": seeds,
        "seats": ["p0", "p1"],
        "cell_count": len(cells),
        "cells": cells,
    }


def canonical(manifest: dict) -> str:
    """The exact byte string both digest implementations hash."""
    text = json.dumps(manifest, sort_keys=True, separators=(",", ":"))
    if not text.isascii():
        raise SealError(
            "sealed manifest must be pure ASCII: Python escapes non-ASCII by "
            "default and serde_json does not, so a non-ASCII manifest would "
            "hash differently in the two lanes"
        )
    return text


def manifest_digest(manifest: dict) -> str:
    return hashlib.sha256(canonical(manifest).encode()).hexdigest()


def assemble() -> tuple[dict, str]:
    source = INPUTS / "opponent_manifest.json"
    if not source.is_file():
        raise SealError(
            f"missing sealed input: {source}. This file is the split; it is "
            "gitignored and cannot be recovered from a clone."
        )
    manifest_input = json.loads(source.read_text())
    validate(manifest_input, committed_roster())
    manifest = build_manifest(manifest_input)
    SEALED_ROOT.mkdir(parents=True, exist_ok=True)
    (SEALED_ROOT / "heldout_v1.json").write_text(json.dumps(manifest, indent=2) + "\n")
    return manifest, manifest_digest(manifest)


# --------------------------------------------------------------------------
# verification
# --------------------------------------------------------------------------


def cross_check_rust_digest(digest: str) -> str | None:
    """Prove the Rust hasher agrees, rather than assuming the canonical forms match.

    Returns a problem description, or ``None`` when the two lanes agree or the
    check could not be run (no cargo, offline build unavailable). A skipped
    cross-check is reported by the caller, never silently treated as a pass.
    """
    try:
        completed = subprocess.run(
            [
                "cargo",
                "run",
                "--offline",
                "--quiet",
                "--release",
                "-p",
                "cardbench-magic-policies",
                "--bin",
                "rav-code-policy-sweep",
                "--",
                "--print-heldout-digest",
            ],
            cwd=MAGIC_ROOT,
            text=True,
            capture_output=True,
            timeout=900,
        )
    except (OSError, subprocess.TimeoutExpired) as exc:  # pragma: no cover
        return f"skipped: {exc}"
    if completed.returncode != 0:
        return f"skipped: sweep binary would not run:\n{completed.stderr[-2000:]}"
    reported = completed.stdout.strip().splitlines()[-1].strip()
    if reported != digest:
        return f"rust digest {reported!r} != python digest {digest!r}"
    return None


def verify(digest: str, manifest: dict) -> int:
    roster = committed_roster()
    heldout = roster["heldout"]
    problems = []
    if heldout.get("sha256") != digest:
        problems.append(
            f"roster sha256 {heldout.get('sha256')!r} != assembled {digest!r}"
        )
    for field, actual in (
        ("opponent_count", len(manifest["opponents"])),
        ("cell_count", manifest["cell_count"]),
        ("seeds", manifest["seeds"]),
    ):
        if int(heldout.get(field, -1)) != actual:
            problems.append(
                f"roster {field}={heldout.get(field)} != assembled {actual}"
            )

    if problems:
        for problem in problems:
            print(f"sealed_heldout_mismatch: {problem}", file=sys.stderr)
        return 1

    cross = cross_check_rust_digest(digest)
    if cross and not cross.startswith("skipped"):
        print(f"sealed_heldout_mismatch: {cross}", file=sys.stderr)
        return 1

    note = "" if cross is None else f" (rust cross-check {cross})"
    print(
        f"sealed heldout verified: sha256={digest} "
        f"opponents={len(manifest['opponents'])} cells={manifest['cell_count']}{note}"
    )
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
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
        manifest, digest = assemble()
    except SealError as exc:
        print(f"sealed_heldout_error: {exc}", file=sys.stderr)
        return 1

    if args.print_roster_fields:
        print(
            json.dumps(
                {
                    "sha256": digest,
                    "opponent_count": len(manifest["opponents"]),
                    "seeds": manifest["seeds"],
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
        f"{len(manifest['opponents'])} opponents, {manifest['seeds']} seeds"
    )
    print(f"sha256={digest}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
