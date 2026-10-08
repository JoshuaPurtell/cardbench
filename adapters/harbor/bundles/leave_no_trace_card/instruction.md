Implement the candidate-owned Leave No Trace definition at `varieties/magic/sets/ravnica_city_of_guilds/src/cards/leave_no_trace.rs`.

Leave No Trace is RAV #23, a white `{1}{W}` Instant with Radiance: destroy target enchantment and each other enchantment that shares a color with it. Preserve its white identity, exact cost, typed enchantment target, and Radiance effect.

The isolated authority begins with a white target enchantment controlled by the opponent, another white enchantment controlled by the pilot, and a blue enchantment controlled by the opponent. It verifies that both white enchantments are destroyed and moved to their owners' graveyards while the blue enchantment stays on the battlefield. It also checks the exact payment, destruction and resolution order, continuation to a terminal full-game result, deterministic replay, and gameplay mutations.

Keep the candidate module declarative and limited to the printed card definition. Do not add rules prose, art, scans, or flavor text.
