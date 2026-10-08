# Wojek Embermage RAV full-game task

Implement Wojek Embermage (#152) in `varieties/magic/sets/ravnica_city_of_guilds/src/cards/wojek_embermage.rs`. Keep the `{3}{R}` red 1/2 creature definition and `{T}: Wojek Embermage deals 1 damage to target creature and each other creature that shares a color with it` binding in that module.

The deterministic shuffled PvP trace casts Wojek Embermage and two Watchwolves. Its tap ability targets the opponent's Watchwolf; resolution deals exactly 1 damage to that target and the friendly Watchwolf because they share colors, while the red source is excluded. The game then continues to a full-game win.

Read the task README and `features.toml` in the trusted handoff. The public workspace exposes only the target module; task authority, scorer, materializer, and specifications are not available to the candidate. Do not add other files.
