# RAV Watery Grave single-card PvP task

This task qualifies Watery Grave (RAV #286) through a shuffled 60-card
two-player game. The pilot plays one copy after paying two life so it enters
untapped, and declines that payment for a second copy so it enters tapped. The
paid copy produces both blue from the paid Grave plus black generic mana to cast Surveilling Sprite ({1}{U}), then black from the same Grave plus blue generic mana to cast Roofstalker Wight ({1}{B}); those
spells then contribute to the pilot's terminal win.

The trusted authority verifies both entry choices, the two-life receipt and
resulting life total, the entry tapped states, both mana colors and their use,
spell payment/resolution, combat, terminal victory, deterministic replay, and
mutations to the entry cost, tapped behavior, mana colors, entry choice, and
mana ability tap cost. The source target is the candidate-owned
`cards/watery_grave.rs` module. This is diagnostic reference evidence only:
it grants no card credit and no Harbor reward.

Materialize with `materialize_watery_grave_task.py --workspace PATH --manifest
PATH`; verify with the same arguments and `--verify`. The scorer validates the
trusted report outside the candidate process.
