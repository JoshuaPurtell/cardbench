# Darkblast RAV card task

Implement the candidate-owned module for Darkblast, RAV #82. It is a black `{B}` instant that gives target creature -1/-1 until end of turn and has Dredge 3.

The trusted authority first validates normalized candidate source and its direct-literal AST, before any candidate function runs. It then runs a shuffled, legal 60-card PvP game against Watchwolf. Two opposing 3/3 Watchwolves are each reduced to 2/2 by the same Darkblast object. Each blocks the pilot's Watchwolf and dies to its three combat damage while dealing two damage back. After Darkblast reaches its owner's graveyard, one draw is replaced by Dredge 3: three exact library objects are milled, the same Darkblast object returns to hand, and it is recast at the second Watchwolf. The game continues to a normal terminal pilot win and must replay deterministically.

Cost, modifier, Dredge presence/count, and compile-valid injected-side-effect mutations must be rejected. The task is diagnostic only and awards zero card credit and zero reward.
