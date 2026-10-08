Implement the candidate-owned Dowsing Shaman module at `varieties/magic/sets/ravnica_city_of_guilds/src/cards/dowsing_shaman.rs`.

Dowsing Shaman is a green `{4}{G}` 3/4 creature with `{2}{G}, {T}: Return target enchantment card from your graveyard to your hand`. Keep the module declarative and limited to the candidate-owned card definition and activated-ability binding.

The local full-game authority checks exact casting and activation payments, an Aura legally attached to a creature that dies, the exact Aura card in its controller's graveyard as the ability target, the tap cost and stack resolution, the Aura's move to hand, a terminal game, deterministic replay, and semantic mutants. Do not add rules prose, art, scans, or flavor text.
