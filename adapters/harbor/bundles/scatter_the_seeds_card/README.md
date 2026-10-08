# CardBench Magic single-card task: Scatter the Seeds

Implement the RAV Scatter the Seeds definition at collector #181 in
`varieties/magic/sets/ravnica_city_of_guilds/src/cards/scatter_the_seeds.rs`.
Keep its candidate-owned declarative `definition() -> CardDefinition` source
boundary.

The diagnostic plays a shuffled legal 60-card duel. The caster pays `{3}{G}{G}`
with three green mana and two distinct creatures tapped for generic Convoke.
The Instant creates three green 1/1 Saproling tokens. The authority checks
their live characteristics, attacks only with those tokens, and requires a
terminal win through their combat damage. It replays the same seed and rejects
compiled mutants for Convoke, cost, token count, and token body.

This diagnostic credits zero cards and reward; it does not claim coverage of
the 291-name / 306-printing RAV catalog or run a provider-backed trial.
