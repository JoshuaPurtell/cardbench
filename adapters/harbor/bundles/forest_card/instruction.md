# CardBench Magic single-card task: Forest

Implement the RAV Forest module at `varieties/magic/sets/ravnica_city_of_guilds/src/cards/forest.rs`.

Forest has no mana cost or color identity, is a basic Land with the Forest type line, and produces one green mana through its intrinsic basic-land ability. The module must provide a pure `definition() -> CardDefinition` and `basic_land_type_binding() -> BasicLandTypeBinding`, and preserve the RAV definition registry's delegation boundary.

The trusted verifier constructs a legal shuffled sixty-card game, records Forest's entry, typed green mana activation/output, the green spell payment and battlefield entry, and a terminal combat win. Four RAV printings (collector numbers 303–306) share this one identity. This is a one-card diagnostic against 291 names and 306 printings; it does not award set credit or Harbor reward.
