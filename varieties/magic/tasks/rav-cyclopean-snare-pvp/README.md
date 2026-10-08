# Cyclopean Snare RAV full-game task

Implement Cyclopean Snare (#259) in `varieties/magic/sets/ravnica_city_of_guilds/src/cards/cyclopean_snare.rs`. Keep both the card definition and its activated binding in this candidate-owned module. It is a colorless `{2}` artifact with “`{3}, {T}: Tap target creature. Return Cyclopean Snare to its owner's hand.`”

The deterministic shuffled PvP game casts Snare, pays its `{3}` and tap activation costs targeting the opponent's Watchwolf, verifies the target taps and that the same Snare object returns to its owner's hand, then uses the pilot's Watchwolf to reach a full-game win.

The authority checks source and registry delegation, deterministic replay, exact costs and target, the target's tapped state, the source's zone change, event order, and gameplay mutations for missing target tap, missing source return, missing source tap cost, wrong activation cost, and missing creature target. This diagnostic task awards no card credit or Harbor reward. Only the declared module is editable in the public candidate workspace.
