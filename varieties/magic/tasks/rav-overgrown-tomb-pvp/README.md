# RAV Overgrown Tomb single-card PvP task

This task qualifies Overgrown Tomb (RAV #279) in a shuffled 60-card
two-player game. The pilot pays two life for one copy to enter untapped and
declines for a second copy to enter tapped. The paid Tomb produces Green that
is spent to cast Elves of Deep Shadow ({G}); it later produces Black for
Roofstalker Wight ({1}{B}), with a Forest paying that spell's generic pip. The
creatures help the pilot reach a terminal win.

The trusted authority verifies both entry choices, the two-life receipt and
resulting life total, entry states, each mana color from the paid Tomb and its
use in exact spell payments, spell resolution, combat, terminal victory,
deterministic replay, and mutations to entry cost, tapped behavior, mana
colors, entry choice, and tap cost. The candidate target is the
`cards/overgrown_tomb.rs` module. This is diagnostic evidence only: it grants
no card credit or Harbor reward.

Materialize with `materialize_overgrown_tomb_task.py --workspace PATH --manifest
PATH`; verify with the same command and `--verify`. The scorer validates the
trusted report outside the candidate process.
