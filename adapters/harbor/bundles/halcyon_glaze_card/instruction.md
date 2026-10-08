Implement the candidate module `varieties/magic/sets/ravnica_city_of_guilds/src/cards/halcyon_glaze.rs` with the exported `definition()` and `triggered_ability()` functions.

Match RAV #54: blue `{1}{U}{U}` Enchantment; whenever you cast a creature spell, it becomes a 4/4 Illusion creature with Flying until end of turn. It remains blue and retains Enchantment while animated. The task preserves the original RAV “play a creature spell” wording as the equivalent cast trigger; current Oracle makes type retention explicit. `CardDefinition` has no base subtype field, so Illusion is asserted only during animation.

The task checks a shuffled, legal full-game duel, trigger stack/resolution ordering for controller creature casts, no trigger from an opponent creature cast, Cleanup restoration, five four-damage Glaze attacks, deterministic replay, semantic mutations, and a pre-call exact-source/AST guard. Keep `pilot_reward`, `harbor_reward`, and `cards_credited` at zero.
