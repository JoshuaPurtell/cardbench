# Lore Broker RAV PvP task

Implement Lore Broker (#57) in the candidate-owned module at `varieties/magic/sets/ravnica_city_of_guilds/src/cards/lore_broker.rs`. The trusted authority validates the candidate's normalized source and literal AST before calling either candidate function. It then runs and replays a deterministic, legal two-player 60-card game.

The trace checks the blue `{1}{U}` 1/2 creature, its tap-only activated ability, and a full-game resolution in which both players draw before either privately chooses a card from their own hand to discard. It checks that the other player cannot see the pending choice or its candidate identities, and that both public discard receipts follow both choices. Goliath Spider supplies ordinary later-turn combat and a terminal win.

The source note follows [`RAV_PVP_SOURCE_CONTRACT.md`](../rav-set-pvp/RAV_PVP_SOURCE_CONTRACT.md): the RAV printing controls public identity and tested semantics; current Oracle wording is consulted only to interpret rules. The printed reference calls for each player to draw a card and then discard a card. Current Oracle wording uses modern pronouns while preserving the same hand-discard operation. The engine definition schema has no creature-subtype field, so Human Rogue is recorded as printed identity but is outside this compatibility slice.

Semantic mutations exercise the body, spell cost, tap cost, and draw/discard effect. A compile-valid injected filesystem-write statement is rejected by the source/AST gate before candidate functions execute. This is an uncredited diagnostic against the 291-name / 306-printing RAV catalog: it reports zero card credit, zero reward, and no provider-backed trials.
