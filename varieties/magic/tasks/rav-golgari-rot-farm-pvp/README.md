# Golgari Rot Farm RAV card task

Implement RAV #278 Golgari Rot Farm in its candidate-owned module. The land enters tapped, its entry trigger targets and returns a land you control to its owner's hand, and its tap ability adds exactly `{B}{G}`.

The shuffled two-player 60-card reference duel proves the tapped post-entry state, legal controller-owned target, exact land hand move, trigger resolution order, and fixed black-green mana bundle from the Rot Farm itself. Use that mana to cast a creature, finish combat, and continue to a terminal seat-zero win. Compare the full event log on deterministic replay. The task awards no card credit or reward.
