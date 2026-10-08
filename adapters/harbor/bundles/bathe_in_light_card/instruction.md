# Implement Bathe in Light

Implement `varieties/magic/sets/ravnica_city_of_guilds/src/cards/bathe_in_light.rs` as a literal-only module exporting `definition() -> CardDefinition`. Preserve RAV #2: Bathe in Light is a white `{1}{W}` instant with a creature target and chosen color; its Radiance protection applies to the target and every other creature sharing at least one color with that target until end of turn.

The trusted authority runs a shuffled 60-card PvP game. It checks the exact recipient set across both controllers, confirms colorless and nonmatching creatures are excluded, verifies protection rejects Lightning Helix aimed at a matching-color creature while allowing it to target a nonmatching blue creature, then continues to a Watchwolf combat win. Keep the module declarative and do not add other files.
