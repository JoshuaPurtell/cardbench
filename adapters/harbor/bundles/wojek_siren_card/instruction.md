# Implement Wojek Siren

Implement `varieties/magic/sets/ravnica_city_of_guilds/src/cards/wojek_siren.rs` as a literal-only module exporting `definition() -> CardDefinition`. Preserve RAV #37's white `{W}` instant identity and the single `RadianceModifyPtUntilEndOfTurn { power: 1, toughness: 1 }` effect.

The authority runs an ordinary shuffled two-player match. It pays one white mana, announces Watchwolf on the live stack, applies the temporary +1/+1 to the target and every other creature that shares a color, leaves an opposing red Goblin Spelunkers unchanged, and continues through pumped combat to a seat-zero win. Source syntax, identity, effect semantics, deterministic replay, mutations, and materialization integrity are checked. No card or set reward is awarded.
