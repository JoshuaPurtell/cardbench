# Gaze of the Gorgon source note

The RAV printing is the controlling source for identity and functionality,
following `RAV_PVP_SOURCE_CONTRACT.md`. The Ravnica: City of Guilds collector
#246 printing is a `{3}{B/G}` instant whose functional branches are: regenerate
the target creature, then at end of combat destroy creatures that blocked or
were blocked by that creature this turn. Retained facts are the public card
identity, set and collector number, hybrid cost, colors, instant type, and the
two effect branches exercised by this trace. The task preserves no scan, art,
flavor text, or card-text corpus.

The current Oracle transcription updates the delayed timing phrase to “at this
turn's next end of combat” and refers to the target as “it.” Those wording
updates clarify the RAV printing's end-of-combat timing and antecedent; they do
not replace its mechanics. The RAV FAQ ruling says the delayed destruction is
performed at the next end-of-combat step in the turn and includes creatures
that blocked the target earlier that turn. The engine trace exercises that
interpretation after a block already occurred. The task follows the RAV source
contract and the applicable Comprehensive Rules for timing.

Source checks:

- RAV #246 card record and printed functional text: [LastDodo collector entry](https://www.lastdodo.fr/fr/items/829185-gaze-of-the-gorgon), which identifies the 2005 Ravnica: City of Guilds printing and collector number.
- Current card record and Oracle wording: [mtg.wtf Gaze of the Gorgon](https://mtg.wtf/card/rav/246/Gaze-of-the-Gorgon).
- RAV FAQ text and the next-end-of-combat ruling: [Ravnica: City of Guilds official FAQ reproduction](https://www.mtgsalvation.com/forums/magic-fundamentals/the-rumor-mill/224968-rav-official-faq).
- Rules baseline: [Comprehensive Rules effective 2026-08-07](https://media.wizards.com/2026/downloads/MagicCompRules%2020260807.pdf), especially sections 117, 400.7, 511, 701.15, and 704.

The existing `gaze_of_gorgon_full_fidelity.rs` fixture is supporting engine
evidence, not a substitute for the isolated full-game PvP trace.
