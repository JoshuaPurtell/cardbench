Implement the candidate-owned Seed Spark definition at `varieties/magic/sets/ravnica_city_of_guilds/src/cards/seed_spark.rs`.

Seed Spark is RAV #30, a white `{3}{W}` Instant. Its first instruction destroys target artifact or enchantment. Its second instruction creates two green 1/1 Saproling creature tokens. Preserve this order and the exact token characteristics.

The isolated authority uses a shuffled 60-card PvP duel. It checks the exact opposing Boros Signet target and destruction source, its move to the graveyard, the two token creation receipts before Seed Spark's resolution receipt, subsequent Saproling combat, a terminal result, deterministic replay, and semantic gameplay mutants. Keep the candidate module declarative and limited to the printed card definition. Do not add rules prose, art, scans, or flavor text.
