# Magic variety

Magic is a sibling CardBench variety, not a mode of the Pokémon engine. Its
engine must model stack/priority, phases and steps, replacement effects,
continuous effects/layers, and Magic-specific zones and deck construction.

## Planned expansion sequence

1. **Ravnica: City of Guilds block** — Ravnica: City of Guilds, Guildpact,
   Dissension.
2. **Return to Ravnica block** — Return to Ravnica, Gatecrash, Dragon's Maze.
3. **Guilds era** — Guilds of Ravnica, Ravnica Allegiance, War of the Spark.

The first implementation should use small, licensed-as-data fixtures or
original scenario descriptions and formal expansion manifests. Do not bundle
official card art.

## Contract

Every expansion directory will provide an `expansion.toml`, legality rules,
shown and held-out scenario manifests, deck pools, and a deterministic engine
pin. Families keep the same public ids as Pokémon:

```text
cardbench/magic/code_policy
cardbench/magic/deck_opt
cardbench/magic/engine
cardbench/magic/react
cardbench/magic/cybernetic
```

See `expansion.schema.toml` for the formal manifest shape.
