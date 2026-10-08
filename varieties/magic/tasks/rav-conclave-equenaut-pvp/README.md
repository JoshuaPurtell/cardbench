# Conclave Equenaut PvP card task

Implement the RAV Conclave Equenaut card definition in the candidate-owned module. The diagnostic authority runs a shuffled 60-card full game: four distinct creatures each pay one generic mana through Convoke, two white mana pay the colored symbols, then the resolved white 3/3 Flying creature attacks over an opposing ground Watchwolf. The convokers are drawn from Birds of Paradise and Watchwolf playsets. A separate rejected-action probe proves the engine does not allow that ground creature to block the live Flying attacker; the actual policy trace remains legal and has zero rejected moves.

The task checks the ordered Convoke, mana payment, cast, resolution, entry, combat, and game-end events; exact payment colors; live characteristics; deterministic replay; and compiled semantic controls for Convoke, the generic cost, and Flying. This remains diagnostic-only: one of 291 RAV names, 306 printings, zero card credit, and zero reward.
