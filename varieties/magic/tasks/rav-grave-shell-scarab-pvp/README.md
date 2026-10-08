# Grave-Shell Scarab RAV card task

Implement the candidate-owned module for Grave-Shell Scarab, Ravnica: City of Guilds #211: a black-green `{2}{B}{G}{G}` 4/4 creature with Dredge 1 and the activated ability `{1}, Sacrifice this creature: Draw a card.`

The trusted authority checks the direct-literal card and ability builders, then runs shuffled 60-card decks through a two-player game. It casts one Scarab, pays the generic activation cost, sacrifices that exact object, chooses it for Dredge 1 to replace the activated ability's draw, verifies the exact one-card mill and return, and recasts that same object. A second activation declines Dredge and must draw normally. The match continues through combat to a terminal pilot win; deterministic replay and semantic mutations are also checked.

Only `varieties/magic/sets/ravnica_city_of_guilds/src/cards/grave_shell_scarab.rs` is candidate-editable. The authority, scorer, materializer, task metadata, and Harbor bundle remain trusted. This diagnostic currently grants zero card credit and zero reward pending independent qualification.
