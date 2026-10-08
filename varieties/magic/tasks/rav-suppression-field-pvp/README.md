# RAV Suppression Field single-card PvP task

This task qualifies Suppression Field (RAV #31) in a shuffled two-player 60-card game. Seat zero casts the `{1}{W}` enchantment. While it remains on the battlefield, seat one casts Dimir Guildmage and activates its `{3}{U}` target-player-draw ability; the trace must show the live Field source increasing the nonmana activation to `{5}{U}` and the player paying that exact cost. An Island mana ability activated afterward remains untaxed.

The full-game match verifies Suppression Field's cast and battlefield entry, Dimir Guildmage's cast and draw ability, the cost-calculation receipt naming the Field source and `{2}` increase, the exact ability payment, one card moving from seat zero's library to hand, replay, and a terminal seat-zero PvP win. Mutations removing the live modifier or changing its amount must fail the gameplay oracle. The report credits no set coverage or Harbor reward.

Run `materialize_suppression_field_task.py --workspace PATH --manifest PATH`, then verify with `--verify`. The authority bin is `rav-suppression-field-pvp`; the scorer takes `--workspace`, `--manifest`, `--output-root`, and `--candidate-kind reference|agent`. Harbor/provider evaluation has not been run.
