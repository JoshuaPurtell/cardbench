# Implement Shambling Shell

Implement `varieties/magic/sets/ravnica_city_of_guilds/src/cards/shambling_shell.rs` with `definition()` and the candidate-owned activated ability. Match RAV #230: black-green `{1}{B}{G}`, 3/1 Creature — Plant Zombie, sacrifice Shambling Shell to put a +1/+1 counter on target creature, and Dredge 3. `CardDefinition` lacks creature subtype fields; preserve Plant and Zombie as printed identity without claiming they are modeled characteristics.

Keep all Shambling Shell-specific runtime behavior in this candidate-owned module. The authority runs a legal deterministic PvP game, verifies the source is sacrificed as the activation cost and the exact target receives one counter, then replaces an actual draw with Dredge 3 and checks the exact three milled objects and return of the same Shell object to hand. It also checks replay, semantic mutations, and a pre-call normalized-source/literal-AST guard, including rejection of a compile-valid injected side effect.
