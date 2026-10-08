Implement the candidate-owned Siege Wurm definition in `varieties/magic/sets/ravnica_city_of_guilds/src/cards/siege_wurm.rs`.

Siege Wurm is RAV #183, a green 5/5 creature costing `{5}{G}{G}` with Convoke and Trample. Preserve its printed identity, exact mana cost, creature type, power, toughness, and both keywords.

The isolated authority plays a shuffled, full-game duel from normal 60-card RAV decks. The pilot casts real RAV creatures and Scatter the Seeds before casting Siege Wurm with seven distinct creatures for Convoke: two pay green and five pay generic. The opponent casts and blocks with Watchwolf. The authority checks the Wurm's characteristics, its Convoke receipts, 3 damage to Watchwolf, 2 trample damage to the opponent, lethal-damage state-based action, Watchwolf's graveyard move, combat order, terminal win, deterministic replay, and gameplay mutations.

Keep the candidate module declarative and limited to the printed card definition. Do not add rules prose, art, scans, or flavor text.
