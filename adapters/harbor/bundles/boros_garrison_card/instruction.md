# Implement Boros Garrison

Implement `varieties/magic/sets/ravnica_city_of_guilds/src/cards/boros_garrison.rs`
with literal candidate factories `definition()`, `land_entry_binding()`,
`mana_ability()`, and `triggered_ability()`. Preserve the RAV #275 land identity,
tapped entry, mandatory trigger that returns a land you control to its owner's
hand, and exact `{R}{W}` tap-mana bundle.

The shuffled reference duel must select and return a controlled basic land,
then tap Boros Garrison itself to add one red and one white mana together. Spend
the bundle and generic mana to cast Skyknight Legionnaire, attack, and finish a
terminal PvP win. The authority checks the ordered state/event facts and full
replay determinism. Tapped-entry, target-restriction, trigger, and bundle
mutations must be detected. The task awards no card credit or reward.

Keep the module declarative and do not add or modify candidate files outside
this card module.
