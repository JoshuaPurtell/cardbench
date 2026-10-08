# Drift of Phantasms (RAV #46)

Implement the candidate-owned card module for the single RAV printing. Preserve the blue `{2}{U}` Creature — Spirit identity, 0/5 characteristics, Defender, Flying, and Transmute `{1}{U}{U}`.

The full-game authority casts a Drift of Phantasms, then uses another physical copy's Transmute at sorcery timing. It pays `{1}{U}{U}`, discards that source, privately chooses and reveals a different Drift with the same mana value, puts that object into hand, and shuffles. In combat, the opponent's Snapping Drake attacks and the Drift blocks it using Flying; Defender keeps the Drift out of the pilot's attacker declarations. The shuffled duel continues to a normal seat-zero win and deterministic replay.

The authority validates normalized source and literal AST before candidate calls, rejects a compile-valid injected side-effect function, and detects changes to printed cost, Defender, Flying, Transmute, and its activation cost. The Spirit subtype remains documented identity only because `CardDefinition` does not encode creature subtypes. The materializer exposes only the Drift module as writable and stubs trusted PvP binaries; credit and rewards remain zero.

See [source-note.md](source-note.md) for the printed-source authority and Oracle terminology reconciliation.
