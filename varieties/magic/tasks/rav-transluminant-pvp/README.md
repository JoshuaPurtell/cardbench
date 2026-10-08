# RAV Transluminant single-card diagnostic

This isolated task targets Transluminant (RAV collector #186), a green 2/2 creature costing `{1}{G}`. A legal constructed game casts it from a 60-card Selesnya list; Golgari answers it with Putrefy. The typed trace proves the creature's exact battlefield incarnation moves to the graveyard before its optional dies ability resolves.

The same shuffled matchup runs both choices. Accepting the trigger creates exactly one live 1/1 white Spirit with flying. Declining creates no token from this ability. Both branches replay deterministically and report 291 RAV names / 306 printings with zero card credit and reward. Internal mutations cover the body, missing trigger, and wrong token.

The target module contains both `definition()` and candidate-owned `triggered_ability()`. The rules baseline is the Wizards Comprehensive Rules effective August 7, 2026, at the September 23, 2026 cutoff. Independent review and a candidate trial remain separate qualification gates.
