# Duskmantle, House of Shadow RAV full-game task

Implement Duskmantle, House of Shadow (#277) in `varieties/magic/sets/ravnica_city_of_guilds/src/cards/duskmantle_house_of_shadow.rs`. Keep its definition and activated-ability binding in this candidate-owned module. It is a colorless nonbasic land with “{T}: Target player mills a card.”

The deterministic shuffled 60-card PvP trace plays Duskmantle, activates its stack-using ability targeting the opponent, and verifies that exactly one exact object moves from that opponent's library to their graveyard while Duskmantle pays its tap cost and remains on the battlefield. Watchwolf then attacks repeatedly to finish the full game.

The authority checks the literal candidate-module boundary, registry delegation for both identity and binding, deterministic replay, exact target and zone provenance, event order, and gameplay mutants for a two-card mill, a missing tap cost, self-mill, and a missing player target. This is a diagnostic task; it awards no card credit or Harbor reward.

Only the target module is editable in the public candidate workspace. See `features.toml` for the behavior and source contract.
