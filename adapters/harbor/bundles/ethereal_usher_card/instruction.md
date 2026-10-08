# Implement Ethereal Usher

Implement `varieties/magic/sets/ravnica_city_of_guilds/src/cards/ethereal_usher.rs` with `definition()` and `activated_ability()`. Match RAV #47: blue `{5}{U}`, 2/3 Creature — Spirit, `{U}, {T}: Target creature is unblockable this turn`, and Transmute `{1}{U}{U}`. `CardDefinition` lacks a subtype field; preserve Spirit as source identity without claiming it as a modeled characteristic.

The authority uses shuffled legal 60-card decks, activates Transmute at sorcery timing and verifies the hidden choice, reveal, destination, and shuffle; casts an Usher; pays and taps to target the pilot's Watchwolf; rejects a real opposing block on that unblockable attacker; checks duration expiry; and continues to a deterministic terminal pilot win. The source gate checks normalized source plus literal AST before candidate calls and rejects a compile-valid side-effect fixture.
