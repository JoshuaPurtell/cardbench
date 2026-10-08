# Implement Woebringer Demon

Implement `varieties/magic/sets/ravnica_city_of_guilds/src/cards/woebringer_demon.rs` with literal candidate factories named `definition()` and `triggered_ability()`. Preserve RAV #113's black `{3}{B}{B}` 4/4 creature with Flying and its mandatory beginning-of-each-upkeep creature sacrifice.

The active player for each upkeep must receive the choice and sacrifice one creature they control if able. The full reference duel checks both player seats, exact sacrifice and graveyard move receipts, trigger resolution order, exact casting payment, Flying combat, a terminal seat-zero win, and deterministic replay. Cost, stats, Flying, and trigger-removal mutants must be detected. Diagnostic reports award no cards and no reward.
