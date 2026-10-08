# Galvanic Arc RAV card task

Implement candidate-owned Galvanic Arc for RAV #126. The RAV printing is a red `{2}{R}` Enchantment — Aura with `Enchant creature`; when it enters, it deals 3 damage to target creature or player; the enchanted creature has First Strike. The module exposes the definition, mandatory `etb-deal-three` trigger, and Aura attachment binding. The engine models Aura identity as an Enchantment plus typed attachment behavior; `CardDefinition` cannot retain the printed Aura subtype separately.

The authority substitutes the candidate definition and bindings into a normal shuffled 60-card duel. Seat zero casts and attaches Galvanic Arc to its Watchwolf, targets the opposing Watchwolf with the mandatory trigger, proves exactly 3 damage and the target's graveyard move, then verifies the attached Watchwolf's derived First Strike during combat. A lethal first-strike event removes the other Watchwolf before normal combat damage; seat zero must win the full game. The report requires deterministic replay and gameplay mutations for damage, trigger presence, First Strike, and the exact mana cost. A pinned source/AST check runs before candidate functions; a compile-valid side-effect injection must reject before any candidate call.

The task remains diagnostic. It awards no card coverage or Harbor reward; every reward and credit field is zero.
