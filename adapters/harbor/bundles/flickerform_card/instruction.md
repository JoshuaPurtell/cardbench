Implement the candidate-owned Flickerform module at `varieties/magic/sets/ravnica_city_of_guilds/src/cards/flickerform.rs`.

Flickerform is a white `{1}{W}` Enchantment Aura that enchants a creature. Its `{2}{W}{W}` activated ability exiles the enchanted creature and every Aura attached to it, then returns those cards under their owners' control at the next end step.

Keep the module declarative and limited to the card definition and activated-ability binding. The isolated authority checks exact casting and activation identity, the linked creature and Aura objects, exile and return incarnations, the delayed end-step receipt, legal reattachment and resulting characteristics, later combat, a terminal full game, deterministic replay, and semantic mutants. Do not add rules prose, art, scans, or flavor text.
