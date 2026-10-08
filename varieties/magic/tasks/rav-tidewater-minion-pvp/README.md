# RAV Tidewater Minion single-card diagnostic

This task targets Tidewater Minion (Ravnica: City of Guilds collector #71): a blue `{3}{U}{U}` 4/4 Creature — Elemental Minion with Defender. Its two activated abilities are independent: `{T}: Untap target permanent.` and `{4}: Tidewater Minion loses defender until end of turn.`

The bounded reference uses shuffled 60-card PvP decks. The pilot casts Tidewater Minion; the opponent casts and taps a Boros Signet so Tidewater Minion's tap ability can target that opposing nonland permanent. The trace records the tap cost, target on the activated stack, exact untap state change, source incarnation, and resolution order. It then pays the printed generic `{4}` cost, removes Defender until end of turn, attacks, and wins the complete duel. It checks replay and gameplay/source mutations.

The candidate source and AST guard runs before any candidate factory call. A parse-valid, type-correct side-effect mutation must be rejected before candidate evaluation. The Plant-like printed subtype data is represented here as Elemental Minion; the current `CardDefinition` type cannot assert creature subtypes, so executable state records Creature only and task metadata retains both subtypes.

All report rewards and credited-card counts are zero. This local diagnostic does not complete set coverage or grant Harbor qualification. The editable module is `varieties/magic/sets/ravnica_city_of_guilds/src/cards/tidewater_minion.rs`; it exposes `definition()`, `untap_target_permanent_ability()`, and `lose_defender_ability()`.

The rules baseline is the Wizards Comprehensive Rules effective August 7, 2026, with authority cutoff September 24, 2026. See [source-note.md](source-note.md) for the printed/current wording reconciliation and source links.
