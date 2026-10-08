# Implement Sell-Sword Brute

Implement `varieties/magic/sets/ravnica_city_of_guilds/src/cards/sell_sword_brute.rs` as a literal-only module with candidate-owned `definition()` and `triggered_ability()` functions. Preserve the RAV #142 identity, `{1}{R}` red 2/2 creature characteristics, and its mandatory untargeted dies trigger that deals two damage to its controller.

The shuffled PvP reference casts Sell-Sword Brute for `{1}{R}`. The opponent pays `{1}{B}` for Last Gasp and resolves it against the Brute, causing the toughness-zero state action and graveyard move. The Brute's trigger then resolves from the same source incarnation and deals two damage to its controller. The game continues to a terminal pilot win. Preserve deterministic replay; a gameplay mutation from two damage to one must fail the trusted trace oracle.
