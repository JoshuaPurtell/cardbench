# RAV Rust policy fixtures

This crate is the public development surface for `cardbench/magic/code_policy`.
It contains two deterministic Rust policies:

- `rav.boros-tempo.v1` develops lands/mana, prioritizes `Lightning Helix` and
  `Char`, and completes combat declarations.
- `rav.selesnya-convoke.v1` develops Forests/Brownscales, uses legal convoke,
  attacks, blocks, and completes combat declarations.

Policies do not mutate `Game`. They inspect `GameView`, return `PolicyAction`,
and submit it through `Game::submit_policy_move`. A successful submission adds a
`PolicyMoveSubmitted` event in the same canonical log as the resulting cast,
priority, resolution, damage, life, and token events.

If an interaction cannot be expressed by the currently implemented engine
slice, a policy must return `PolicyAction::ReportEngineWeakness { code, detail }`
rather than pretend it is legal. The reporting policy must have priority. The
engine records `EngineWeaknessRevealed` followed by the
`PolicyMoveSubmitted { kind: ReportEngineWeakness }` receipt; the report itself
does not alter zones, priority, or turn state. Use a stable capability code and
reproducible public detail. See [`../engine/INVARIANTS.md`](../engine/INVARIANTS.md)
for the complete invariant and reporting contract.

Run the fixture match with:

```bash
cargo run -p cardbench-magic-policies --bin rav-policy-match
cargo run -p cardbench-magic-policies --bin rav-deck-match
cargo run -p cardbench-magic-policies --bin rav-deck-sweep
```

The checked-in contract is [`reference_match.toml`](reference_match.toml). It
pins the required event kinds, terminal state, and full-event-log digest for the
seeded development opening. It is not a hidden evaluation, a complete game AI,
or a claim of full Oracle Magic coverage.

`rav-deck-match` runs the actual two 60-card fixtures from shuffled libraries,
opening hands, and normal engine-submitted moves. `rav-deck-sweep` runs four
public seeds and promotes only engine findings to its summary:

- `EngineBug`: `Game::validate_invariants()` failed.
- `CapabilityGap`: a policy submitted `ReportEngineWeakness` for unsupported rules.

Policy rejection and configured runner limits are reported separately; neither
is evidence of an engine defect by itself. The combat regression suite documents
and prevents the previously discovered stale-dead-blocker invariant bug.
