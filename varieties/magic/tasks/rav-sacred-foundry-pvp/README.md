# RAV Sacred Foundry single-card PvP task

This task qualifies Sacred Foundry (RAV #280) through a shuffled 60-card, two-player game. The pilot pays two life for one copy to enter untapped and declines the payment for a second copy so it enters tapped. The paid copy then produces white that is spent on Gate Hound's {2}{W} cost and red that is spent to choose the red half of Boros Recruit's {R/W} hybrid cost. Island and Swamp basics provide Gate Hound's generic mana, so only Sacred Foundry can provide the traced white and red.

The trusted authority checks both entry choices, the two-life receipt and resulting life total, tapped states, source-bound mana receipts, exact colored spell payments, resolution, combat, terminal victory, deterministic replay, and mutations to the entry life, tapped behavior, color choices, entry choice, and tap cost. This is diagnostic reference evidence only; it grants no card credit or Harbor reward.

Materialize with materialize_sacred_foundry_task.py --workspace PATH --manifest PATH; verify the materialized files with the same arguments and --verify. The scorer validates the trusted report outside the candidate process.
