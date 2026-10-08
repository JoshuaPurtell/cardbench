# Cleansing Beam full-game card task

Implement the candidate-owned Cleansing Beam definition at `varieties/magic/sets/ravnica_city_of_guilds/src/cards/cleansing_beam.rs`. Cleansing Beam is RAV collector #118, a red Instant costing `{4}{R}`. Its Radiance instruction deals 2 damage to target creature and each other creature that shares a color with that target.

The authority runs a shuffled 60-card duel. Its board fixture places a white 2/2 creature under the opponent, another white 2/2 creature under the pilot, and a blue 2/2 creature under the opponent. The pilot casts Cleansing Beam at the opposing white creature: the target and pilot-controlled white creature must each take exactly 2 damage and be put into their owners’ graveyards by lethal-damage state-based actions, while the blue creature remains on the battlefield. The trace checks exact red `{4}{R}` payment, damage and move receipts, spell resolution ordering, deterministic replay, and continuation to a terminal pilot win. Mutations remove Radiance, change damage to 1, or change the printed cost.

This is a diagnostic pilot only. It reports one identity among 291 RAV names and 306 printings and awards zero card credit and zero reward. No provider trial is part of the reference checks. Materialize with `materialize_cleansing_beam_task.py --workspace PATH --manifest PATH`; verify with the same command plus `--verify`.
