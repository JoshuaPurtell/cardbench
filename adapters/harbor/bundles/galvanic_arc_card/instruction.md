Implement the public RAV Galvanic Arc card module at `varieties/magic/sets/ravnica_city_of_guilds/src/cards/galvanic_arc.rs`.

Preserve the original RAV #126 printing: red `{2}{R}` Enchantment — Aura, `Enchant creature`, mandatory ETB 3 damage to target creature or player, and First Strike granted to the enchanted creature. The engine's typed card model stores Enchantment plus Aura attachment behavior; it cannot represent the Aura subtype independently. Current Oracle wording says “any target”; the RAV task intentionally retains the printed creature-or-player target boundary.

Keep the definition, ETB trigger, and Aura attachment behavior in the candidate module. Do not add executable top-level side effects. The trusted source guard pins the normalized literal module and rejects a compile-valid side-effect injection before candidate functions are called. This task is diagnostic and does not award set credit or reward.
