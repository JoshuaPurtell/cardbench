# RAV Temple Garden single-card PvP task

This task qualifies Temple Garden (RAV #284) through a shuffled 60-card
two-player game. The pilot plays one copy after paying two life so it enters
untapped, and declines that payment for a second copy so it enters tapped. The
paid copy produces both green and white mana for real creature spells; those
spells then contribute to the pilot's terminal win.

The trusted authority verifies both entry choices, the two-life receipt and
resulting life total, the entry tapped states, both mana colors and their use,
spell payment/resolution, combat, terminal victory, deterministic replay, and
mutations to the entry cost, tapped behavior, mana colors, entry choice, and
mana ability tap cost. The source target is the candidate-owned
`cards/temple_garden.rs` module. This is diagnostic reference evidence only:
it grants no card credit and no Harbor reward.

Materialize with `materialize_temple_garden_task.py --workspace PATH --manifest
PATH`; verify with the same arguments and `--verify`. The scorer validates the
trusted report outside the candidate process.
