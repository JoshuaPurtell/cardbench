# Stone-Seeder Hierophant RAV card task

Implement Stone-Seeder Hierophant (RAV #184) in `varieties/magic/sets/ravnica_city_of_guilds/src/cards/stone_seeder_hierophant.rs` with `definition()`, `activated_ability()`, and `triggered_ability()`.

The printed card is a green `{2}{G}{G}` 1/1 creature with two abilities: its controller may tap it to untap a target land, and whenever a land enters under its controller's control it untaps itself. It has no mana ability. The task records the printed Human and Druid subtypes in public metadata because the current `CardDefinition` model cannot assert creature subtypes.

The full-game authority uses legal shuffled 60-card decks. It must cast the card for the exact cost, activate and resolve the target-land ability, play a land and resolve exactly the controller-scoped untap trigger, ignore an opponent's land entry, and later complete a deterministic PvP win. The report includes semantic mutations and source-shape rejection before candidate functions execute. It credits zero cards and awards zero reward.
