# Undercity Shade RAV card task

Implement Undercity Shade (RAV #110) in `varieties/magic/sets/ravnica_city_of_guilds/src/cards/undercity_shade.rs`. The candidate module owns the black `{4}{B}` 1/1 creature definition, Fear/black-only evasion, and `{B}` self-pump ability. The printed Shade subtype is retained in task metadata but is not asserted by the current `CardDefinition` model.

The reference authority uses two legal shuffled 60-card decks. It casts Undercity Shade for the printed cost, pays black mana to resolve the +1/+1 effect, attempts combat against a live green-white Watchwolf, and verifies that the nonblack nonartifact blocker is rejected without changing the event log. It then checks two separate activations, combat damage, cleanup expiration to base 1/1 stats, deterministic replay, ordered typed events, and a terminal Player 0 win. A parse-valid side-effecting source fixture is rejected before candidate factories run; the task does not claim that fixture was compiler-validated. The trace awards zero card credit and zero reward.

Fear follows the RAV printing: artifact creatures and black creatures are eligible blockers. Current Oracle records retain the same gameplay behavior. The task source note records the card-name/self-reference wording reconciliation; subtype identity is descriptive metadata only.

Materialize with `materialize_undercity_shade_task.py --workspace PATH --manifest PATH`; verify with the same command plus `--verify`. The scorer accepts `--candidate-kind reference|agent` and runs the candidate in the bounded offline worker.
