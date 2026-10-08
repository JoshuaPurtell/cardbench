# CardBench Magic single-card task: Selesnya Evangel

Implement the RAV Selesnya Evangel definition and its task-local activated
ability in
`varieties/magic/sets/ravnica_city_of_guilds/src/cards/selesnya_evangel.rs`.
The card is collector #228, a green-white 1/2 creature.

The trusted verifier plays a seeded legal 60-card PvP match, pays one green and
one white mana to cast the creature, then activates its ability after it can
legally tap. Activation costs one generic mana, taps Selesnya Evangel, and taps
one distinct creature you control. The ability must enter and resolve from the
stack, create a green 1/1 Saproling creature, and leave that token live on the
battlefield. Ordered event receipts, token identity and characteristics,
terminal game state, and deterministic replay are checked.

Keep the source as a pure declarative definition and ability literal. Do not
include copied card rules prose, artwork, or flavor text. The task is a one-card
diagnostic over all 291 RAV identities and 306 printings; it does not award
whole-set coverage or Harbor reward.
