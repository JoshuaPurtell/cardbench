# Vitu-Ghazi, the City-Tree PvP card task

Implement Vitu-Ghazi, the City-Tree in `varieties/magic/sets/ravnica_city_of_guilds/src/cards/vitu_ghazi.rs`. Keep the card definition, colorless mana ability, and token ability declarative and candidate-owned.

The shuffled, legal 60-card PvP trace uses one copy across two turns. It taps for exactly one colorless mana, which pays one generic mana of a five-generic fixture spell. After untapping, the same land pays `{2}{G}{W}` and its tap cost for a stack-backed ability. That ability resolves and creates a green 1/1 Saproling under its controller's control. The trace continues through a Saproling attack and a terminal seat-zero win.

The authority checks the 291-name/306-printing RAV denominator, typed projected events and their order, live token characteristics, distinct source identities, deterministic replay, and compiled semantic mutations for wrong mana color, a missing tap cost, and missing token creation. This report is diagnostic-only: no reference receipt or candidate credit is awarded here.
