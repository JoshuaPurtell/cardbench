# Seeds of Strength full-game card task

Implement the candidate-owned RAV Seeds of Strength definition in `varieties/magic/sets/ravnica_city_of_guilds/src/cards/seeds_of_strength.rs`. Seeds of Strength is a green-white `{G}{W}` Instant that gives up to three target creatures +1/+1 until end of turn.

The diagnostic authority casts the spell on three distinct objects: one Watchwolf and two Elves of Deep Shadow. It checks exact `{G}{W}` payment and spell identity, the three independently sourced power/toughness effects, each creature's base characteristics before the spell, its boosted characteristics while the effects apply, and restoration after cleanup. The same three creatures attack after expiration and deal damage equal to their restored power. The shuffled 60-card duel must continue to a terminal win and replay deterministically. Gameplay mutations change the modifier, remove the third modifier, or change the cost.

This is a single-card diagnostic pilot. It reports one identity among 291 RAV names and 306 printings and grants zero card credit or reward. Materialize with `materialize_seeds_of_strength_task.py --workspace PATH --manifest PATH`; verify with the same command plus `--verify`.
