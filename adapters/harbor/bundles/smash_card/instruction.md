# Implement Smash

Implement `varieties/magic/sets/ravnica_city_of_guilds/src/cards/smash.rs` as a literal-only module exporting `definition() -> CardDefinition`. Preserve the RAV #143 identity, red `{2}{R}` instant characteristics, and the ordered effect list: destroy one target artifact, then draw one card.

The trusted authority runs an ordinary shuffled two-player game. It resolves an opponent Boros Signet, pays three red mana to cast Smash at that exact artifact, validates the destruction and graveyard receipts, observes a different library card entering the pilot's hand after destruction, and continues to a seat-zero win through Viashino Slasher combat. Source syntax, identity and effect order, local mutation, materializer integrity, and the reference trace are checked. No card or set reward is awarded.
