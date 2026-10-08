# Sewerdreg card task

Implement Sewerdreg (#104) in `varieties/magic/sets/ravnica_city_of_guilds/src/cards/sewerdreg.rs` by providing the candidate-owned `definition()` and `activated_ability()` functions.

Sewerdreg is a black 3/3 creature that costs `{3}{B}{B}`, has Fear, and has `{B}: Regenerate Sewerdreg`. The deterministic full-game trace pays the exact five-black spell cost, responds to Char with the no-target self-regeneration ability, verifies that the shield saves Sewerdreg from lethal damage, then attacks through a nonblack nonartifact Elves of Deep Shadow. The authority probes and rejects the illegal Fear block without changing game state, submits a legal empty blocker declaration, and checks combat damage and a terminal seat-zero win.

The reference report is diagnostic-only: 1 of 291 unique RAV names from 306 printings is trace-passed, zero cards are credited, and reward is zero. It includes a deterministic replay and compiled wrong-color regeneration and Fear-removal mutants. This package does not run a provider trial.
