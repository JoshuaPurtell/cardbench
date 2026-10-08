# Implement Blockbuster

Implement `varieties/magic/sets/ravnica_city_of_guilds/src/cards/blockbuster.rs` with literal-only `definition()` and `activated_ability()` functions. Preserve the RAV #115 card identity and `{3}{R}{R}` red enchantment. Its `{1}{R}` targetless activated ability sacrifices Blockbuster as a cost; the ability then deals 3 damage to each tapped creature and each player.

The full-game trace requires source sacrifice and its graveyard move before the ability is stacked. It observes which creatures are tapped immediately before resolution, verifies damage to exactly those creatures and both players, leaves untapped creatures undamaged, then records any resulting state-based deaths and a terminal game. The report is diagnostic-only, with zero credit and reward.

The RAV text controls. Current Oracle substitutes “this enchantment” for the self-name but does not change the ability. See the task source note for the sources and the legacy catalog label correction.
