# Implement Convolute

Implement `varieties/magic/sets/ravnica_city_of_guilds/src/cards/convolute.rs` as a literal-only module exporting `definition() -> CardDefinition`. Preserve the RAV #41 identity: blue `{2}{U}` instant, with `Effect::CounterTargetSpellUnlessControllerPays { mana_cost: ManaCost::new(4) }`.

The trusted authority runs two shuffled, legal 60-card PvP matches. One observes the target controller explicitly declining `{4}` and the exact Watchwolf being countered. The other observes the target controller activating four lands to pay `{4}` and the target creature resolving. Both require the pilot's exact-spell Convolute response, `{2}{U}` payment, deterministic replay, and a later Watchwolf combat win. Keep the module declarative and do not add other files.
