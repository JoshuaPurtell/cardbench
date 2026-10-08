Implement the candidate-owned Tidewater Minion module at `varieties/magic/sets/ravnica_city_of_guilds/src/cards/tidewater_minion.rs`. Preserve the three factory APIs: `definition()`, `untap_target_permanent_ability()`, and `lose_defender_ability()`.

Tidewater Minion is RAV #71, a blue `{3}{U}{U}` 4/4 Creature — Elemental Minion with Defender. Its printed abilities are `{T}: Untap target permanent.` and `{4}: Tidewater Minion loses defender until end of turn.` The second ability costs generic four, not blue mana. The current `CardDefinition` cannot represent subtypes, so Elemental Minion remains in task metadata.

The authority uses a full shuffled 60-card PvP duel. It casts Tidewater Minion and targets the opponent's tapped Boros Signet with its tap ability, then verifies that exact target appears on the stack and becomes untapped. It pays `{4}` to remove Defender, attacks for four damage, confirms the temporary effect expires, and continues to a terminal Player 0 win. Replay and mutations check the printed costs, body, target scope, effects, both bindings, and source-AST rejection of compile-valid side effects.

Do not change files outside the declared candidate module. This local pilot reports zero reward and zero card credit; independent review and Harbor qualification remain separate.
