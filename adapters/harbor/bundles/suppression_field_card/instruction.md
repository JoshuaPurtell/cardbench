# Implement Suppression Field

Implement `varieties/magic/sets/ravnica_city_of_guilds/src/cards/suppression_field.rs` as a declarative module with `definition()` and `cost_modifier_binding()`. Preserve RAV #31 as a white `{1}{W}` enchantment. Its live cost modifier must add exactly `{2}` to nonmana activated abilities and leave mana abilities unaffected.

The isolated reference authority casts Suppression Field in a shuffled two-player constructed game. Seat one casts Dimir Guildmage and activates its `{3}{U}` target-player-draw ability while the Field remains on the battlefield. The trace must capture a cost calculation sourced by Suppression Field, payment of `{5}{U}`, the targeted card draw, a post-resolution Island mana ability, replay, and a seat-zero terminal PvP win. The report credits no RAV coverage or Harbor reward.
