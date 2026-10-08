Implement the candidate-owned Keening Banshee module at `varieties/magic/sets/ravnica_city_of_guilds/src/cards/keening_banshee.rs`.

Keening Banshee is RAV #92, a black 2/2 Creature with Flying costing `{2}{B}{B}`. Define its mandatory enters-the-battlefield ability with ID `etb-target-minus-two`; it targets a creature and gives that exact target -2/-2 until end of turn.

The isolated authority checks the exact Banshee spell and target object, controller-only target selection, trigger/source identity and matching source incarnation at resolution, the layer-seven P/T effect and resulting target characteristics or state-based-action move, then follows Banshee combat to a terminal pilot win. It replays deterministically and checks five semantic gameplay mutations. Keep the module declarative and limited to the printed card definition and trigger. Do not add rules prose, art, scans, or flavor text.
