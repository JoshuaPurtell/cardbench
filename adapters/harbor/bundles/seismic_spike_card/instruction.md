# Implement Seismic Spike

Implement `varieties/magic/sets/ravnica_city_of_guilds/src/cards/seismic_spike.rs` as a literal-only module exporting `definition() -> CardDefinition`. Preserve RAV #141 identity, red `{2}{R}{R}` sorcery characteristics, and the ordered effect list: destroy a target land, then add two red mana to the controller.

The trusted authority runs an ordinary shuffled two-player game. It pays four red mana to cast Seismic Spike at an opposing Mountain, validates the exact destruction and graveyard receipts, observes two red mana added after the destruction and before the spell finishes resolving, then spends that mana on Viashino Slasher. The game continues through a seat-zero combat win. Source syntax, identity and effect order, local mutation, materializer integrity, and the reference trace are checked. No card or set reward is awarded.
