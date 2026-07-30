# RAV Rust policy fixtures

This crate is the public development surface for `cardbench/magic/code_policy`.
It contains six deterministic Rust policies:

- `rav.boros-tempo.v1` develops lands/mana, prioritizes `Lightning Helix` and
  `Char`, and completes combat declarations.
- `rav.selesnya-convoke.v1` develops Forests/Brownscales, uses legal convoke,
  attacks, blocks, and completes combat declarations.
- `rav.boros-char-control.v1` develops red/white mana, protects its own life
  total around Char, and uses Helix for lethal or interaction.
- `rav.selesnya-siege.v1` builds a green convoke board and chooses explicit
  attacks and low-value blockers.
- `rav.golgari-attrition.v1` holds black mana for Last Gasp while executing a
  green Brownscale/convoke plan.
- `rav.selesnya-radiance-tokens.v1` is a three-color development fixture for
  tokens, convoke, Rally radiance, continuous layers, and combat.

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
cargo run -p cardbench-magic-policies --bin rav-engine-tournament
cargo run -p cardbench-magic-policies --bin rav-reference-deck-matrix
cargo run -p cardbench-magic-policies --bin rav-engine-audit
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

`rav-engine-tournament` is stricter: it runs sixteen public seeds and fails for
every invariant violation, capability gap, policy rejection, or bounded
non-winner. It is the command to use when the goal is engine bug discovery,
not merely observing policy behavior.

`rav-reference-deck-matrix` runs every ordered pair of indexed public decks
across eight deterministic seeds, preserving deck IDs and digests on each
trace. `rav-engine-audit` adds adversarial public-API probes and a shorter
interactive matrix; both fail closed when an invariant, policy, or coverage
problem is discovered.
