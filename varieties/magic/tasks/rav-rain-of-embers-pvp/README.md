# Rain of Embers RAV card task

Implement the candidate-owned definition for Rain of Embers (RAV #138), a `{1}{R}` sorcery that deals 1 damage to each creature and each player. The reference authority casts it in a shuffled 60-card two-player game with a 1/1 Elves of Deep Shadow and a 3/3 Watchwolf on each side.

The trace records all four creature damage packets and both player packets before either Elf moves to the graveyard. Both players remain at 19 life, both Watchwolves remain on the battlefield with one marked damage, and the two Elves die after the complete damage batch. It then continues to a terminal result. Diagnostic reports credit no RAV coverage or Harbor reward.
