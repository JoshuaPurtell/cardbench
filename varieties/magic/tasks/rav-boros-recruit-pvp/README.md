# Boros Recruit single-card RAV PvP trace

This task covers Boros Recruit (RAV collector #243): the red/white hybrid `{R/W}` creature, 1/1, with First Strike. The authority runs two deterministic full-game traces from shuffled 60-card Boros Aggro decks, explicitly paying the hybrid symbol once with Red and once with White, and records each cast, resolution, and battlefield entry. Each trace validates the live 1/1 First Strike characteristics and replays its seed deterministically.

In a later combat, Boros Recruit attacks and Elves of Deep Shadow from the Golgari deck blocks it. The typed event sequence records the First Strike damage step, Recruit dealing one damage, the 1/1 blocker dying and moving to its graveyard before the normal combat damage step, and post-combat state showing Recruit tapped but undamaged. The rules pin is Wizards Comprehensive Rules effective 2026-08-07. The result is diagnostic-only against 291 unique RAV names and 306 printings; it credits no set coverage or Harbor reward.

The candidate boundary is only `src/cards/boros_recruit.rs`, a literal declarative card definition. Materialization uses the shared fail-closed workspace helper. Harbor configuration binds the catalog, rules contract, engine and event projection, candidate materializer, scorer, task, and bundle files. No provider-backed candidate trial is part of this task.
