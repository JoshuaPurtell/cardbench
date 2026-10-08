# Dark Heart of the Wood card task

Implement the candidate-owned definition, activated ability, and typed cost in `varieties/magic/sets/ravnica_city_of_guilds/src/cards/dark_heart_of_the_wood.rs`. Preserve RAV #200's black-green `{B}{G}` Enchantment identity and the activated ability: pay `{G}`, sacrifice a Forest as a cost, then gain 3 life on resolution.

The trusted authority uses shuffled legal 60-card decks in a two-player game. It casts Dark Heart of the Wood and Watchwolf, allows a combat hit to make the life-gain observable, then activates the Heart with a separate Forest sacrificed. It verifies the exact green payment, typed Forest sacrifice and graveyard move before the ability is stacked, 3 life gained only on resolution, deterministic replay, semantic mutations, and a terminal seat-zero win.

This is a local diagnostic task. It awards no cards or reward and does not run Harbor/provider trials during reference qualification.
