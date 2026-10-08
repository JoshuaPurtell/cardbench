# Implement Telling Time

Implement `varieties/magic/sets/ravnica_city_of_guilds/src/cards/telling_time.rs` as a literal-only module exporting `definition() -> CardDefinition`. Preserve RAV #69: Telling Time is a blue `{1}{U}` instant with `Effect::LookAtTopCardsPutOneInHandOneOnTopRestOnBottom { count: 3 }`.

The trusted authority runs a shuffled, legal 60-card PvP game. It verifies actual cast payment, the controller-private ordered top-three snapshot, the exhaustive choice of one card for hand, one for the library top, and one for the bottom, and the resulting zones/order. The opponent must not receive the private decision or any candidate identity. The duel continues to a seat-zero Watchwolf combat win and must replay deterministically. Keep the module declarative; do not add other files.
