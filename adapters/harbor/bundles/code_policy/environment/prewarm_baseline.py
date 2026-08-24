#!/usr/bin/env python3
"""Compile the ranking origin into the image, on the split(s) named in argv.

The verifier's 900s budget cannot absorb two cold Rust builds plus two sweeps,
and the graded container has no network at all (`NestedTrial.network="none"`),
so crates.io is unreachable at grade time. Both problems are solved here, at
bake time:

  * every crate the benchmark package needs lands in ``CARGO_HOME``, so the
    grade-time build resolves offline;
  * the baseline's target tree is fully populated, so the graded run pays only
    for the candidate.

The work itself lives in each variety's ``run_policy_sweep.prewarm``, because
what has to be warm is variety-specific and a prewarm that guessed would leave
a target tree nothing ever reads -- image bloat masquerading as a cache.
"""

from __future__ import annotations

import os
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[5]
VARIETY = os.environ.get("CARDBENCH_VARIETY", "pokemon")
sys.path.insert(0, str(REPO_ROOT / "varieties" / VARIETY / "scripts"))

import run_policy_sweep as sweep  # noqa: E402


def main(splits: list[str]) -> int:
    # Each variety's driver owns its own prewarm, because what has to be warm
    # differs: pokemon builds a benchmark package keyed on (policy, engine,
    # roster) per split, magic builds one candidate crate that every split
    # reuses. This script only decides which driver to ask.
    entry = getattr(sweep, "prewarm", None)
    if entry is None:
        raise SystemExit(
            f"varieties/{VARIETY}/scripts/run_policy_sweep.py defines no prewarm(); "
            "a bake that skipped it would ship an image whose first graded build "
            "is cold and offline"
        )
    entry(splits)
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:] or ["heldout"]))
