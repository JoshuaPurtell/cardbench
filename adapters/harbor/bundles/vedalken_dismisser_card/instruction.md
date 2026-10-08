Implement the candidate-owned Vedalken Dismisser module at `varieties/magic/sets/ravnica_city_of_guilds/src/cards/vedalken_dismisser.rs`.

Vedalken Dismisser is RAV #73, a blue 2/2 Creature costing `{5}{U}`. Define its mandatory enters-the-battlefield ability with the ID `etb-target-creature-owner-library-top`; it targets a creature and puts that exact permanent on top of its owner's library.

The isolated authority checks the exact Dismisser spell, the controller-private target selection for its mandatory trigger, the exact opposing creature object's owner and zone move, the owner's library top identity and size change, and matching source incarnation receipts. It continues Watchwolf combat to a terminal pilot win, replays deterministically, and checks five semantic gameplay mutations. Keep the module declarative and limited to the printed card definition and trigger. Do not add rules prose, art, scans, or flavor text.
