# RAV Boros Signet PvP task

Implement the candidate-owned Boros Signet definition and paid red-white mana binding. A legal 60-card game casts the Signet, pays its generic activation cost, taps it, uses its red and white bundle to cast Skyknight Legionnaire, checks the spell's actual colored payment and same-turn haste attack, and finishes with a terminal player-zero win. The authority checks deterministic replay and mutates the bundle to a wrong color pair.

The task workspace exposes only the Signet module and uses the shared Magic task workspace filter. Skyknight Legionnaire is a reviewed support card whose typed flying and haste characteristics are checked as part of the payment trace. The trace is a local Rust authority; no provider trial is run.
