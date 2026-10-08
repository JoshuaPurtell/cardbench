# Implement Mark of Eviction

Implement `varieties/magic/sets/ravnica_city_of_guilds/src/cards/mark_of_eviction.rs` with literal `definition()` and `triggered_ability()` factories. Preserve RAV #58 identity, blue `{U}` enchantment Aura identity, creature target and attachment, and the Aura-controller-upkeep ability that returns its enchanted creature and every Aura attached to that creature to their respective owners' hands.

The reference attaches Mark to the opponent's War-Torch Goblin. The opponent's upkeep passes without a trigger. At Mark's controller's later upkeep, both the Goblin and Mark return to their owners' hands, with an owner-view receipt for each. The duel must reach a terminal seat-zero win and replay identically. Semantic gameplay mutations must be detected.
