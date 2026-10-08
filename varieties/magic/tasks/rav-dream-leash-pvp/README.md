# Dream Leash (RAV #45)

Implement the candidate-owned module for Dream Leash's single RAV printing. Preserve the blue `{3}{U}{U}` Enchantment — Aura identity, `Enchant permanent`, the cast-time requirement that its target is tapped, and control of the enchanted permanent.

The authority plays a legal two-player game. The opponent attacks with Watchwolf, tapping it; the pilot casts Dream Leash on that exact Watchwolf; and Tidewater Minion then untaps it while Dream Leash remains on the stack. The Aura must resolve and attach despite the target being untapped, take control of it, and remain attached. The duel reaches a normal seat-zero win and a deterministic replay.

Normalized candidate source and its literal AST shape are checked before candidate calls. A compile-valid injected side-effect function is rejected, and the untapped-target, control-effect, and mana-cost mutations must be detected. The materializer permits changes only to the Dream Leash module and stubs trusted PvP binaries in the public workspace. No card is credited; all rewards remain zero.

See [source-note.md](source-note.md) for printed-source authority and Oracle reconciliation.
