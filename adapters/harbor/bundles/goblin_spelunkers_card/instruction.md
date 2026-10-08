# CardBench Magic single-card task: Goblin Spelunkers

Implement the RAV Goblin Spelunkers definition in
`varieties/magic/sets/ravnica_city_of_guilds/src/cards/goblin_spelunkers.rs`.
The card is collector #128, a red 2/2 creature with mana cost `{2}{R}` and
Mountainwalk.

The trusted verifier plays a seeded, legal 60-card PvP match. It pays red plus
two generic mana, resolves Goblin Spelunkers, and stages a real opposing
creature's block attempt while that defender controls a Mountain. The engine
must reject the illegal block because of Mountainwalk, without changing the
event log. The defender then declares no blockers and the game continues to a
terminal result. The authority checks the complete ordered trace, live card
characteristics, deterministic replay, removal mutation, and compile-valid
stub rejection.

Keep the source as a pure declarative definition. Do not include copied card
rules prose, artwork, or flavor text. This task diagnoses one card among all
291 RAV identities and 306 printings; it does not award whole-set coverage or
Harbor reward.
