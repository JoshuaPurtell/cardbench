# CardBench Magic single-card task: Carven Caryatid

Implement Carven Caryatid in `varieties/magic/sets/ravnica_city_of_guilds/src/cards/carven_caryatid.rs` by providing `definition()` and `triggered_ability()`.

Carven Caryatid is a green 2/5 Creature with mana cost `{1}{G}{G}` and Defender. Its mandatory enter-the-battlefield ability draws one card for its controller.

The trusted authority runs a shuffled 60-card full game. It checks exact spell payment, cast, resolution, battlefield entry, trigger, and draw movement. It separately submits an illegal attack declaration for Carven Caryatid and verifies the engine rejects it without changing the match. The accepted full-game trace uses Watchwolf for combat and reaches a P0 win. The authority checks deterministic replay and compiled mutations to the trigger, Defender, and toughness. The task reports one of 291 unique RAV names and 306 printings, with zero credit and zero reward.
