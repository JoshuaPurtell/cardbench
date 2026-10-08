# Selesnya Sagittars task

This diagnostic RAV card task evaluates Selesnya Sagittars (#229) in a complete shuffled two-player game. Implement only `definition()` and `tap_damage_ability()` in the target module.

The authority verifies its exact `{3}{G}{W}` cost and live 2/5 Reach characteristics; blocks a flying Courier Hawk; activates the tap ability during combat against an attacking Elves of Deep Shadow; checks one damage, target removal, the Reach combat exchange, and later Watchwolf lethal damage; and confirms that an unrelated noncombat creature is rejected as an activation target without changing the probe state. Deterministic replay and four compiled gameplay mutants guard the slice.

The report is diagnostic-only: one card is trace-passed out of the 291-name, 306-printing RAV catalog. It credits zero cards and awards zero reward. No provider trial is part of local materialization or authority checks.

Generate and verify a candidate workspace with `varieties/magic/scripts/materialize_selesnya_sagittars_task.py`. Score a submitted workspace through the isolated worker using `varieties/magic/scripts/score_selesnya_sagittars_pvp.py` after registering the `rav-selesnya-sagittars-pvp` session binary.
