# Task

Implement the candidate-owned `RAV-COPY-ENCHANTMENT` card definition in `varieties/magic/sets/ravnica_city_of_guilds/src/cards/copy_enchantment.rs`.

RAV #42 is a blue `{2}{U}` Enchantment. On entry its controller may choose an enchantment already on the battlefield; if chosen, it enters as a copy. The set's `EntryCopyBinding` and the engine's Aura attachment choice are part of the reviewed runtime contract. Preserve the public definition identity and supported-rules tags. Do not change task files, authority, registry, engine, or any other source.

The trusted scorer validates the normal PvP trace, selected and declined decisions, copied-Aura attachment and ETB trigger, terminal result, replay, mutation sensitivity, and source guard. No rewards or card credits are issued.
