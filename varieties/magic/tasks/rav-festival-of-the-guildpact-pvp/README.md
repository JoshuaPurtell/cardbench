# Festival of the Guildpact full-game card task

Implement the candidate-owned RAV Festival of the Guildpact module in `varieties/magic/sets/ravnica_city_of_guilds/src/cards/festival_of_the_guildpact.rs` as a white `{X}{W}` Instant.

The diagnostic authority chooses X=4 and pays one white plus four generic mana. It verifies that Festival creates a four-damage shield for its controller, an opponent's Char resolves while all four damage to that controller is prevented, and Festival's draw instruction moves a card to the controller's hand. The shuffled 60-card duel continues through combat to a deterministic seat-zero win; gameplay mutations check chosen X, prevention, and drawing.

This is a single-card diagnostic for RAV collector number 17. It reports one passed identity among 291 unique names and 306 printings and awards no set credit or Harbor reward.
