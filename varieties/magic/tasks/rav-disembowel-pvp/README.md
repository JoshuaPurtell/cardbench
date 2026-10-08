# Disembowel RAV card task

Implement the literal RAV #85 Disembowel definition in the candidate-owned card module. It is a black instant with cost `{X}{B}` and a targeted creature destruction instruction that applies only when the target's mana value equals the chosen X.

The authority uses a shuffled two-player 60-card duel. The pilot pays `{1}{B}` with two black mana, targets the opponent's War-Torch Goblin (mana value 1), and must demonstrate the exact target destruction, deterministic replay, and a terminal pilot win. The authority also checks that X=2 cannot target the mana-value-1 Goblin; changing exact equality to “at most X”, removing destruction, or changing the printed cost must be detected. Reports are diagnostic only and award no card credit or reward.
