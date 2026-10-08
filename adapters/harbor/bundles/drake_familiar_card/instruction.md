# CardBench Magic single-card task: Drake Familiar

Implement Drake Familiar in `varieties/magic/sets/ravnica_city_of_guilds/src/cards/drake_familiar.rs`. Keep its definition and triggered ability in that candidate-owned module. Match RAV #44: blue `{1}{U}`, 2/1 Drake creature, Flying, and a mandatory target-free ETB choice to return any battlefield enchantment to its owner's hand or sacrifice Drake Familiar.

The RAV FAQ and its 2005 ruling clarify that the choice can return any battlefield enchantment, including one controlled by an opponent or one that cannot be targeted. The choice is made during resolution and does not target. The controller may decline even when an enchantment is available; with none available, sacrifice is required if the exact source incarnation is still on the battlefield. The chosen card returns to its owner's hand. Current Oracle preserves these relevant semantics while modernizing the entry wording.

The reference trace starts with legal deterministic 60-card PvP decks, has the opponent cast Dark Heart of the Wood, then casts Drake Familiar and returns that opponent-owned enchantment during resolution before completing a seat-zero win. The task verifies the owner-hand return, trigger ordering, source behavior, deterministic replay, gameplay mutations, exact-source/AST validation, and rejection of a syntactically parsed side-effect injection before candidate functions are called. This fixture is not claimed to have been compiled.

Read the task README and `features.toml` in the trusted handoff before editing. The public workspace exposes only the target card module. Do not add other files to the candidate submission.
