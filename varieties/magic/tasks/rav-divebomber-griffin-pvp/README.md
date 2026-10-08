# Divebomber Griffin full-game card task

Implement the candidate-owned Divebomber Griffin definition and activated ability in `varieties/magic/sets/ravnica_city_of_guilds/src/cards/divebomber_griffin.rs`. It is a white `{3}{W}{W}` 3/2 creature with flying. Its zero-mana, non-tap activated ability sacrifices Divebomber Griffin as a cost and deals 3 damage to target attacking or blocking creature.

The trusted authority runs a shuffled 60-card two-player game. It casts Divebomber Griffin, waits until the opponent attacks, activates the ability while the creature is attacking, checks that the Griffin is sacrificed before resolution, verifies the exact target and three damage, confirms lethal cleanup, and plays through to a terminal P0 combat win. The trace is replayed deterministically and gameplay mutations cover Flying, ability presence, target restriction, and damage amount.

This is a diagnostic task over 291 distinct RAV names and 306 printings. It awards zero card credit and zero reward.
