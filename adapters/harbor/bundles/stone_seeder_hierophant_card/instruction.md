Implement the Stone-Seeder Hierophant candidate module for original Ravnica: City of Guilds printing #184.

Required interface: `definition()`, `activated_ability()`, and `triggered_ability()`.

Required behavior: green `{2}{G}{G}` 1/1 creature identity; `{T}: Untap target land`; and a mandatory trigger when a land enters under your control that untaps Stone-Seeder Hierophant. An opponent's land entry does not trigger the ability. The card has no mana ability. Keep the Human and Druid subtype facts in mind; executable `CardDefinition` state cannot currently assert creature subtypes.

Only the candidate module may change. The diagnostic trace, scorer, and materializer are trusted task infrastructure.
