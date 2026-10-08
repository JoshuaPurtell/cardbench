# Hunted Phantasm PvP task

Implement the candidate-owned Hunted Phantasm definition and its opponent-targeted enters-the-battlefield trigger in `varieties/magic/sets/ravnica_city_of_guilds/src/cards/hunted_phantasm.rs`.

## Source note

This task follows [`RAV_PVP_SOURCE_CONTRACT.md`](../rav-set-pvp/RAV_PVP_SOURCE_CONTRACT.md): the RAV printing controls public identity and the gameplay semantics under test; Oracle wording is consulted only to interpret rules questions. The checked [RAV #55 public record](https://www.mtg.wtf/card/rav/55/Hunted-Phantasm) confirms the identity, including the printed Spirit subtype. Current Oracle wording modernizes the entry and token-creation terminology but preserves the printed behavior tested here: a blue `{1}{U}{U}` 4/6 Creature that cannot be blocked, whose entry has a target opponent create five red 1/1 Goblins. No gameplay-changing reconciliation or excluded later erratum was found.

`Spirit` is retained as a public type-line fact in `features.toml`. The current `CardDefinition` model represents the Creature card type but has no creature-subtype field, so this task does not claim to assert Spirit in the executable definition or trace.

The reference authority validates the complete candidate source before calling either candidate function. It runs a deterministic, legal two-player, 60-card game and records the exact `{1}{U}{U}` payment, blue 4/6 creature characteristics, unblockability, the controller's submitted opponent-only ETB target decision, and five red 1/1 Goblin tokens created under that opponent's control while both players remain alive. The projected trace binds the target-choice move to the trigger controller's seat and records the controller's policy receipt. It then demonstrates Hunted Phantasm attacking and dealing player damage on a later turn through normal combat before the game reaches a terminal win. The complete projected trace is replayed deterministically.

Gameplay mutations cover the card's body, unblockability, mana cost, ETB condition, opponent-only target, Goblin characteristics, and token count. A compile-valid injected filesystem-write candidate is rejected by exact-source and AST validation before candidate functions execute.

This is an uncredited diagnostic against the 291-name / 306-printing RAV catalog. It reports zero card credit and zero reward. Candidate trials and provider-backed runs are outside this task.
