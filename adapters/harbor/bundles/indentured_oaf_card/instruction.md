# Task

Implement only `varieties/magic/sets/ravnica_city_of_guilds/src/cards/indentured_oaf.rs` for RAV #133. Keep the RAV printed source identity and prevention direction. The oracle is source authority only for resolving the current wording; it does not replace the printed text or identity.

The trusted authority validates the exact candidate source and AST before calling the candidate factory. It then runs a normal shuffled 60-card duel and checks the printed cost, red 4/3 creature, red hybrid payment for Boros Recruit, Recruit's one First Strike damage to Oaf, four prevented damage from Oaf to Recruit, the surviving undamaged blocker, replay, and semantic mutations. Do not modify task tools, authority, registry, engine, tests, task metadata, or any other file.
