# CardBench Magic single-card task: Flow of Ideas

Implement the RAV Flow of Ideas definition at collector #50 in
`varieties/magic/sets/ravnica_city_of_guilds/src/cards/flow_of_ideas.rs`.
Preserve the candidate-owned pure `definition() -> CardDefinition` source
boundary and the existing fields.

The diagnostic runs a legal shuffled 60-card duel. Flow of Ideas costs `{5}{U}`
and must draw one card for each Island its controller controls. The authority
records six controlled Islands and five opponent Islands at cast time, then
requires six distinct hand movements between cast and resolution. Typed events
also verify exact mana payment, graveyard movement, deterministic replay, and a
terminal Watchwolf combat win. Compiled wrong-cost, wrong-color, wrong-land-type,
and fixed-one-card-draw mutants must be detected.

This package measures one card against 291 unique RAV identities and 306
printings. It grants no set coverage or reward. The task is diagnostic-only;
independent review and candidate evaluation remain separate.
