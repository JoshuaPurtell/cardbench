# Magic variety

Magic is a sibling CardBench variety, not a mode of the Pokémon engine. This
directory is a Rust workspace whose reference engine owns Magic-specific rules;
sets supply small executable card definitions and declarative block manifests.

## Runnable surface

```bash
cd varieties/magic
cargo test --workspace
cargo run -p cardbench-magic-rav --bin rav-engine-parity
cargo run -p cardbench-magic-policies --bin rav-policy-match
cargo run -p cardbench-magic-policies --bin rav-deck-match
cargo run -p cardbench-magic-policies --bin rav-deck-sweep
cargo run -p cardbench-magic-policies --bin rav-engine-tournament
cargo run -p cardbench-magic-policies --bin rav-reference-deck-matrix
cargo run -p cardbench-magic-policies --bin rav-engine-audit
```

`rav-engine-parity` validates the original Ravnica-block manifests and public
deck pool, executes twenty-three RAV scenarios twice, and compares each deterministic
event log against its fixed public digest. With `--output-root PATH`, the Rust
binary writes `engine-check.json` and `reward.txt` for the CardBench Harbor
receipt.

| Component | Location | Scope in this milestone |
| --- | --- | --- |
| Expansion-neutral rules | `engine/` | Library, hand, battlefield, graveyard, exile, stack; priority; phases/steps; mana costs; lands; state-based actions; draw replacements; narrow instant/sorcery spell targeting and counters; continuous-effect layers 4–7 |
| RAV fixture crate | `sets/ravnica_city_of_guilds/` | Stack responses/countering, convoke, dredge, transmute, radiance, tokens, and layer/SBA scenarios |
| Block substrate | `sets/*/expansion.toml` | RAV executable; Guildpact and Dissension formal, intentionally non-executable manifests |

The public engine task id is `cardbench/magic/engine`. Its future sibling task
ids are retained without conflating their rewards:

```text
cardbench/magic/code_policy
cardbench/magic/deck_opt
cardbench/magic/engine
cardbench/magic/react
cardbench/magic/cybernetic
```

## Integrity and coverage boundaries

[`engine/INVARIANTS.md`](engine/INVARIANTS.md) is the enforceable public
integrity contract: object/zone ownership, stack exclusivity, priority and
pass sequencing, turn progression, combat state, effects, and state-based
action fixed points. Runners should call `Game::validate_invariants()` after
explicit setup; accepted policy submissions are validated by the engine so a
corrupted state fails at its source.

When a policy reaches a rules interaction the implemented slice cannot
represent, it must submit `PolicyAction::ReportEngineWeakness { code, detail }`
instead of inventing a legal approximation. A successful report emits
`EngineWeaknessRevealed`, then its `PolicyMoveSubmitted` receipt, without
changing game state or priority. This makes coverage gaps visible in the
canonical event log and is particularly important for unsupported combat and
card-text interactions.

## Current card-slice contract

The RAV crate implements only the semantic fragments stated by each card's
`supported_rules` field. For example, `Muddle the Mixture` contributes its
target-instant-or-sorcery counter face and transmute, not an assertion that
every printed ability on that card is available. This makes the initial
expansion slice honest and lets later set work add executable semantics without
changing engine ownership.

RAV shown scenarios are fixture-driven from
`sets/ravnica_city_of_guilds/scenarios/public/train_scenarios.toml`; each declares
setup, actions, state assertions, event markers, and a fixed digest. They cover:

- casting to the stack and both-player priority passes (`Lightning Helix`);
- colored-cost creature casting and permanent characteristics (`Watchwolf`);
- a bounded tap-for-one-of-five-colors mana ability, including summoning-sickness
  rejection and next-turn activation (`Birds of Paradise`);
- a one-mana, fully convoked targeted temporary boost (`Gather Courage`);
- colored and generic convoke payment plus token creation (`Scatter the Seeds`);
- dredge as a draw replacement (`Golgari Brownscale`);
- declared generic Dredge/Convoke compatibility slices with fixed event logs;
  these cover only the named shared keyword plus normal casting/base
  characteristics, never omitted printed behavior (`Golgari Grave-Troll`,
  `Necroplasm`, `Grave-Shell Scarab`, `Shambling Shell`, `Conclave Equenaut`,
  `Conclave Phalanx`, `Guardian of Vitu-Ghazi`, and `Autochthon Wurm`);
- a target-instant-or-sorcery stack counter plus transmute, equal mana-value search, and seeded deterministic shuffle (`Muddle the Mixture`);
- radiance color matching and layer-7 modifiers (`Rally the Righteous`); and
- zero-toughness state-based action after a continuous effect (`Last Gasp`);
- targeted temporary modifiers plus transmute (`Dizzy Spell`), transmute-only
  compatibility (`Brainspoil`), and a targeted temporary modifier plus dredge
  (`Darkblast`); and
- cleanup expiration, land-play limits, and rejected priority/convoke/dredge actions.

## Rust policy development match

Fifteen public 60-card RAV fixture decks live in
`sets/ravnica_city_of_guilds/decks/`. They cover Boros tempo, burn, convoke,
radiance, and token plans; Selesnya convoke and siege; Golgari attrition,
dredge, and Wurm pressure; three-color radiance/convoke; and three Dimir
transmute plans. Each has a matching Rust policy in `policies/`. Policies
receive a public `GameView`, return a narrow `PolicyAction`, and the engine
accepts the move only via
`Game::submit_policy_move`; normal priority, target, and payment checks remain
the engine's responsibility.

`rav-policy-match` runs a seeded, scripted development opening from those two
deck fixtures. The public contract is
[`policies/reference_match.toml`](policies/reference_match.toml): six accepted
policy submissions, life totals `[23, 17]`, three Saproling tokens, and the
following required event families:

- `PolicyMoveSubmitted`, `SpellCast`, `ConvokeUsed`, and `TokenCreated`;
- `PriorityPassed` and `SpellResolved`; and
- `DamageDealtToPlayer` and `LifeGained`.

The runner also pins the complete canonical log to
`fnv1a64:ca603a0d3e5d8114` and prints every event. This is a deterministic
engine-development trace. It exercises a representative prepared opening, not
a claim of full Oracle Magic support; any unsupported interaction encountered
by a longer policy run must surface through `EngineWeaknessRevealed`.

### Full-deck engine probes

`rav-deck-match` is the deck-level probe: it expands the two public 60-card
fixtures into libraries, deterministically shuffles, draws seven-card opening
hands, plays land/mana/stack/combat actions through `Game::submit_policy_move`,
and validates invariants after setup and every accepted move. The default seed
has a pinned public regression result: Selesnya wins on turn 30 after 759
accepted moves, life `[-2, 10]`, digest `fnv1a64:1335e960a109682d`.

`rav-deck-sweep` repeats the same full match for seeds `11`, `73`, `127`, and
`521`, and prints only genuine engine findings in its summary. Findings are
classified precisely:

- `EngineBug` means an engine invariant was violated.
- `CapabilityGap` means a policy explicitly submitted
  `ReportEngineWeakness` for an unsupported rule.

Invalid policy proposals and configured move/turn ceilings are recorded as
separate match outcomes; they are not mislabeled as engine defects. The current
four-seed sweep has zero engine findings. A combat/SBA invariant defect found
during development—dead blockers remained referenced by the combat assignment
after lethal damage—has a permanent regression test and is fixed.

`rav-engine-tournament` is the fail-closed broader probe. It runs seeds `0..16`
and exits nonzero for every invariant violation, explicit capability gap,
rejected policy move, or bounded incomplete run. A rules-valid simultaneous
loss is retained as a completed draw, not mislabeled as a failure. Its present
baseline has sixteen completed games and zero failures.

`rav-reference-deck-matrix` is the broader fail-closed campaign: every ordered
pair of shown decks is replayed across eight seeds. Each result records both
deck IDs, the seed, termination, and canonical digest, so a rejection or an
invariant failure is attributable to one exact matchup. Set
`RAV_MATRIX_OUTPUT_ROOT=PATH` to retain one canonical event log per game,
`event-log-manifest.tsv`, and `matrix-summary.txt`; set
`RAV_MATRIX_SEED_COUNT=N` for a smaller public review pass and
`RAV_MATRIX_SUMMARY_ONLY=1` to suppress trace printing. `rav-engine-audit`
adds public API adversarial probes and an interactive three-seed version of
that full matrix. The engine integration suite also covers rejected-action
atomicity, mana-boundary clearing, LIFO/countered stack paths, SBA fixed points,
continuous-effect lifetime, blocked-combat history, terminal draws, and
multiplayer survivor priority.

The latest eight-seed public review generated 1,680 complete logs with zero
engine/policy/capability failures. It deliberately includes real Dredge,
transmute, token-SBA, effect-expiry, and simultaneous-loss-draw traces; exact
counts and the review checks are recorded in
[`ENGINE_BUG_LEDGER.md`](ENGINE_BUG_LEDGER.md).

## Provenance and rights

See [DATA_PROVENANCE.md](DATA_PROVENANCE.md). This repository contains no
official card art, scan, flavor text, full Oracle-text database, credentials,
or hidden scenarios. CardBench-authored Rust and manifest structure are MIT;
Magic names and related marks remain the property of their respective owners.

## Expansion order

1. Ravnica: City of Guilds (`RAV`) — initial executable slice.
2. Guildpact (`GPT`) — formal manifest; replicate, haunt, and bloodthirst await
   executable card slices.
3. Dissension (`DIS`) — formal manifest; forecast, hellbent, and graft await
   executable card slices.
