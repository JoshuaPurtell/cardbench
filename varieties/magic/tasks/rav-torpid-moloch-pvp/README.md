# Torpid Moloch card task

Implement the candidate-owned Torpid Moloch definition and activated ability in `varieties/magic/sets/ravnica_city_of_guilds/src/cards/torpid_moloch.rs`.

Torpid Moloch (#147) is a red 3/2 creature that costs `{R}` and has Defender. Its no-mana activated ability sacrifices three lands to remove Defender from Torpid Moloch until end of turn. The deterministic full-game authority pays exactly one red mana to cast Moloch, then on seven later turns pays the three-land cost, resolves the temporary effect, legally attacks through an empty block, and reaches a seat-zero terminal win.

The authority checks live characteristics before and after the effect, ordered typed casting, land-sacrifice, activation, resolution, combat and expiry events, deterministic replay, and three compiled semantic mutants. This is a diagnostic-only vertical: 1 of 291 unique RAV card names, 306 printings, zero credited cards and zero reward. No provider trial is part of the task.

The task workspace exposes only the candidate card module. The provided Harbor bundle invokes the bounded scorer against the trusted reference authority.
