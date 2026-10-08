# Implement Coalhauler Swine

Implement `varieties/magic/sets/ravnica_city_of_guilds/src/cards/coalhauler_swine.rs` as a literal-only module with `definition()` and `triggered_ability()`.

Preserve RAV #119 identity, red `{4}{R}{R}` cost, 4/4 body, and the mandatory `ReceivesDamage` trigger whose effect uses the received-damage amount to deal damage to each player. Do not replace the captured amount with a fixed amount or add targets.

The trusted authority uses a shuffled two-player 60-card duel. Viashino Fangtail deals one damage to the Swine; the trace checks exact trigger provenance and damage receipts to both players, both life totals at 19 after resolution, deterministic replay, and a terminal seat-zero win. The diagnostic scorer keeps rewards and credited cards at zero.
