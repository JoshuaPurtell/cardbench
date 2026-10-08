# Implement Induce Paranoia

Implement `varieties/magic/sets/ravnica_city_of_guilds/src/cards/induce_paranoia.rs` as a literal-only module exporting `definition() -> CardDefinition`. Preserve RAV #56: a blue `{2}{U}{U}` instant that counters target spell and, only when black mana was spent to cast it, makes the target spell's controller put the top number of cards equal to that spell's mana value into their graveyard.

The trusted authority runs two shuffled, legal 60-card PvP games. It checks the exact target identity and response window, actual payment colors, counter provenance, target spell's terminal zone, the exact ordered top-three library objects in the black-spent branch, and no mill in the control branch. Both games must reach a terminal seat-zero win and replay deterministically. It also checks mutations and rejects malformed or side-effecting candidate source before candidate calls. Keep the candidate module declarative and do not add files.
