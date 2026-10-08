# Peel from Reality RAV card task

Implement the literal RAV #61 definition in the candidate-owned module. The task checks the blue `{1}{U}` instant in a shuffled, legal two-player 60-card duel.

The reference casts Peel with two distinct creatures as targets: its controller's Bird of Paradise first and the opponent's War-Torch Goblin second. The authority checks the exact target order, mana payment, both target moves to their respective owners' hands, and the owners' private hand views. It then continues to a terminal seat-zero win and requires deterministic replay. Gameplay mutations swap either target scope, remove the second return, or change the cost. Diagnostic reports award no card credit or Harbor reward.
