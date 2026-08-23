//! Red regression for Farseek's typed land search slice.

use cardbench_magic_engine::{
    CardType, CastRequest, Color, DecisionSelection, Game, GameEvent, PlayerId, PolicyAction, Zone,
};
use cardbench_magic_rav::{
    card_definitions, rav_activated_ability_bindings, rav_additional_spell_cost_bindings,
    rav_basic_land_type_bindings, rav_land_entry_bindings, rav_mana_ability_bindings,
    rav_static_continuous_effect_bindings, rav_triggered_ability_bindings,
};

#[test]
fn farseek_has_its_exact_supported_casting_chassis() {
    let farseek = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-FARSEEK")
        .expect("Farseek definition exists");
    assert_eq!(farseek.name, "Farseek");
    assert_eq!(farseek.mana_cost.generic, 1);
    assert_eq!(farseek.mana_cost.colored, vec![Color::Green]);
    assert_eq!(
        farseek.card_types,
        [CardType::Sorcery].into_iter().collect()
    );
    assert!(
        farseek
            .supported_rules
            .contains(&"library-nonforest-land-type-search")
    );
    assert!(
        farseek
            .supported_rules
            .contains(&"battlefield-tapped-land-entry")
    );
}

#[test]
fn farseek_returns_only_a_controller_owned_nonforest_typed_land_tapped() {
    let mut game =
        Game::new_with_basic_land_types(card_definitions(), 2, rav_basic_land_type_bindings())
            .expect("RAV fixture builds with typed land lines");
    let farseek = game
        .add_card(PlayerId(0), "RAV-FARSEEK", Zone::Hand)
        .expect("Farseek setup");
    let forest = game
        .add_card(PlayerId(0), "RAV-FOREST", Zone::Library)
        .expect("Forest setup");
    let plains = game
        .add_card(PlayerId(0), "RAV-PLAINS", Zone::Library)
        .expect("Plains setup");
    let opponents_island = game
        .add_card(PlayerId(1), "RAV-ISLAND", Zone::Library)
        .expect("opponent Island setup");
    game.grant_mana(PlayerId(0), Color::Green, 2)
        .expect("Farseek mana");

    game.cast_spell(
        PlayerId(0),
        CastRequest {
            card: farseek,
            targets: vec![],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("Farseek casts");
    game.pass_priority(PlayerId(0)).expect("caster passes");
    game.pass_priority(PlayerId(1))
        .expect("opponent passes and opens the private search choice");
    let decision = game
        .view_for_player(PlayerId(0))
        .expect("controller view")
        .pending_decision
        .expect("Farseek search choice is pending");
    game.submit_policy_move(
        PlayerId(0),
        "test.farseek.select-plains.v1",
        PolicyAction::SubmitDecision {
            decision: decision.id,
            selection: DecisionSelection::Objects(vec![plains]),
        },
    )
    .expect("controller selects Plains");

    assert_eq!(game.zone_of(plains), Some(Zone::Battlefield));
    assert!(game.object(plains).expect("Plains persists").tapped);
    assert_eq!(game.zone_of(forest), Some(Zone::Library));
    assert_eq!(game.zone_of(opponents_island), Some(Zone::Library));
    assert_eq!(game.zone_of(farseek), Some(Zone::Graveyard));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::LibraryShuffled {
            player: PlayerId(0),
            cards: 1,
        }
    )));
    println!("Farseek trace: {:?}", game.canonical_event_log());
    game.validate_invariants()
        .expect("Farseek preserves zone, stack, and event invariants");
}

#[test]
#[allow(clippy::too_many_lines)] // Spell, land entry, and delayed trigger ordering form one transcript.
fn farseek_land_entry_waits_for_its_spell_to_finish_before_triggering() {
    let mut game = Game::new_with_all_bindings_triggers_static_continuous_effects_and_land_entries(
        card_definitions(),
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        rav_activated_ability_bindings(),
        rav_triggered_ability_bindings(),
        rav_static_continuous_effect_bindings(),
        rav_land_entry_bindings(),
    )
    .expect("full RAV fixture builds");
    let stone_seeder = game
        .add_card(
            PlayerId(0),
            "RAV-STONE-SEEDER-HIEROPHANT",
            Zone::Battlefield,
        )
        .expect("Stone-Seeder setup");
    let farseek = game
        .add_card(PlayerId(0), "RAV-FARSEEK", Zone::Hand)
        .expect("Farseek setup");
    let plains = game
        .add_card(PlayerId(0), "RAV-PLAINS", Zone::Library)
        .expect("Plains setup");
    game.grant_mana(PlayerId(0), Color::Green, 2)
        .expect("Farseek mana");

    game.cast_spell(
        PlayerId(0),
        CastRequest {
            card: farseek,
            targets: vec![],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("Farseek casts");
    game.pass_priority(PlayerId(0)).expect("caster passes");
    game.pass_priority(PlayerId(1))
        .expect("Farseek opens its private search choice");
    let decision = game
        .view_for_player(PlayerId(0))
        .expect("controller view")
        .pending_decision
        .expect("Farseek search choice is pending");
    game.submit_policy_move(
        PlayerId(0),
        "test.farseek.select-plains-for-landfall.v1",
        PolicyAction::SubmitDecision {
            decision: decision.id,
            selection: DecisionSelection::Objects(vec![plains]),
        },
    )
    .expect("controller selects Plains");

    let spell_resolved = game
        .event_log
        .iter()
        .position(|event| matches!(event, GameEvent::SpellResolved { card } if *card == farseek))
        .expect("Farseek resolution receipt");
    let spell_to_graveyard = game
        .event_log
        .iter()
        .position(|event| matches!(event, GameEvent::CardMoved { card, to: Zone::Graveyard } if *card == farseek))
        .expect("Farseek terminal zone receipt");
    let trigger_stacked = game
        .event_log
        .iter()
        .position(|event| {
            matches!(
                event,
                GameEvent::TriggeredAbilityStacked { source, ability, .. }
                    if *source == stone_seeder && *ability == "landfall-untap-source"
            )
        })
        .expect("Stone-Seeder trigger receipt");
    assert!(spell_resolved < spell_to_graveyard);
    assert!(spell_to_graveyard < trigger_stacked);
    assert_eq!(game.zone_of(plains), Some(Zone::Battlefield));
    assert_eq!(
        game.stack.len(),
        1,
        "the land-entry trigger is stack-backed"
    );

    game.pass_priority(PlayerId(0))
        .expect("controller passes trigger");
    game.pass_priority(PlayerId(1))
        .expect("opponent resolves trigger");
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::AbilityResolved { source, ability, .. }
            if *source == stone_seeder && *ability == "landfall-untap-source"
    )));
    game.validate_invariants()
        .expect("deferred land-entry trigger batch is fully flushed");
}

#[test]
fn farseek_respects_current_turn_library_search_prevention() {
    let mut game =
        Game::new_with_basic_land_types(card_definitions(), 2, rav_basic_land_type_bindings())
            .expect("RAV fixture builds with typed land lines");
    let shadow = game
        .add_card(PlayerId(0), "RAV-SHADOW-OF-DOUBT", Zone::Hand)
        .expect("Shadow setup");
    let farseek = game
        .add_card(PlayerId(0), "RAV-FARSEEK", Zone::Hand)
        .expect("Farseek setup");
    let plains = game
        .add_card(PlayerId(0), "RAV-PLAINS", Zone::Library)
        .expect("Plains setup");
    let shadow_draw = game
        .add_card(PlayerId(0), "RAV-FOREST", Zone::Library)
        .expect("Shadow draw setup");
    game.grant_mana(PlayerId(0), Color::Blue, 1)
        .expect("Shadow mana");
    game.grant_mana(PlayerId(0), Color::Black, 1)
        .expect("Shadow mana");
    game.grant_mana(PlayerId(0), Color::Green, 2)
        .expect("Farseek mana");

    game.cast_spell(
        PlayerId(0),
        CastRequest {
            card: shadow,
            targets: vec![],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("Shadow casts");
    game.pass_priority(PlayerId(0))
        .expect("caster passes Shadow");
    game.pass_priority(PlayerId(1)).expect("Shadow resolves");
    game.cast_spell(
        PlayerId(0),
        CastRequest {
            card: farseek,
            targets: vec![],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("Farseek still casts through prevention");
    game.pass_priority(PlayerId(0))
        .expect("caster passes Farseek");
    game.pass_priority(PlayerId(1))
        .expect("Farseek resolves without searching");

    assert_eq!(game.zone_of(plains), Some(Zone::Library));
    assert_eq!(game.zone_of(shadow_draw), Some(Zone::Hand));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::LibrarySearchResolved {
            player: PlayerId(0),
            source,
            found: None,
            ..
        } if *source == farseek
    )));
    game.validate_invariants()
        .expect("search prevention remains compatible with Farseek resolution");
}
