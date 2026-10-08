Implement the candidate-owned Bramble Elemental module at `varieties/magic/sets/ravnica_city_of_guilds/src/cards/bramble_elemental.rs`.

Keep the module to the public card identity and its triggered-ability binding. The trace tests that an Aura attaching to Bramble Elemental creates an optional trigger controlled by Bramble's controller, even when another player controls the Aura. An Aura attached to a different creature must not trigger Bramble. Accepting the trigger creates exactly two green 1/1 Saproling creatures.

The isolated authority also checks the green `{3}{G}{G}` cost, the 4/4 body, exact source/attachment identities, the terminal full-game result, deterministic replay, and gameplay mutations. Do not add rules text, card scans, art, or flavor content.
