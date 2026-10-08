# Implement Golgari Rot Farm

Implement `varieties/magic/sets/ravnica_city_of_guilds/src/cards/golgari_rot_farm.rs` with candidate factories `definition()`, `land_entry_binding()`, `mana_ability()`, and `triggered_ability()`. Preserve RAV #278's tapped entry, controlled-land return trigger, and exact black-green mana bundle.

The reference duel must select and return a land controlled by the Rot Farm's controller, then tap the Rot Farm itself to add one black and one green mana together. Use the bundle to cast a creature, complete a terminal PvP win, and reproduce the same ordered event log on replay. Tapped-entry, target, trigger, and bundle mutations must fail. Diagnostic output grants no credit or reward.
