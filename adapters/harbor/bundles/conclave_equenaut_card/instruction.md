# CardBench Magic single-card task: Conclave Equenaut

Implement Conclave Equenaut in `varieties/magic/sets/ravnica_city_of_guilds/src/cards/conclave_equenaut.rs` by returning the correct declarative `CardDefinition`.

Conclave Equenaut is a white 3/3 Creature with mana cost `{4}{W}{W}` and the Convoke and Flying keywords. The full-game trace casts it from a shuffled 60-card deck. Four distinct untapped creatures, drawn from Birds of Paradise and Watchwolf playsets, each contribute one generic mana through Convoke; two White mana pay the exact colored symbols. Once it resolves, Equenaut attacks while an opposing ground Watchwolf is present. The engine rejects an attempted ground block under Flying, then accepts the legal empty blocker declaration and records three combat damage.

The authority checks ordered payment, cast, resolution, entry, attack, blocker, damage, and game-end events; live characteristics; deterministic replay; and semantic controls for Convoke, cost, and Flying. It reports one passed diagnostic card out of 291 names and 306 printings, with zero card credit and zero reward.
