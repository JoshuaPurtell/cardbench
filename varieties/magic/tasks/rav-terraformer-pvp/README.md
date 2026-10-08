# Terraformer full-game card task

Implement the candidate-owned Terraformer module at `varieties/magic/sets/ravnica_city_of_guilds/src/cards/terraformer.rs`. Terraformer is RAV collector #70, a blue 2/2 Creature costing `{2}{U}` with “{1}: Each land you control becomes a basic land type of your choice until end of turn.”

The local authority runs a shuffled 60-card PvP duel. It casts Terraformer, activates it on the opponent's precombat main phase, proves that only the controller's lands become Forests and that a former Island produces Green, then activates it again on the controller's precombat main phase. The second conversion supplies all four Green mana to cast Greater Mossdog for `{3}{G}`. The trace checks both exact `{1}` payments, source and incarnation identity, each per-land type-layer effect and end-of-turn expiration, restoration of both players' lands, the converted Island mana receipts, the Mossdog spell and terminal combat win. The game is replayed deterministically and six semantic gameplay mutations must be detected.

This is diagnostic evidence for one identity among 291 RAV card names and 306 printings. It grants zero card credit and zero reward. No Harbor/provider candidate trial is part of local validation. Materialize with `materialize_terraformer_task.py --workspace PATH --manifest PATH`; verify using the same command plus `--verify`.
