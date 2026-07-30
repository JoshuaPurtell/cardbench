# Pokémon match visualization

`render_frame.py` converts a normalized state snapshot into a deterministic
256×160 PNG. The palette and geometry are deliberately low-fi: active slots,
five bench slots, prize/deck/discard counters, HP bars, energy pips, and turn
ownership.

The source of truth is benchmark state/event data. Frames are evidence only;
scores never depend on image pixels.
