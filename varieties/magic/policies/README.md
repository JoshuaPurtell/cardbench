# RAV Rust policy fixtures

This crate is the public development surface for `cardbench/magic/code_policy`.
It contains two deliberately small, deterministic Rust policies:

- `rav.boros-tempo.v1` submits `Lightning Helix` when it has priority, then passes.
- `rav.selesnya-convoke.v1` submits `Scatter the Seeds` with three valid convoke
  contributions when the prepared state makes that legal, then passes.

Policies do not mutate `Game`. They inspect `GameView`, return `PolicyAction`,
and submit it through `Game::submit_policy_move`. A successful submission adds a
`PolicyMoveSubmitted` event in the same canonical log as the resulting cast,
priority, resolution, damage, life, and token events.

Run the fixture match with:

```bash
cargo run -p cardbench-magic-policies --bin rav-policy-match
```

The checked-in contract is [`reference_match.toml`](reference_match.toml). It
pins the required event kinds, terminal state, and full-event-log digest for the
seeded development opening. It is not a hidden evaluation or a complete game AI.
