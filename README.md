# CardBench

CardBench evaluates card games as engineered systems. It is a public benchmark
parallel to GameBench, with its own `cardbench` task identity and two execution
lanes:

- Harbor bundles live in this repository.
- Dock packages and benchmark registration live in
  [`JoshuaPurtell/evals`](https://github.com/JoshuaPurtell/evals).

The product focus is **code policies and deckbuilding**. Engine implementation,
ReAct play against pinned code-policy opponents, and cybernetic uplift are
separate families so play skill, deck quality, and engine fidelity are never
collapsed into one reward.

## Varieties and families

| Variety | Status | Expansion plan |
| --- | --- | --- |
| Pokémon TCG | Active | Crystal Guardians first; Dragon Frontiers next; Holon Phantoms remains a stub |
| Magic: The Gathering | Reserved | Formal block manifests, beginning with the original Ravnica block, then Return to Ravnica and Guilds of Ravnica |

## The code_policy evaluation surface

`code_policy` is scored over a fixed grid of **cells**, where one cell is
`candidate deck | opponent deck | opponent policy | seat | seed`. The candidate
and the reference baseline are swept over identical cells and compared pairwise,
so a lift must survive a paired bootstrap rather than a difference of two
blended win rates.

| Split | Decks | Opponents | Cells | Role |
| --- | --- | --- | --- | --- |
| train | 5 (published) | 5 (published) | 400 | agent feedback |
| heldout | 4 (sealed) | 8 (sealed) | 384 | **authority** |

`varieties/pokemon/rosters/code_policy_v1.json` is the contract: it names the
whole train surface and publishes only the *shape* of the heldout split plus a
sha256 pinning it. The heldout decks are authored from the RS/SS card pool,
which the train decks never touch, so no heldout deck shares a non-Energy card
with anything the agent can see. Sealed assets live under
`varieties/pokemon/.sealed/` — gitignored, and excluded from both the Harbor
workspace copy and the Dock workspace upload.

| Family | Submission | v0 status |
| --- | --- | --- |
| `code_policy` | Rust policy | P0 runnable, train/heldout split |
| `deck_opt` | Decklist JSON | P0 runnable |
| `card` | One expansion card module | P1 reference verify runnable |
| `set_engine` | Whole set plus expansion-specific engine hooks | P1 public-train reference runnable |
| `engine` | Shared/core engine substrate | P1 compile substrate present |
| `full_engine` | Standalone engine service | P1 reserved |
| `react` | Model actions through a ReAct loop | P2 parser/renderer lifted |
| `cybernetic` | Budgeted hybrid policy | P2 scaffold |

## Quick start

```bash
# Score the reference policy on the sealed heldout split (writes a leaderboard
# with a baseline row, a heldout scorecard, and a train sweep for diagnosis)
./adapters/harbor/run.sh code-policy verify pokemon

# Choose-from-pool deck evaluation under a frozen policy
./adapters/harbor/run.sh deck-opt verify pokemon

# Validate the consolidated Pokémon engine workspace
./adapters/harbor/run.sh engine verify pokemon

# Verify one legacy single-card task against sealed evaluator assets
CARDBENCH_SEALED_ROOT=/trusted/cardbench-sealed \
  ./adapters/harbor/run.sh card verify pokemon --instance df-097-rayquaza-ex

# Verify the Crystal Guardians set + set-engine overlay on shown scenarios
./adapters/harbor/run.sh set-engine verify pokemon \
  --expansion crystal_guardians --variant 0pct --suite train

# List the family/variety surface
./adapters/harbor/run.sh code-policy list pokemon
```

Each P0 run writes authority and visualization artifacts beneath
`artifacts/`. Pixels are derived from event/state data and are never used as
the score.

## Layout

- `varieties/pokemon/engine/` — pinned shared Rust engine
- `varieties/pokemon/cards/` — single-card specs, public stubs, and sealed hashes
- `varieties/pokemon/sets/` — expansion manifests and legacy public reference kits; fresh heldout authority stays sealed
- `varieties/pokemon/policies/` — stable policy ABI and reference policies
- `varieties/pokemon/decks/` — shown deck pool
- `varieties/pokemon/react/` — lifted view renderer and action parser
- `varieties/pokemon/viz/` — deterministic low-fi board renderer
- `varieties/magic/` — Magic/Ravnica architecture and expansion manifest schema
- `adapters/harbor/` — unified Harbor entry point and family bundles

The authoritative implementation handoff is `evals/tcg.md`.
