Implement the candidate-owned Autochthon Wurm definition in `varieties/magic/sets/ravnica_city_of_guilds/src/cards/autochthon_wurm.rs`.

Autochthon Wurm is RAV #191, a green-white 9/14 creature costing `{10}{G}{G}{G}{W}{W}` with Convoke and Trample. Preserve its printed identity, exact mana cost, creature type, power, toughness, and keywords.

The isolated authority runs a shuffled full-game duel from normal 60-card RAV decks. It casts Autochthon Wurm after tapping nine creatures for Convoke (two green, two white, and five generic) and paying six mana from the pool. The opponent casts Birds of Paradise and Caregiver and declares both as blockers. The attacking player orders them; the Wurm assigns lethal damage to both and seven excess Trample damage to the opponent. The duel then reaches a terminal pilot win. The authority checks the typed event sequence, post-combat state, deterministic replay, and gameplay mutations.

Keep the candidate module declarative and limited to the printed card definition. Do not add rules prose, art, scans, or flavor text.
