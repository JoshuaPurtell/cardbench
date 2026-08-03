//! Public trigger provenance and simultaneous-entry contract for Bloodbond March.

use cardbench_magic_engine::{CastRequest, Color, Game, GameEvent, PlayerId, Step, Zone};
use cardbench_magic_rav::{
    RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, rav_activated_ability_bindings,
    rav_additional_spell_cost_bindings, rav_attachment_bindings, rav_basic_land_type_bindings,
    rav_mana_ability_bindings,
    rav_static_entry_restriction_bindings, rav_triggered_ability_bindings,
};

fn game_with_rav_triggers() -> Game {
    let mut game = Game::new_with_all_bindings_and_triggers(
        card_definitions(),
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        rav_activated_ability_bindings(),
        rav_triggered_ability_bindings(),
    )
    .expect("RAV fixture constructs");
    game.register_static_entry_restriction_bindings(rav_static_entry_restriction_bindings())
        .expect("RAV entry restrictions register");
    game.register_attachment_bindings(rav_attachment_bindings())
        .expect("RAV attachment bindings register");
    game
}

fn pass_pair(game: &mut Game) {
    let first = game.priority;
    game.pass_priority(first).expect("first player passes");
    let second = game.priority;
    game.pass_priority(second).expect("second player passes");
}

fn advance_to_first_main(game: &mut Game) {
    game.begin_game().expect("fixture begins game");
    while game.step != Step::PrecombatMain {
        let player = game.priority;
        game.pass_priority(player)
            .expect("fixture priority advances toward main");
    }
    assert_eq!(game.priority, PlayerId(0));
}

#[test]
#[allow(clippy::too_many_lines)] // The public stack transcript remains one reviewable contract.
fn bloodbond_march_captures_opponent_creature_cast_and_returns_every_matching_graveyard_card() {
    let controller = PlayerId(1);
    let caster = PlayerId(0);
    let mut game = game_with_rav_triggers();
    let march = game
        .put_on_battlefield(controller, "RAV-BLOODBOND-MARCH")
        .expect("March battlefield setup");
    let controller_graveyard_watchwolf = game
        .add_card(controller, "RAV-WATCHWOLF", Zone::Graveyard)
        .expect("controller graveyard creature setup");
    let caster_graveyard_watchwolf = game
        .add_card(caster, "RAV-WATCHWOLF", Zone::Graveyard)
        .expect("caster graveyard creature setup");
    let unrelated_graveyard_creature = game
        .add_card(controller, "RAV-GLASS-GOLEM", Zone::Graveyard)
        .expect("unrelated graveyard creature setup");
    let cast_watchwolf = game
        .add_card(caster, "RAV-WATCHWOLF", Zone::Hand)
        .expect("creature spell setup");
    let forest = game
        .put_on_battlefield(caster, "RAV-FOREST")
        .expect("green mana source setup");
    let plains = game
        .put_on_battlefield(caster, "RAV-PLAINS")
        .expect("white mana source setup");
    game.clear_event_log();

    advance_to_first_main(&mut game);
    game.clear_event_log();
    game.activate_mana_ability(caster, forest, Color::Green)
        .expect("Forest produces green mana");
    game.activate_mana_ability(caster, plains, Color::White)
        .expect("Plains produces white mana");
    game.cast_spell(
        caster,
        CastRequest {
            card: cast_watchwolf,
            targets: vec![],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("opponent casts creature spell");

    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::TriggeredAbilityStacked { source, ability, .. }
            if *source == march
                && *ability == "any-player-creature-cast-return-matching-graveyard-creatures"
    )));
    pass_pair(&mut game);

    assert_eq!(
        game.zone_of(controller_graveyard_watchwolf),
        Some(Zone::Battlefield)
    );
    assert_eq!(
        game.zone_of(caster_graveyard_watchwolf),
        Some(Zone::Battlefield)
    );
    assert_eq!(
        game.zone_of(unrelated_graveyard_creature),
        Some(Zone::Graveyard)
    );
    assert!(
        game.stack
            .iter()
            .any(|stack_object| stack_object.card == cast_watchwolf),
        "the original creature spell remains below its triggered ability"
    );

    let cast = game
        .event_log
        .iter()
        .position(
            |event| matches!(event, GameEvent::SpellCast { card, .. } if *card == cast_watchwolf),
        )
        .expect("cast receipt");
    let trigger = game
        .event_log
        .iter()
        .position(|event| matches!(
            event,
            GameEvent::TriggeredAbilityStacked { source, ability, .. }
                if *source == march
                    && *ability == "any-player-creature-cast-return-matching-graveyard-creatures"
        ))
        .expect("trigger receipt");
    let first_return = game
        .event_log
        .iter()
        .position(|event| matches!(
            event,
            GameEvent::CardMoved { card, to: Zone::Battlefield }
                if *card == controller_graveyard_watchwolf || *card == caster_graveyard_watchwolf
        ))
        .expect("simultaneous return receipt");
    let resolved = game
        .event_log
        .iter()
        .position(|event| matches!(
            event,
            GameEvent::AbilityResolved { source, ability, .. }
                if *source == march
                    && *ability == "any-player-creature-cast-return-matching-graveyard-creatures"
        ))
        .expect("trigger resolution receipt");
    assert!(cast < trigger && trigger < first_return && first_return < resolved);

    pass_pair(&mut game);
    assert_eq!(game.zone_of(cast_watchwolf), Some(Zone::Battlefield));
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&"RAV-BLOODBOND-MARCH"));
    game.validate_invariants()
        .expect("Bloodbond March preserves stack and zone lifecycle invariants");
    eprintln!("bloodbond_march_trace={:?}", game.canonical_event_log());
}

#[test]
fn bloodbond_march_simultaneous_creature_entries_do_not_apply_newly_entered_gatekeepers() {
    let controller = PlayerId(1);
    let caster = PlayerId(0);
    let mut game = game_with_rav_triggers();
    game.put_on_battlefield(controller, "RAV-BLOODBOND-MARCH")
        .expect("March battlefield setup");
    let caster_graveyard_gatekeeper = game
        .add_card(caster, "RAV-LOXODON-GATEKEEPER", Zone::Graveyard)
        .expect("caster graveyard Gatekeeper setup");
    let controller_graveyard_gatekeeper = game
        .add_card(controller, "RAV-LOXODON-GATEKEEPER", Zone::Graveyard)
        .expect("controller graveyard Gatekeeper setup");
    let cast_gatekeeper = game
        .add_card(caster, "RAV-LOXODON-GATEKEEPER", Zone::Hand)
        .expect("creature spell setup");
    let plains = (0..6)
        .map(|_| {
            game.put_on_battlefield(caster, "RAV-PLAINS")
                .expect("white mana source setup")
        })
        .collect::<Vec<_>>();

    advance_to_first_main(&mut game);
    game.clear_event_log();
    for plain in plains {
        game.activate_mana_ability(caster, plain, Color::White)
            .expect("Plains produces white mana");
    }
    game.cast_spell(
        caster,
        CastRequest {
            card: cast_gatekeeper,
            targets: vec![],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("caster casts Gatekeeper");
    pass_pair(&mut game);

    for returned in [caster_graveyard_gatekeeper, controller_graveyard_gatekeeper] {
        assert_eq!(game.zone_of(returned), Some(Zone::Battlefield));
        assert!(
            !game
                .object(returned)
                .expect("returned Gatekeeper exists")
                .tapped,
            "a Gatekeeper entering simultaneously cannot make its peer enter tapped"
        );
        assert!(!game.event_log.iter().any(|event| matches!(
            event,
            GameEvent::PermanentEnteredTapped { permanent, .. } if *permanent == returned
        )));
    }
    game.validate_invariants()
        .expect("simultaneous entry uses only pre-event replacement sources");
}
