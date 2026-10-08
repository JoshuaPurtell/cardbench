# Boros Garrison RAV card task

Implement RAV #275 in the candidate-owned module at
`varieties/magic/sets/ravnica_city_of_guilds/src/cards/boros_garrison.rs`.
The module exports four literal factories: `definition()`,
`land_entry_binding()`, `mana_ability()`, and `triggered_ability()`.

The shuffled two-player reference duel verifies that Boros Garrison enters
tapped, its mandatory entry trigger selects and returns a land its controller
owns to that card owner's hand, and its own tap ability produces exactly one
red and one white mana together. It spends those colors plus generic mana to
cast Skyknight Legionnaire, attacks, and continues to a terminal seat-zero win.
The authority checks the ordered projected events, captures the returned card's
hand zone at the return event, and compares the complete event log on replay.

Gameplay mutations cover untapped entry, a changed target restriction, a
missing return trigger, and a changed mana bundle. This is an uncredited local
diagnostic: no card credit or reward is issued.
