# Woebringer Demon RAV card task

Implement RAV #113 Woebringer Demon in its candidate-owned module with a candidate-owned trigger binding. It is a black 4/4 creature with Flying and costs `{3}{B}{B}`. At the beginning of each upkeep, that upkeep's active player chooses and sacrifices a creature they control if able.

The shuffled two-player 60-card reference duel proves exact five-mana casting, base characteristics, Flying combat, and separate upkeep choices by both seats. Each choice must be opened for the active player, completed by that player, and followed by the exact creature's sacrifice and graveyard move before the trigger resolves. Continue through a terminal pilot win and deterministic replay. Cost, stats, Flying, and trigger-removal mutations must be detected. The task awards no card credit or reward.
