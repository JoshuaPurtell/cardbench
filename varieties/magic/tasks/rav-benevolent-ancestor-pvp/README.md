# Benevolent Ancestor RAV full-game task

Implement Benevolent Ancestor (#3) in `varieties/magic/sets/ravnica_city_of_guilds/src/cards/benevolent_ancestor.rs`. Keep the definition and its activated ability in this candidate-owned module. The definition must represent a white `{2}{W}` 0/4 creature with Defender. Its `{T}` ability targets a creature or player and creates one damage-prevention shield until end of turn.

The deterministic shuffled 60-card PvP trace casts Benevolent Ancestor, then waits for Elves of Deep Shadow to attack. It activates the ability targeting seat zero, verifies the source tapped, the shield was created, and the next one damage was prevented without being recorded as damage dealt. The game continues with Watchwolf combat to a seat-zero win.

The authority validates RAV's 291 unique names / 306 printings, the candidate-owned definition and binding, ordered projected events, deterministic replay, and gameplay mutations for shield amount, target, tap cost, and white casting cost. This diagnostic task awards no card credit or Harbor reward.
