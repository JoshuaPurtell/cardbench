# Hunted Horror source note

This task follows [`RAV_PVP_SOURCE_CONTRACT.md`](../rav-set-pvp/RAV_PVP_SOURCE_CONTRACT.md): the original Ravnica: City of Guilds printing controls card identity and the behavior under test; current Oracle is consulted only to interpret rules questions. The public RAV #90 record is [Hunted Horror](https://www.mtg.wtf/card/rav/90). It identifies a black `{B}{B}` Creature — Horror, 7/7, with Trample and an enters-the-battlefield ability that gives a target opponent two green 3/3 Centaur creature tokens with Protection from Black.

The original printing uses the legacy “comes into play” / “put into play” terminology. Current Oracle uses “enters” / “creates”; the trigger event, opponent target, token count and characteristics, and Protection from Black are unchanged. No gameplay-changing erratum is used in this trace.

The public type line includes the Horror subtype. `CardDefinition` currently represents the Creature card type but has no creature-subtype field, so Horror is retained as public identity in `features.toml` and is not asserted by the executable definition or trace. The created Centaur token subtype is represented in `TokenSpec` and checked in the game trace.

The RAV identity facts and CardBench-authored semantic summary are retained; no scan, image, card-rules prose corpus, flavor text, or upstream response payload is stored. Rules baseline: *Magic: The Gathering Comprehensive Rules*, effective 2026-08-07, with the 2026-09-24 task cutoff.
