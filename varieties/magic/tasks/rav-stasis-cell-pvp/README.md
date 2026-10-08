# RAV Stasis Cell PvP task

Implement the RAV #66 Stasis Cell semantics in its candidate module. The public card contract is `{4}{U}` for an Aura that enchants a creature and makes that creature unable to attack or block. Its activated ability costs `{3}{U}` and attaches Stasis Cell to a target creature. The target can still use its nonmana activated abilities.

The trusted full-game authority must use legal shuffled 60-card decks. It verifies actual colored payments, Aura entry and attachment, the target's attack/block restrictions, an activated ability remaining usable while enchanted, and a `{3}{U}` reattachment that moves the restriction to the new creature. It also verifies the complete match reaches a P0 win and replays deterministically. The authority pins the literal module and checks its AST before it calls candidate functions; a compile-valid side-effect injection must be rejected before evaluation.

The source reconciliation in [source-note.md](source-note.md) documents that the existing shared inline approximation suppressed nonmana activations and omitted the blue reattachment symbol. Both differ from the RAV card text and are corrected by this candidate-owned definition and bindings. This is a local diagnostic task; it does not award provider reward, Harbor credit, or set coverage.
