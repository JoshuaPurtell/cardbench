# Source note — Grifter's Blade

The RAV catalog binds Grifter's Blade to collector number 263. The retained card facts are a colorless `{3}` Artifact — Equipment with Flash; an as-enters choice to attach to a creature its controller controls, or enter unattached; an equipped-creature `+1/+1` bonus; and Equip `{1}`. The public source contract is [RAV_PVP_SOURCE_CONTRACT.md](../rav-set-pvp/RAV_PVP_SOURCE_CONTRACT.md); the catalog denominator is [rav_main_set.tsv](../../sets/ravnica_city_of_guilds/catalog/rav_main_set.tsv).

The original RAV wording describes the Equipment entering equipped to a creature of its controller's choice. Contemporary discussion treated this as an entry choice rather than a triggered ability. Current Oracle wording makes the timing explicit with an “as this Equipment enters” choice and permits it to enter unattached when no choice is made. Both forms are non-targeting: neither puts an ability on the stack, creates a target, or opens a response window between the choice and attachment. This task models the printed RAV contract with that entry timing, rather than the earlier inaccurate target-bearing ETB implementation.

The engine binding implements this choice when a permanent spell resolves from the stack. Other effects that put the Equipment directly onto the battlefield do not yet have a resumable entry-choice continuation and are explicitly outside this task's trace claim.

Source pages used to verify identity and compare wording:

- [Star City Games: Grifter's Blade, RAV #263](https://starcitygames.com/grifters-blade-sgl-mtg-rav-263-enf/) — RAV identity, collector number, and current card record.
- [Card Kingdom: Grifter's Blade](https://www.cardkingdom.com/mtg/ravnica/grifters-blade) — Ravnica set, type, collector number, cost, current Oracle wording, and Equip cost.
- [2005 Ravnica preview discussion](https://forums.tomshardware.com/threads/grifters-blade.122329/) — contemporary record of the original “comes into play equipping” text and discussion of its non-triggered entry behavior.

Only public identity and rules-relevant card facts are retained here. No scan, card image, flavor text, or full rules-text payload is included.
