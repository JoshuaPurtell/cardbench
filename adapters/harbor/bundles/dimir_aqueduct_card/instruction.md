# Implement Dimir Aqueduct

Implement `varieties/magic/sets/ravnica_city_of_guilds/src/cards/dimir_aqueduct.rs` with candidate factories `definition()`, `land_entry_binding()`, `mana_ability()`, and `triggered_ability()`. Preserve RAV #276's tapped entry, controlled-land return trigger, and exact blue-black mana bundle.

The reference duel must select and return a land controlled by the Aqueduct's controller, then tap the Aqueduct itself to add one blue and one black mana together. Use the bundle to cast a creature, complete a terminal PvP win, and reproduce the same ordered event log on replay. Tapped-entry, target, trigger, and bundle mutations must fail. Diagnostic output grants no credit or reward.
