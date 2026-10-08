Implement `varieties/magic/sets/ravnica_city_of_guilds/src/cards/loxodon_hierarch.rs` as a declarative candidate module exposing `definition()`, `triggered_ability()`, and `activated_ability()`.

Required RAV source behavior:

- Loxodon Hierarch, RAV #214, costs `{2}{G}{W}`, is white and green, and is a 4/4 creature.
- When it enters the battlefield, its mandatory trigger gains its controller 4 life.
- `{G}{W}`, sacrifice Loxodon Hierarch: regenerate each creature you control.
- The activation has no tap cost, no targets, and no sorcery-speed restriction.

The trace checks the paid Green and White cost, source sacrifice, resolution-time team scope, shield use against lethal Char damage, a full-game Player 0 win, replay, and semantic mutations. The printed Creature — Elephant Cleric subtype line is recorded in task metadata; the current engine `CardDefinition` cannot represent subtypes. Preserve only the candidate module API and avoid changes outside the target file.
