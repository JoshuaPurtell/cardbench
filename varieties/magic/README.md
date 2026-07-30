# Magic variety

Magic is a sibling CardBench variety, not a mode of the Pokémon engine. This
directory is a Rust workspace whose reference engine owns Magic-specific rules;
sets supply small executable card definitions and declarative block manifests.

## Runnable surface

```bash
cd varieties/magic
cargo test --workspace
cargo run -p cardbench-magic-rav --bin rav-engine-parity
```

The last command validates the original Ravnica-block manifests and public deck
pool, executes eleven RAV scenarios twice, and compares each deterministic event
log against its fixed public digest. With `--output-root PATH`, the Rust binary
writes `engine-check.json` and `reward.txt` for the CardBench Harbor receipt.

| Component | Location | Scope in this milestone |
| --- | --- | --- |
| Expansion-neutral rules | `engine/` | Library, hand, battlefield, graveyard, exile, stack; priority; phases/steps; mana costs; lands; state-based actions; replacement effects; continuous-effect layers 4–7 |
| RAV fixture crate | `sets/ravnica_city_of_guilds/` | Stack, convoke, dredge, transmute, radiance, tokens, and layer/SBA scenarios |
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

## Current card-slice contract

The RAV crate implements only the semantic fragments stated by each card's
`supported_rules` field. For example, `Muddle the Mixture` contributes the
transmute case, not an assertion that every printed ability on that card is
available. This makes the initial expansion slice honest and lets later set
work add executable semantics without changing engine ownership.

RAV shown scenarios are fixture-driven from
`sets/ravnica_city_of_guilds/scenarios/public/train_scenarios.toml`; each declares
setup, actions, state assertions, event markers, and a fixed digest. They cover:

- casting to the stack and both-player priority passes (`Lightning Helix`);
- colored and generic convoke payment plus token creation (`Scatter the Seeds`);
- dredge as a draw replacement (`Golgari Brownscale`);
- transmute, equal mana-value search, and seeded deterministic shuffle (`Muddle the Mixture`);
- radiance color matching and layer-7 modifiers (`Rally the Righteous`); and
- zero-toughness state-based action after a continuous effect (`Last Gasp`).
- cleanup expiration, land-play limits, and rejected priority/convoke/dredge actions.

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
