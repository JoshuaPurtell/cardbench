Implement Torpid Moloch in `varieties/magic/sets/ravnica_city_of_guilds/src/cards/torpid_moloch.rs` by providing `definition()` and `activated_ability()`.

Torpid Moloch (#147) is a red 3/2 creature that costs `{R}` and has Defender. Its activated ability costs no mana and sacrifices three lands to remove Defender from itself until end of turn. The reference casts it with exactly one red mana, then on seven turns pays the three-land cost, resolves the effect, attacks legally and deals combat damage.

Change only the candidate module. The authority validates live characteristics, ordered typed payment, cast, entry, sacrifice-cost, activation, resolution, attack, damage, expiry, and terminal events; deterministic replay; and three compiled semantic mutants. The result is diagnostic-only at 1/291, with zero card credit and zero reward.
