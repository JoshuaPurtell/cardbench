# CardBench Magic single-card task: Hunted Horror

Implement the candidate-owned Hunted Horror definition and enters-the-battlefield ability in `varieties/magic/sets/ravnica_city_of_guilds/src/cards/hunted_horror.rs`.

The trusted diagnostic authority validates the exact candidate AST before invoking candidate functions. A compile-valid injected `std::process::exit(0)` is rejected at this source boundary before candidate execution, game construction, or report generation. It then runs one deterministic, legal 60-card duel. It casts Hunted Horror for exactly `{B}{B}`, records the battlefield entry, and verifies one target-selection submission by the trigger controller before resolving the mandatory ETB trigger for the opponent. The opponent receives two green 3/3 Centaur creature tokens with Protection from Black. A no-state-change rules probe confirms that a protected Centaur cannot block the black Horror. The opposing Watchwolf blocks the 7/7, and the trace verifies that Hunted Horror assigns 3 damage to Watchwolf and 4 excess trample damage to the defending player before a terminal seat-zero win. The authority replays the game and checks body, trample, trigger, target, token, and mana-cost mutations.

The printed subtype is Horror. It is retained as public identity, but the current `CardDefinition` model has no creature-subtype field, so the candidate definition and gameplay trace assert Creature only. See [source-note.md](source-note.md) for the RAV/Oracle reconciliation and the model boundary.

This is an uncredited diagnostic task against 291 distinct RAV names and 306 printings. It reports zero card credit and zero reward. Candidate evaluation is separate.
