# Leave No Trace full-game card task

Implement the candidate-owned Leave No Trace definition at `varieties/magic/sets/ravnica_city_of_guilds/src/cards/leave_no_trace.rs`. Leave No Trace is RAV collector #23, a white Instant costing `{1}{W}`. Its Radiance instruction destroys the target enchantment and each other enchantment that shares a color with that target.

The authority runs a shuffled 60-card duel. Its board fixture places a white enchantment under the opponent, another white enchantment under the pilot, and a blue enchantment under the opponent. The pilot casts Leave No Trace at the opposing white enchantment: the target and pilot-controlled white enchantment must be destroyed and moved to their owners' graveyards, while the blue enchantment remains on the battlefield. The trace checks exact white plus one-generic payment, source/target receipts, ordered moves and spell resolution, deterministic replay, and continuation to a terminal pilot win. Mutations omit Radiance, reduce it to target-only destruction, or change the printed cost.

This is a diagnostic pilot only. It reports one identity among 291 RAV names and 306 printings and awards zero card credit and zero reward. No provider trial is part of the reference checks. Materialize with `materialize_leave_no_trace_task.py --workspace PATH --manifest PATH`; verify with the same command plus `--verify`.
