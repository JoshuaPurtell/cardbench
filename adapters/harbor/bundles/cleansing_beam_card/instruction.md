Implement the candidate-owned Cleansing Beam definition at `varieties/magic/sets/ravnica_city_of_guilds/src/cards/cleansing_beam.rs`.

Cleansing Beam is RAV #118, a red `{4}{R}` Instant with Radiance: deal 2 damage to target creature and each other creature that shares a color with it. Preserve its red identity, exact cost, creature targeting, and Radiance damage effect.

The isolated authority begins with a white 2/2 target creature controlled by the opponent, another white 2/2 creature controlled by the pilot, and a blue 2/2 creature controlled by the opponent. It verifies that Cleansing Beam deals 2 damage to both white creatures, lethal-damage state-based actions put them into their owners’ graveyards, and the blue creature stays on the battlefield. It also checks exact payment, damage/SBA/move/resolution order, continuation to a terminal full-game result, deterministic replay, and gameplay mutations.

Keep the candidate module declarative and limited to the printed card definition. Do not add rules prose, art, scans, or flavor text.
