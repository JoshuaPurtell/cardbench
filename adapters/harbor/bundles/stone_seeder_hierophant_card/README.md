# Stone-Seeder Hierophant card task

Implement the candidate module at `varieties/magic/sets/ravnica_city_of_guilds/src/cards/stone_seeder_hierophant.rs`. The public workspace permits edits to that module only.

Preserve RAV #184's green `{2}{G}{G}` 1/1 Creature — Human Druid identity, the targetless controller-land-entry trigger that untaps Stone-Seeder Hierophant, and its `{T}` ability targeting a land to untap it. Human and Druid are retained in task metadata because the engine definition currently cannot assert creature subtypes. Do not add a mana ability.

The trusted diagnostic authority checks a legal shuffled 60-card full-game trace, source and target event ordering, opponent land-entry exclusion, deterministic replay, semantic mutation sensitivity, and pre-call source-shape rejection. The task awards no card credit or reward.
