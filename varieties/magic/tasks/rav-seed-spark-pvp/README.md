# Seed Spark full-game card task

Implement the candidate-owned Seed Spark definition at `varieties/magic/sets/ravnica_city_of_guilds/src/cards/seed_spark.rs`. Seed Spark is RAV collector #30, a white Instant costing `{3}{W}`. Its ordered instructions destroy target artifact or enchantment, then create two green 1/1 Saproling creature tokens.

The authority runs a shuffled 60-card duel. The opponent casts Boros Signet; the pilot pays the exact four mana including white to destroy that exact artifact, observes the target move to its owner's graveyard, and then observes two distinct green Saproling token objects before Seed Spark resolves and moves to its graveyard. Both tokens participate in later combat, and the game continues to a terminal pilot win. The authority replays the match deterministically and detects removal of either spell instruction, a reduced token count, a changed token body, and a changed mana cost.

This is a diagnostic pilot only. It reports one identity among 291 RAV names and 306 printings and awards zero card credit and zero reward. No provider trial is part of the reference checks. Materialize with `materialize_seed_spark_task.py --workspace PATH --manifest PATH`; verify with the same command plus `--verify`.
