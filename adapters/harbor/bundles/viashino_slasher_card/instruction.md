# Implement Viashino Slasher

Implement `varieties/magic/sets/ravnica_city_of_guilds/src/cards/viashino_slasher.rs` as a literal-only module with `definition()` and `pump_ability()`. Preserve RAV #149 identity, `{1}{R}` red 1/2 creature characteristics, and the activated ability `{R}, discard a card: Viashino Slasher gets +1/-1 until end of turn`.

The shuffled PvP reference pays the creature cost, then pays red and discards a real card for the stack-backed ability. The report must verify a live 2/1, cleanup return to 1/2, repeated 2-damage Slasher attacks, ordered event receipts, deterministic replay, and a terminal seat-zero win. A +0/-1 source mutation must fail its live-characteristics gate.
