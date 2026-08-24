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

This calls the sweep's own ``build_binary`` rather than reimplementing it,
because ``benchmark_ai`` keys its target tree on a hash of (policy source,
engine path, opponent roster). A prewarm that computed that key differently
would leave a tree nothing ever reads -- image bloat masquerading as a cache.
"""

from __future__ import annotations

import sys
import tempfile
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[5]
sys.path.insert(0, str(REPO_ROOT / "varieties" / "pokemon" / "scripts"))

import run_policy_sweep as sweep  # noqa: E402


def main(splits: list[str]) -> int:
    baseline = sweep.baseline_policy_path()
    name = sweep.struct_name(baseline)
    for split in splits:
        surface = sweep.load_split(split)
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            roster = sweep.write_opponent_roster(surface, root / "opponent_roster.json")
            binary = sweep.build_binary(baseline, name, roster, root / "baseline")
        if not binary.is_file():
            raise SystemExit(f"prewarm produced no binary for split={split}")
        print(f"prewarmed {split}: {binary}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:] or ["heldout"]))
