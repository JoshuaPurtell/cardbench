# Empty the Catacombs RAV card task

Implement the candidate-owned module for Empty the Catacombs, RAV #86: black `{3}{B}` Sorcery. Its untargeted effect returns all creature cards from each player's own graveyard to their hand, without asking for card selections.

The trusted authority checks the candidate's direct-literal AST before calling its definition, then plays shuffled legal 60-card decks through a full two-player game. Two Caregivers per player are put into the graveyards by Rain of Embers. The pilot resolves Empty the Catacombs, and the trace checks every owner-relative creature return, the noncreature Rain card left behind, the absence of a one-card public-choice prompt, the sorcery's later move to its owner's graveyard, continued combat, terminal win, and deterministic replay. Wrong cost, color, and missing-effect mutations must fail. This remains a zero-credit diagnostic pending independent qualification.
