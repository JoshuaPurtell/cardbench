# RAV Char set-PvP pilot

This is the first per-card Magic PvP task. It implements one public RAV
identity as a compile-valid module at
`varieties/magic/sets/ravnica_city_of_guilds/src/cards/char.rs`.

## Source contract

- Card identity: RAV collector 117, Char.
- Authority: the RAV printing's original rules function under the recorded
  Comprehensive Rules baseline. Current Oracle wording was reviewed as a
  cross-check; later target-domain errata are intentionally excluded from this
  original-set compatibility task.
- Rules baseline: Comprehensive Rules effective August 7, 2026, the latest
  effective edition at the September 23, 2026 authority cutoff. Later
  future-effective updates are excluded.
- Task-specific card inputs are limited to CardBench-authored feature summaries
  and public identifiers. The materialized workspace includes the source tree
  needed to build and score the candidate, but contains no card art, scans, or
  copied card-text corpus.
- The represented target domain is the RAV-era player-or-creature branch. This
  does not claim current Oracle support for later-added target classes.
- Implement the modeled features recorded in `features.toml`; do not infer
  whole-card or whole-set readiness from this pilot.

## Verification contract

The shown reference verifier runs normal shuffled 60-card two-player games to
terminal state. It records player-target and opponent-creature-target traces,
checks the ordered cast / first target damage / controller damage / resolution /
graveyard sequence, folds the life events through the post-resolution
checkpoint, replays each trace deterministically, and checks that a 4-to-3
damage mutation is rejected.

The Harbor candidate worker accepts only a declarative `definition() ->
CardDefinition` source shape. The candidate workspace contains inert verifier
stubs; Harbor mounts the trusted verifier read-only and injects it into the
scoring snapshot only after submission. The scorer checks the report schema,
both ordered event traces, their target and damage facts, the life checkpoints,
and the mutation controls before accepting a pilot pass.

The Harbor report keeps all 291 RAV names and all 306 printings visible. Only
Char is marked as passed by this pilot; whole-set coverage remains incomplete
and whole-set reward stays zero. GPT and DIS remain unreviewed, non-executable
manifests. Independent review is recorded with hashes for the reviewed files;
later source changes clear that gate. The pilot remains diagnostic evidence
until a real agent candidate trial is retained.
