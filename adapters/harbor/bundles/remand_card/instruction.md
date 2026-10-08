# Implement Remand

Implement `varieties/magic/sets/ravnica_city_of_guilds/src/cards/remand.rs` as
a literal-only module exporting `definition() -> CardDefinition`. Preserve
the RAV #63 identity: blue `{1}{U}` instant, with
`Effect::CounterTargetSpellToOwnersHand` followed by `Effect::DrawController`.

The trusted authority casts Remand in a legal shuffled two-player game in
response to one Watchwolf spell. It asserts the exact spell object and counter
source, returns that card to its owner's hand, draws the top card for the
Remand controller, then continues the game to a seat-zero terminal result. It
also requires deterministic replay and gameplay failures for a wrong
destination, missing draw, or missing counter. Keep the module declarative and
do not add other candidate files.
