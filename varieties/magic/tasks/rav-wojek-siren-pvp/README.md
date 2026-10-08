# RAV Wojek Siren single-card task

Implement the candidate-owned module at `varieties/magic/sets/ravnica_city_of_guilds/src/cards/wojek_siren.rs`. Wojek Siren is RAV collector #37, a white `{W}` instant. It targets a creature; that creature and each other creature sharing a color with it get +1/+1 until end of turn.

The trusted authority runs a deterministic sixty-card match. It verifies live target announcement, exact white payment, layer-seven Radiance effect events for the complete shared-color set, unchanged power/toughness for a nonsharing opposing Goblin Spelunkers, and the modified Watchwolf/Boros Recruit combat before a seat-zero win. It also checks deterministic replay and four full-game semantic mutations.

Run the materializer with `--workspace PATH --manifest PATH` and verify with the same arguments plus `--verify`. The Harbor authority bin is `rav-wojek-siren-pvp`; the local Cargo target is also `rav-wojek-siren-pvp`.

Reference reports are diagnostic: they award no set coverage or Harbor reward. Independent review and a real candidate trial remain pending.
