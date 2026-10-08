# Implement Birds of Paradise

Implement `varieties/magic/sets/ravnica_city_of_guilds/src/cards/birds_of_paradise.rs` as a literal-only module exporting `definition() -> CardDefinition` and `mana_ability() -> ManaAbilityBinding`.

Preserve RAV #153's `{G}` green creature identity, 0/1 body, Flying, and a tap-cost mana ability that adds one mana of any of the five colors. The trusted authority casts Birds in a shuffled 60-card PvP game, taps it for white while paying Watchwolf's `{G}{W}` cost with a typed Forest activation providing green, then continues through combat to a terminal win. It checks ordered typed events, deterministic replay, and five compiled semantic mutants. The result awards no card or set credit.

Change only the candidate module. The materialized workspace includes a compile-valid stub and the Harbor scorer runs the trusted reference authority against the submitted candidate.
