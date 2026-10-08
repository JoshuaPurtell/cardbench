# Golgari Rotwurm card task

Implement the candidate-owned Golgari Rotwurm definition and activated ability in `varieties/magic/sets/ravnica_city_of_guilds/src/cards/golgari_rotwurm.rs`.

Golgari Rotwurm (#210) is a black-green 5/4 creature that costs `{3}{B}{G}`. Its activated ability costs `{B}`, sacrifices another creature, and targets a player to lose 1 life. A deterministic full-game authority casts Rotwurm and an Elves of Deep Shadow fodder creature with explicit colored payment, pays the black activation cost, sacrifices the distinct creature, verifies the target opponent loses one life, and attacks through legal empty blocks until the game ends.

The authority checks live characteristics, ordered typed payment, sacrifice-cost and graveyard movement, target life loss, combat and terminal events, deterministic replay, and three compiled semantic mutants. This diagnostic vertical covers 1 of 291 unique RAV card names from 306 printings, credits no cards, and awards zero reward. No provider trial is part of this task.

The materialized workspace exposes only the candidate module. The provided Harbor bundle invokes a bounded scorer against the trusted reference authority.
