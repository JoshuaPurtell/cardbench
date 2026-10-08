# Quickchange full-game card task

Implement the candidate-owned RAV Quickchange definition in `varieties/magic/sets/ravnica_city_of_guilds/src/cards/quickchange.rs`. Quickchange is a blue `{1}{U}` Instant. Its caster chooses a color; the target creature becomes only that color until end of turn, then Quickchange's controller draws one card.

The diagnostic authority casts Quickchange targeting the exact Watchwolf object. It verifies the policy-selected Red color, that the Watchwolf changes from green-white to only red, that the color effect expires at the end of the turn and restores green-white, and that one card moves to the caster's hand. That same Watchwolf attacks after its colors revert. The full shuffled duel continues to a terminal result and must replay deterministically. In-game mutations remove the color replacement, remove the draw, and change the printed cost.

This is a single-card diagnostic pilot. It reports one identity among 291 RAV names and 306 printings and awards zero card credit or reward. Materialize with `materialize_quickchange_task.py --workspace PATH --manifest PATH`; verify with the same command plus `--verify`.
