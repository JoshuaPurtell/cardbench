Implement the candidate-owned Grayscaled Gharial definition in `varieties/magic/sets/ravnica_city_of_guilds/src/cards/grayscaled_gharial.rs`.

Grayscaled Gharial is RAV #52, a blue 1/1 creature costing `{U}` with Islandwalk.

The isolated authority runs a shuffled full-game duel from ordinary 60-card decks. The pilot attacks while the opponent controls an Island and a live Drift of Phantasms blocker. The authority attempts an otherwise legal block, verifies Islandwalk rejects it without changing state, then checks the empty block declaration, unblocked combat damage, replay, a terminal win, and gameplay mutations.

Keep the candidate module declarative and limited to the card definition. Do not add rules prose, art, scans, or flavor text.
