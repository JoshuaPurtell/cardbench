# Implement Selesnya Sanctuary

Implement `varieties/magic/sets/ravnica_city_of_guilds/src/cards/selesnya_sanctuary.rs` with candidate factories `definition()`, `land_entry_binding()`, `mana_ability()`, and `triggered_ability()`. Preserve RAV #281's tapped entry, controlled-land return trigger, and exact green-white mana bundle.

The reference duel must select and return a land controlled by the Sanctuary's controller, then tap the Sanctuary itself to add one green and one white mana together. Use the bundle to cast a creature, complete a terminal PvP win, and reproduce the same ordered event log on replay. Tapped-entry, target, trigger, and bundle mutations must fail. Diagnostic output grants no credit or reward.
