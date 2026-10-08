# RAV Carrion Howler PvP task

Implement Carrion Howler's candidate-owned RAV identity, `{3}{B}` creature cost, 2/2 body, life-payment binding, and stack-backed +2/-1 activation. The trusted reference match uses legal 60-card decks, casts and resolves Carrion Howler with four actual mana including black, pays one life to activate the ability, resolves the ability, attacks as a live 4/1, and reaches a seat-zero terminal win. The trace also observes the temporary modification expire at cleanup and checks deterministic replay.

The authority contains targeted wrong-pump and wrong-life-payment controls. Its compile-valid starter deliberately changes the pump amount and must fail the card source gate. Candidate changes are limited to the single Carrion Howler card module. This is pilot/reference evidence only; it grants no RAV card or set coverage credit and does not run a provider trial.
