# Viashino Slasher RAV card task

Implement the candidate-owned Viashino Slasher module for RAV #149. Its `{1}{R}` red creature is a 1/2 with “{R}, Discard a card: Viashino Slasher gets +1/-1 until end of turn.”

The reference plays a shuffled 60-card PvP game. It pays `{1}{R}` to cast Slasher, then pays the activated ability's `{R}` and discards a real hand card. The ability enters the stack and resolves, making Slasher a live 2/1; cleanup expires the modifier and restores it to 1/2. Slasher repeatedly attacks for 2 damage to end the game. The authority requires deterministic replay and a targeted +0/-1 mutation fails because the live 2/1 is missing. Reports award no RAV coverage or Harbor reward.
