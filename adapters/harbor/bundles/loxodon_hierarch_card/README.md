# Loxodon Hierarch card task

Implement the RAV Loxodon Hierarch candidate module at `varieties/magic/sets/ravnica_city_of_guilds/src/cards/loxodon_hierarch.rs`. Preserve RAV #214's `{2}{G}{W}` white-green 4/4 creature identity, mandatory entry gain of four life, and `{G}{W}`, sacrifice-this-source ability that gives each creature you control a regeneration shield.

The reference authority plays a complete shuffled 60-card duel. It verifies that an opponent's Char is answered by paying both colored mana and sacrificing Loxodon Hierarch, that the ability shields the controller's creatures at resolution but not the opponent's, and that lethal Char damage consumes the targeted Watchwolf's shield. Replay and semantic mutation sensitivity are required. The engine currently lacks subtype fields: retain Elephant Cleric in the source metadata but do not add unrelated engine changes.

Only the declared card module is editable. The trusted authority validates its constrained declarative source shape before invoking candidate functions. Do not edit registry, Cargo, task metadata, scripts, provenance, or authority files.
