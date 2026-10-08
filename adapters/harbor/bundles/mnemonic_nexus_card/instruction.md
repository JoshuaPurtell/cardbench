# Task: implement Mnemonic Nexus

Implement the candidate-owned Mnemonic Nexus definition in the provided RAV card module. Preserve Ravnica: City of Guilds #59: blue Instant, `{3}{U}`, moving each player's own graveyard into that player's own library and shuffling each library.

The authority runs a shuffled legal 60-card two-player game. Before the pilot casts Mnemonic Nexus, each player's graveyard contains a real spell card. The ordered trace must show all cards from seat zero's graveyard moving to seat zero's library, all cards from seat one's graveyard moving to seat one's library, and a shuffle for each library. The resolving instant itself must not be among those moved objects. The same game continues through combat to a normal terminal result.

Keep implementation in the candidate module. Do not change the shared engine, card registry, authority, or task tooling.
