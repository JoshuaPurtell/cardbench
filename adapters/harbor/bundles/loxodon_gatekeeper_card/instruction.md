# CardBench Magic single-card task: Loxodon Gatekeeper

Implement the RAV Loxodon Gatekeeper module at `varieties/magic/sets/ravnica_city_of_guilds/src/cards/loxodon_gatekeeper.rs`.

Loxodon Gatekeeper is collector #25: a white 2/3 Creature costing `{2}{W}{W}`. Its static restriction makes opponents' artifacts, creatures, and lands enter tapped. Provide a pure `definition() -> CardDefinition` and `static_entry_restriction_binding() -> StaticEntryRestrictionBinding`, and preserve the RAV registry's delegation boundary.

The trusted authority constructs a shuffled legal sixty-card match, casts and resolves the Gatekeeper, verifies an opponent Plains enters tapped with source-object provenance, and checks a later combat win. This is a one-card diagnostic against 291 unique RAV names and 306 printings; it does not award set credit or Harbor reward.
