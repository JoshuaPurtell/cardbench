# RAV Leashling card task

Implement the candidate-owned Leashling module at `sets/ravnica_city_of_guilds/src/cards/leashling.rs`. Keep its definition, activated ability, and generalized hand-card cost binding in that module.

The trusted authority runs two legal, shuffled 60-card two-player games to a terminal result. The first casts Leashling for six mana, attacks for three, puts a real hand card on its owner's library top as an activation cost, and checks that the exact source returns to its owner's hand. The second repeats the activation, lets Last Gasp resolve in the response window, and checks that the old ability does not return the source after it has left the battlefield. Both traces check ordered typed events and deterministic replay. Compiled semantic mutations remove the hand-card cost, source-return effect, or 3/3 body.

The source contract follows the RAV printing represented in the catalog and `RAV_PVP_SOURCE_CONTRACT.md`. The task retains public identity facts and CardBench-authored semantic operations only. Rules baseline: Comprehensive Rules effective August 7, 2026, as of the September 23, 2026 authority cutoff. Local materializer and scorer checks do not run Harbor or provider trials.
