# Elves of Deep Shadow card task

Implement the candidate-owned definition and mana ability in `varieties/magic/sets/ravnica_city_of_guilds/src/cards/elves_of_deep_shadow.rs`.

Elves of Deep Shadow (#161) is a green 1/1 creature costing `{G}`. Its tap ability adds `{B}` and deals 1 damage to its controller. The deterministic full-game diagnostic casts both players' Elves, uses the pilot's Elf to produce black mana and pay the black pip of Last Gasp after an opposing Watchwolf attacks, verifies the drawback damage and opposing creature's death, then wins through Watchwolf combat.

The authority checks candidate source and registry delegation, live battlefield characteristics, exact colored spell payments, typed payment/cast/resolution/entry/activation/mana/damage/combat/terminal events in order, deterministic replay, and five semantic mutants. This vertical covers 1 of 291 unique RAV card names from 306 printings. It credits no cards and awards zero reward; external qualification remains pending. No provider trial is part of this task.

The materialized workspace exposes only the candidate module. Its Harbor bundle runs the bounded scorer against the trusted authority.
