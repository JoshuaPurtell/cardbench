# RAV Lightning Helix single-card PvP pilot

This is a separate single-card task for Lightning Helix (RAV collector 213). It
complements the Char set-PvP pilot and reports against the complete RAV
denominator without claiming that the set is complete.

## Source contract

- Card identity: RAV collector 213, Lightning Helix.
- Printed characteristics: `{R}{W}`, red-white instant.
- Authority: RAV printed semantics; current Oracle was checked as a
  cross-check. The target domain is the RAV-era player-or-creature branch.
- Rules baseline: Comprehensive Rules effective August 7, 2026, the latest
  effective edition at the September 23, 2026 authority cutoff. Later
  future-effective updates are excluded.
- Task-specific inputs are CardBench-authored feature summaries and public
  identifiers. The build workspace includes the source tree needed for scoring,
  but no card art, scans, or copied card-text corpus.

## Verification contract

The authority uses normal shuffled 60-card two-player games to terminal state.
It records player-target and opponent-creature-target traces, requires the
target damage then controller life-gain sequence, compares post-resolution
life and zone state, replays deterministically, and rejects a 3-to-2 damage
mutation. A compile-valid low-damage stub must fail on the same semantic trace.

The report keeps all 291 RAV names and 306 printings visible. Only Lightning
Helix is eligible for this pilot; whole-set coverage and reward remain zero.
Independent review is hash-bound. The task remains diagnostic evidence until a
real candidate trial is retained.

The candidate source must stay a pure, declarative `definition() ->
CardDefinition` with the existing imports and field layout. Keep behavior in the
card-definition fields; the verifier rejects extra items, executable helper
logic, or side effects.
