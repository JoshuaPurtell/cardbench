# CardBench Magic single-card task: Grave-Shell Scarab

Implement the candidate-owned definition and activated ability in `varieties/magic/sets/ravnica_city_of_guilds/src/cards/grave_shell_scarab.rs`. That is the only candidate-editable file.

Preserve the RAV #211 printing: a black-green `{2}{B}{G}{G}` 4/4 creature with Dredge 1 and `{1}, Sacrifice this creature: Draw a card.` The shuffled PvP trace casts Grave-Shell Scarab, pays `{1}`, sacrifices that exact object, and replaces the activated ability's draw with Dredge 1 from its own graveyard. The trace checks one card milled, the same object returned to hand, and that object recast. A second activation declines Dredge and must draw normally. The match continues to a terminal pilot win.

The authority checks deterministic replay and semantic mutations for the printed cost, body, Dredge number, activation cost, source sacrifice, ability binding, and draw effect. The scorer remains diagnostic and awards zero credit or reward pending independent qualification.
