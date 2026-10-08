# Boros Swiftblade single-card RAV PvP trace

This task covers Boros Swiftblade (RAV collector #193): the red and white `{R}{W}` Creature, 1/2, with Double Strike. The authority casts it from a shuffled 60-card Boros Aggro deck and records one red and one white mana paid, cast, resolution, and battlefield entry. It validates the live 1/2 Double Strike characteristics and replays the same seed deterministically.

In a later unblocked combat, Swiftblade attacks the opponent in an ordinary two-player constructed game. The typed sequence records one damage to the defending player during First Strike combat damage and another during normal combat damage, in that order, before the match's terminal seat-zero win. The rules pin is Wizards Comprehensive Rules effective 2026-08-07. The result is diagnostic-only against 291 unique RAV names and 306 printings; it credits no set coverage or Harbor reward.

The candidate boundary is only `src/cards/boros_swiftblade.rs`, a literal declarative card definition. Materialization uses the shared fail-closed workspace helper. Harbor configuration binds the catalog, rules contract, engine and event projection, candidate materializer, scorer, task, and bundle files. No provider-backed candidate trial is part of this task.
