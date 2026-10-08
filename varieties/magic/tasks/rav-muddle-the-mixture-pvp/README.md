# Muddle the Mixture RAV full-game task

Implement Muddle the Mixture (#60) in `varieties/magic/sets/ravnica_city_of_guilds/src/cards/muddle_the_mixture.rs`. Its RAV printing is a blue `{U}{U}` Instant that counters target instant or sorcery spells and has Transmute `{1}{U}{U}`.

The trusted authority uses shuffled legal 60-card decks in a two-player game. Seat zero activates Transmute at sorcery timing, pays the exact cost, privately selects and reveals a second physical Muddle, moves that object to hand, shuffles, then casts that same object for `{U}{U}` to counter the opponent's `{1}{R}` Rain of Embers. The trace records the exact countered object, its owner, the spell counter and source zone moves, and the lack of spell resolution for Rain. Combat continues to a terminal seat-zero win. The authority requires deterministic replay and detects changes to the printed spell cost, counter destination/effect, target restriction, Transmute presence, and Transmute cost.

The source guard compares the candidate module against the pinned normalized literal AST before invoking candidate functions. A syntactically valid injected file-write fixture must be rejected at the guard with zero candidate calls. The public task workspace allows changes only to the candidate card module.

This task records local diagnostic evidence for one identity in the 291-name / 306-printing RAV denominator. Rewards and credited cards remain zero. No provider or Harbor candidate trial is part of the local gates.

Materialize a public workspace with `python3 varieties/magic/scripts/materialize_muddle_the_mixture_task.py --workspace PATH --manifest PATH`; verify it with the same command plus `--verify`.
