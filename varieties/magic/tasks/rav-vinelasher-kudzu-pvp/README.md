# RAV Vinelasher Kudzu single-card diagnostic

This task targets Vinelasher Kudzu (Ravnica: City of Guilds collector #189): a green `{1}{G}` 1/1 Creature — Plant. Its mandatory landfall ability puts one +1/+1 counter on that Kudzu whenever a land enters under its controller's control.

The authority plays a complete shuffled 60-card duel. It casts Kudzu, then plays another controlled land and resolves the trigger through the ordinary stack. It also records the opponent's land entry and confirms it does not trigger the pilot's Kudzu. The game continues to a Player 0 terminal win; replay and semantic/source mutations are required.

The normalized source and AST guard runs before any candidate function is called. A parse-valid, type-correct side-effect source mutation is rejected before candidate evaluation. This local diagnostic reports zero card credit and zero reward and does not complete set coverage or grant Harbor qualification.

The editable candidate file is `varieties/magic/sets/ravnica_city_of_guilds/src/cards/vinelasher_kudzu.rs`; it exposes `definition()` and `triggered_ability()`. The printed Plant subtype is retained in task metadata because `CardDefinition` currently has no subtype field. See [source-note.md](source-note.md) for the RAV/Oracle wording reconciliation and references.

Rules baseline: Wizards Comprehensive Rules effective August 7, 2026, with an authority cutoff of September 24, 2026. Independent review, candidate trials, and Harbor receipts remain separate.
