# Task: implement Empty the Catacombs

Implement the candidate-owned Empty the Catacombs definition in the provided RAV card module. Preserve Ravnica: City of Guilds #86: black Sorcery, `{3}{B}`, returning all creature cards from each player's own graveyard to their hand.

The authority runs a shuffled legal 60-card two-player game. Before the pilot casts Empty the Catacombs, each player's graveyard contains at least two creature cards, plus a noncreature spell control. The ordered trace must show every creature card returning to its owner's hand, the noncreature remaining in its graveyard, and the sorcery moving to its owner's graveyard only after resolving. The effect does not ask players to choose which creature to return. The same game continues through combat to a normal terminal result.

Keep implementation in the candidate module. Do not change the shared engine, card registry, authority, or task tooling.
