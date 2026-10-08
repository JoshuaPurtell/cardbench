# Farseek full-game card task

Implement the candidate-owned RAV Farseek module in `varieties/magic/sets/ravnica_city_of_guilds/src/cards/farseek.rs` as the printed green `{1}{G}` Sorcery and typed library search.

The diagnostic authority casts Farseek in a shuffled 60-card duel, privately selects a Plains from a library containing all four legal non-Forest basic land types, and verifies that the Plains enters the battlefield tapped without a reveal event. The library shuffles, and the full game continues through combat to a deterministic seat-zero win. Gameplay mutations check Forest exclusion, tapped entry, and the no-reveal selection contract.

This is a single-card diagnostic for RAV collector number 163. It reports one passed identity among 291 unique names and 306 printings; it awards no set credit or Harbor reward.
