# CardBench Magic single-card task: Privileged Position

Implement Privileged Position (RAV #251) in `varieties/magic/sets/ravnica_city_of_guilds/src/cards/privileged_position.rs`. Preserve the printed `{2}{G/W}{G/W}{G/W}` Enchantment identity and the RAV text: other permanents you control cannot be targeted by spells or abilities your opponents control.

The trusted source-aware full-game trace casts Privileged Position and Watchwolf, rejects the opponent's Putrefy attempt at that Watchwolf before payment or state mutation, and then allows the controller to cast Gather Courage at that same Watchwolf. It verifies the effect does not behave as Shroud, checks the completed seat-zero win and deterministic replay, and detects semantic and source mutations.

This is a reference-only diagnostic. It awards no card credit or Harbor reward.
