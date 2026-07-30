"""VENDORED COPY — do not edit locally.

Source: JoshuaPurtell/evals @ 87e065f0785c51197dfae3890ac110d2a657e6e1
        suites/product/factory/factorybench/projects/stats_gate.py

Vendored rather than imported so CardBench never depends on evals at runtime,
the same rule evals/tcg.md applies to GameBench patterns. Both lanes (Harbor in
this repo, Dock in evals) compute the heldout lift verdict from this one copy,
so the two lanes cannot diverge. Pin recorded in engine_pins.toml; re-vendor
from upstream and bump the pin rather than editing here.

Original docstring follows.

Statistical-rigor gate (F3): a lift only counts when the heldout CI excludes 0.

Implements the standing ruling from the 2026-06-28 factory-as-researcher handoff:
candidate-vs-champion lifts are accepted iff the paired-bootstrap confidence
interval over per-unit score deltas lies strictly above zero AND the sample is
large enough (``min_n``). No raw-threshold acceptance path exists here on purpose.

Pure stdlib (``random``/``statistics``/``math``) so the gate is importable in any
lane without optional dependency groups, and deterministic given ``seed``.

Failure classes are surfaced in error messages with a stable prefix:
``alignment_error`` (mismatched/empty inputs), ``value_error`` (NaN/inf scores),
``config_error`` (bad alpha/n_boot/min_n).
"""

from __future__ import annotations

import math
import random
from dataclasses import dataclass
from statistics import fmean
from typing import Sequence

REASON_CI_EXCLUDES_ZERO = "ci_excludes_zero"
REASON_CI_BELOW_ZERO = "ci_below_zero"
REASON_CI_INCLUDES_ZERO = "ci_includes_zero"
REASON_INSUFFICIENT_N = "insufficient_n"


def _require_finite(values: Sequence[float], name: str) -> None:
    for i, v in enumerate(values):
        if math.isnan(v) or math.isinf(v):
            raise ValueError(
                f"value_error: {name}[{i}] is {v!r}; scores must be finite floats"
            )


@dataclass(frozen=True)
class PairedScores:
    """Per-unit scores for a candidate and the incumbent champion, aligned by unit.

    ``unit_ids`` are seed or example ids; ``candidate[i]`` and ``champion[i]``
    must both be the score on ``unit_ids[i]``.
    """

    unit_ids: tuple[str, ...]
    candidate: tuple[float, ...]
    champion: tuple[float, ...]

    def __post_init__(self) -> None:
        n_ids = len(self.unit_ids)
        n_cand = len(self.candidate)
        n_champ = len(self.champion)
        if not (n_ids == n_cand == n_champ):
            raise ValueError(
                "alignment_error: PairedScores lengths differ "
                f"(unit_ids={n_ids}, candidate={n_cand}, champion={n_champ}); "
                "scores must be paired per unit"
            )
        if n_ids == 0:
            raise ValueError("alignment_error: PairedScores is empty (0 units)")
        if len(set(self.unit_ids)) != n_ids:
            raise ValueError(
                "alignment_error: PairedScores.unit_ids contains duplicates; "
                "each unit must appear exactly once"
            )
        _require_finite(self.candidate, "candidate")
        _require_finite(self.champion, "champion")

    @property
    def deltas(self) -> tuple[float, ...]:
        return tuple(c - ch for c, ch in zip(self.candidate, self.champion))


@dataclass(frozen=True)
class LiftVerdict:
    accepted: bool
    mean_delta: float
    ci_lo: float
    ci_hi: float
    n: int
    alpha: float
    min_n: int
    reason: str  # ci_excludes_zero | ci_below_zero | ci_includes_zero | insufficient_n
    direction: str  # positive | negative | mixed

    def to_payload(self) -> dict[str, object]:
        """JSON-safe dict for scorecards/reports."""
        return {
            "accepted": self.accepted,
            "mean_delta": self.mean_delta,
            "ci_lo": self.ci_lo,
            "ci_hi": self.ci_hi,
            "n": self.n,
            "alpha": self.alpha,
            "min_n": self.min_n,
            "reason": self.reason,
            "direction": self.direction,
        }


def _quantile(sorted_values: list[float], q: float) -> float:
    """Linear-interpolation quantile over an ascending-sorted list."""
    pos = q * (len(sorted_values) - 1)
    lo = math.floor(pos)
    hi = math.ceil(pos)
    if lo == hi:
        return sorted_values[lo]
    frac = pos - lo
    return sorted_values[lo] * (1.0 - frac) + sorted_values[hi] * frac


def paired_bootstrap_ci(
    deltas: Sequence[float],
    *,
    n_boot: int = 10000,
    alpha: float = 0.05,
    seed: int = 1337,
) -> tuple[float, float, float]:
    """Returns (ci_lo, ci_hi, mean_delta) via percentile bootstrap over paired deltas."""
    if len(deltas) == 0:
        raise ValueError("alignment_error: deltas is empty; need >=1 paired delta")
    _require_finite(deltas, "deltas")
    if not (0.0 < alpha < 1.0):
        raise ValueError(f"config_error: alpha={alpha} must be in (0, 1)")
    if n_boot < 1:
        raise ValueError(f"config_error: n_boot={n_boot} must be >= 1")

    pool = list(deltas)
    n = len(pool)
    rng = random.Random(seed)
    boot_means = sorted(fmean(rng.choices(pool, k=n)) for _ in range(n_boot))
    ci_lo = _quantile(boot_means, alpha / 2.0)
    ci_hi = _quantile(boot_means, 1.0 - alpha / 2.0)
    return (ci_lo, ci_hi, fmean(pool))


def significance_gate(
    scores: PairedScores,
    *,
    min_n: int = 50,
    alpha: float = 0.05,
    n_boot: int = 10000,
    seed: int = 1337,
) -> LiftVerdict:
    """accepted iff n >= min_n AND ci_lo > 0. Deterministic given seed."""
    if min_n < 1:
        raise ValueError(f"config_error: min_n={min_n} must be >= 1")
    n = len(scores.unit_ids)
    deltas = scores.deltas
    ci_lo, ci_hi, mean_delta = paired_bootstrap_ci(
        deltas, n_boot=n_boot, alpha=alpha, seed=seed
    )
    if ci_lo > 0.0:
        direction = "positive"
    elif ci_hi < 0.0:
        direction = "negative"
    else:
        direction = "mixed"

    if n < min_n:
        reason = REASON_INSUFFICIENT_N
        accepted = False
    elif direction == "positive":
        reason = REASON_CI_EXCLUDES_ZERO
        accepted = True
    elif direction == "negative":
        reason = REASON_CI_BELOW_ZERO
        accepted = False
    else:
        reason = REASON_CI_INCLUDES_ZERO
        accepted = False
    return LiftVerdict(
        accepted=accepted,
        mean_delta=mean_delta,
        ci_lo=ci_lo,
        ci_hi=ci_hi,
        n=n,
        alpha=alpha,
        min_n=min_n,
        reason=reason,
        direction=direction,
    )


__all__ = [
    "LiftVerdict",
    "PairedScores",
    "REASON_CI_BELOW_ZERO",
    "REASON_CI_EXCLUDES_ZERO",
    "REASON_CI_INCLUDES_ZERO",
    "REASON_INSUFFICIENT_N",
    "paired_bootstrap_ci",
    "significance_gate",
]
