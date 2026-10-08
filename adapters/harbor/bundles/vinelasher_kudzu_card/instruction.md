Implement `varieties/magic/sets/ravnica_city_of_guilds/src/cards/vinelasher_kudzu.rs` as a declarative candidate module exposing `definition()` and `triggered_ability()`.

Required RAV behavior:

- Vinelasher Kudzu is RAV #189, a green `{1}{G}` 1/1 Creature — Plant.
- Whenever a land enters under its controller's control, its mandatory trigger puts one +1/+1 counter on that same Vinelasher Kudzu.
- An opponent's land entry does not trigger the ability.

The trace uses ordinary shuffled 60-card decks, requires a complete PvP terminal result, records cast and trigger event order and source incarnation, and replays deterministically. Keep the printed Plant subtype in mind; the current `CardDefinition` model cannot represent creature subtypes, and the task metadata records that limitation.

The source guard runs before candidate calls. Preserve the candidate module API and do not edit other files.
