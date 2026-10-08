# Twilight Drover source note

## Source contract

The RAV #33 printing is authoritative for this task. Its card facts are white, `{2}{W}`, Creature — Spirit, 1/1; a mandatory trigger places one +1/+1 counter on Twilight Drover whenever a creature token leaves play; and `{2}{W}`, remove one +1/+1 counter from Twilight Drover, creates two 1/1 white Spirit creature tokens with flying.

The current Oracle text for RAV #33 uses “leaves the battlefield,” “this creature,” and “create.” The effect structure remains the same: one trigger per departing creature token, including the Drover's own Spirit tokens, and two Spirit tokens per activation. The task does not broaden the trigger to nontoken creatures. The original-era term “leaves play” is normalized to the present battlefield zone in the engine event model.

The candidate's nontoken body subtype cannot currently be encoded in `CardDefinition`; the retained printed identity is Spirit, but this task makes no subtype assertion for the body. `TokenSpec::white_spirit()` does carry Spirit subtype and flying for both activated tokens, and the full-game trace checks those facts.

## Public references checked 2026-09-24

- [RAV #33 current Oracle transcription and 2005 rulings](https://www.mtg.wtf/card/rav/33/Twilight-Drover)
- [RAV card-face transcription preserving the 2005 wording](https://www.lastdodo.com/en/items/829511-twilight-drover)
- [RAV forum-era spoiler transcript, dated September 2005](https://static.mtgsalvation.com/forums/magic-fundamentals/the-rumor-mill/224930-rav-the-rest-of-it-update-post-104)

The two secondary transcriptions agree on the original cost, token-only condition, counter, two-token quantity, colors, subtype, and flying. The task uses that printed-era text as its authority and records current Oracle phrasing only as reconciliation; no art, scan, or card-text corpus is retained in the public task files.
