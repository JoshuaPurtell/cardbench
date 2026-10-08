# Implement Surveilling Sprite

Implement `varieties/magic/sets/ravnica_city_of_guilds/src/cards/surveilling_sprite.rs` as a literal-only module exporting `definition()` and `triggered_ability()`. Preserve RAV #67, the `{1}{U}` blue 1/1 creature, Flying, and its optional, target-free dies trigger that draws its controller a card.

The trusted reference authority runs two ordinary shuffled PvP games. Each pays exactly `{1}{U}` to cast Surveilling Sprite and attacks past a live Lurking Informant. The opponent pays `{2}{R}` for Char and deals four lethal damage to the Sprite. Verify its state-based death, graveyard movement, and dies-trigger stack order. In one game accept the optional draw and verify a card moves to hand before the ability resolves; in the other decline. Each game reaches a seat-zero win. Mutations cover Flying, cost, power, trigger presence, draw effect, and optionality. Do not modify files outside the module.
