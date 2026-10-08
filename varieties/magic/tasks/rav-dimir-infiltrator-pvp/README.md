# Dimir Infiltrator RAV card task

Implement Ravnica: City of Guilds #203, Dimir Infiltrator: a blue-black `{U}{B}` 1/3 Spirit with unblockable and Transmute `{1}{U}{B}`. Its Transmute ability can be activated from its owner's hand at sorcery timing by paying the cost and discarding this card, then finding a card with the same mana value.

The shuffled PvP authority transmute-searches a Dimir Signet (also mana value 2), verifies the selected physical card is revealed and put into hand privately, then casts an Infiltrator and attacks past opponent-controlled Carven Caryatid blockers. It checks deterministic replay, terminal seat-zero win, and semantic mutations. This diagnostic awards zero card credit and reward.
