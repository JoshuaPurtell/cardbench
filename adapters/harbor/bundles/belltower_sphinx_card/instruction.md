# Implement Belltower Sphinx

Implement `varieties/magic/sets/ravnica_city_of_guilds/src/cards/belltower_sphinx.rs` as a declarative module with `definition()` and `triggered_ability()`. Preserve the RAV #38 blue `{4}{U}` 2/5 creature with Flying. Its mandatory, non-targeted `ReceivesDamage` trigger must mill the amount of damage from the controller of that damage source.

The isolated reference authority casts Belltower Sphinx in a shuffled two-player constructed game. Viashino Fangtail deals one damage to it; the Sphinx trigger stacks and resolves and moves exactly one card from seat one's library to seat one's graveyard. The game then continues to a seat-zero terminal win, and a second execution with the same seed must replay the same full event log. The report credits no RAV coverage or Harbor reward.
