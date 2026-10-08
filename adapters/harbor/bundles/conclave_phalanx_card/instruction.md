# CardBench Magic single-card task: Conclave Phalanx

Implement Conclave Phalanx in `varieties/magic/sets/ravnica_city_of_guilds/src/cards/conclave_phalanx.rs` by returning the correct declarative definition and trigger binding.

Conclave Phalanx is a white 2/4 Creature with mana cost `{4}{W}` and Convoke. Its enters-the-battlefield ability makes its controller gain life equal to the number of creatures that player controls as the ability resolves. The shuffled full-game trace taps four distinct creatures for the four generic mana, pays the white symbol with one White mana, and resolves the trigger while five creatures are controlled by its controller and one creature is controlled by the opponent.

The authority validates ordered typed payment, casting, resolution, battlefield entry, trigger, life gain, combat, and game-end events; exact contributor provenance; live characteristics; deterministic replay; and gameplay mutants for missing Convoke, increased generic cost, and an incorrect global creature count. The public module stub compiles but does not satisfy the card contract. The report is diagnostic-only, with 1 of 291 names passing out of 306 printings and zero credit or reward.
