# Carven Caryatid full-game card task

Implement Carven Caryatid in `varieties/magic/sets/ravnica_city_of_guilds/src/cards/carven_caryatid.rs` by returning its declarative definition and candidate-owned ETB trigger binding.

The authority runs a shuffled 60-card duel. It pays `{1}{G}{G}`, resolves the green 2/5 Defender, and verifies that the mandatory ETB trigger moves a card from the controller's library into their hand. It attempts a Defender attack in a separate rejected-action probe and checks the engine leaves the match unchanged. The legal game then attacks with Watchwolf and reaches a P0 win.

The task checks exact payment, cast, resolution, entry, trigger, draw movement, attack legality, combat, terminal events, live characteristics, deterministic replay, and compiled semantic controls for the trigger, Defender, and toughness. It reports one of 291 unique RAV names and 306 printings, with zero card credit and zero reward.
