Implement the candidate-owned RAV Undercity Shade module at `varieties/magic/sets/ravnica_city_of_guilds/src/cards/undercity_shade.rs`.

Keep the module declarative. It owns the black `{4}{B}` 1/1 creature identity, Fear, and the `{B}` ability that gives itself +1/+1 until end of turn. The printed Shade subtype appears in public task metadata; the executable `CardDefinition` schema cannot assert it yet.

The isolated evaluator casts Undercity Shade in a constructed 60-card PvP game, pays and resolves the pump, verifies that a live green-white Watchwolf cannot block because it is neither black nor an artifact, and checks the rejected block leaves the event log unchanged. It also checks the temporary stats expire at cleanup, replay determinism, and terminal game state. Do not add flavor, art, scans, or copied card rules text.
