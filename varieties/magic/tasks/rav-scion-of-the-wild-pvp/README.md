# Scion of the Wild task

This diagnostic RAV card task evaluates Scion of the Wild (#182) in a complete shuffled two-player game. Implement only `definition()` and `static_continuous_effect()` in the target module.

The authority pays the exact `{1}{G}{G}` cost, checks the live 1/1 Scion while it is the only controlled creature, then casts Watchwolf and checks Scion immediately updates to 2/2. It verifies typed payment, cast, resolution, battlefield-entry, attacker, combat-damage, and terminal events; deterministic replay; and a compiled in-game mutant with the creature-count effect removed.

The report is diagnostic-only: one trace-passed card from the 291-name, 306-printing RAV catalog, zero credited cards, and zero reward. Local materialization and authority checks do not run provider trials.

Generate and verify a candidate workspace with `varieties/magic/scripts/materialize_scion_of_the_wild_task.py`. Score a submitted workspace through the isolated worker using `varieties/magic/scripts/score_scion_of_the_wild_pvp.py` after registering the `rav-scion-of-the-wild-pvp` session binary. Do not change files outside the candidate module.
