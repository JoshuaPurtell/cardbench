# Sabertooth Alley Cat RAV card task

Implement the literal RAV #140 definition in the candidate-owned module. The task checks the red `{1}{R}{R}` 2/1 creature and its Mountain-dependent blocking restriction in a shuffled two-player 60-card game.

The reference authority casts the Cat while its controller has no Mountain, proves that an attempted block is rejected without changing the event log, then plays a Mountain and legally blocks a War-Torch Goblin. Combat moves both 1-toughness creatures to their owners' graveyards. The duel continues to a seat-zero win and the same seed must replay identically. Gameplay mutations removing the restriction, changing the cost, or changing the Cat's power must fail. Diagnostic reports award no card credit or Harbor reward.
