# CardBench Magic single-card task: Flow of Ideas

Implement Flow of Ideas at RAV collector #50 in
`varieties/magic/sets/ravnica_city_of_guilds/src/cards/flow_of_ideas.rs`.
The printed definition is a blue Sorcery costing `{5}{U}`. On resolution, its
controller draws one card for each Island they control. Keep the candidate
implementation as the declarative `definition() -> CardDefinition` in the
existing module and preserve its field layout.

The full-game authority makes the controller pay six blue mana with six
controlled Islands while the opponent controls five. It checks the live source
definition and captures the Island counts at the spell's cast event, requires
six different cards to enter the controller's hand between `SpellCast` and
`SpellResolved`, and validates graveyard movement and typed event order. A
green-white Watchwolf follow-up finishes the duel. The same shuffle seed is
replayed, and compiled semantic mutants cover the cost, color, land type, and
draw count.

The result is diagnostic only: one card is checked against the 291-name / 306
printing RAV catalog; reward and whole-set coverage stay at zero/false. The
Comprehensive Rules snapshot is effective August 7, 2026, at the September 24,
2026 authority cutoff.
