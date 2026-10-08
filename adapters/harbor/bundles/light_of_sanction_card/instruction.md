# CardBench Magic single-card task: Light of Sanction

Implement Light of Sanction (RAV #24) in `varieties/magic/sets/ravnica_city_of_guilds/src/cards/light_of_sanction.rs`. Keep its behavior candidate-owned through the definition and static continuous-effect binding.

The trusted reference uses a shuffled 60-card duel. It requires Light of Sanction to prevent four damage from its controller's Char to a controlled Watchwolf while Char's separate two damage to its caster still resolves. An opponent's Char must deal four damage to that same Watchwolf and move it to the graveyard while Light remains on the battlefield. A Skyknight Legionnaire then ends the full game. The task checks ordered public events, object identity, post-event state, deterministic replay, and removal/scope/keyword mutations.

This is reference-only diagnostic evidence. It does not award card credit or Harbor reward; set coverage remains incomplete.
