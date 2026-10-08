# Mnemonic Nexus RAV card task

Implement the candidate-owned module for Mnemonic Nexus, RAV #59. It is a blue `{3}{U}` Instant with an untargeted instruction that moves each player's own graveyard into that same player's library and shuffles each library.

The trusted authority checks the candidate's direct-literal AST before calling its definition, then plays shuffled legal 60-card decks through a full two-player game. Both players cast a Rain of Embers so their graveyards contain distinct owner-controlled objects. The pilot resolves Mnemonic Nexus, and the trace checks the exact graveyard contents at cast, owner-relative movement of every card, one shuffle for each player, the spell's later move to its owner's graveyard, continued combat, terminal win, and deterministic replay. Wrong cost, color, and missing-effect mutations must fail. This remains a zero-credit diagnostic pending independent qualification.
