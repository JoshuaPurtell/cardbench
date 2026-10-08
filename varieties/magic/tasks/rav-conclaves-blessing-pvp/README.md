# Conclave's Blessing full-game card task

Implement the candidate-owned definition in `varieties/magic/sets/ravnica_city_of_guilds/src/cards/conclaves_blessing.rs`. Conclave's Blessing is a white `{3}{W}` Enchantment Aura with Convoke that enchants a creature. The enchanted creature gets +0/+2 for each other creature controlled by Conclave's Blessing's controller.

The isolated authority casts the Aura on an opponent's creature in a shuffled 60-card two-player game, pays three generic mana with three distinct creatures through Convoke and the remaining white symbol with mana, verifies controller-relative toughness, then continues through combat to a terminal P0 win. It also checks deterministic replay and gameplay mutations for the modifier, counted player's identity, attachment, cost, and Convoke. The diagnostic covers one identity among 291 RAV names and 306 printings and awards no credit or reward.
