//! Full-fidelity regression for Perilous Forays' sacrifice-and-search activation.

use cardbench_magic_engine::{
    AbilityActivation, CardType, Color, DecisionSelection, Game, GameEvent, ManaCost, PlayerId,
    PolicyAction, Zone,
};
use cardbench_magic_rav::{
    RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, rav_activated_ability_bindings,
    rav_additional_spell_cost_bindings, rav_basic_land_type_bindings, rav_mana_ability_bindings,
    rav_triggered_ability_bindings,
};

#[test]
fn perilous_forays_has_the_exact_activated_search_chassis() {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-PERILOUS-FORAYS")
        .expect("Perilous Forays definition exists");
    assert_eq!(definition.name, "Perilous Forays");
    assert_eq!(
        definition.mana_cost,
        ManaCost::with_colors(3, [Color::Green, Color::Green])
    );
    assert_eq!(
        definition.card_types,
        [CardType::Enchantment].into_iter().collect()
    );
    assert!(
        definition
            .supported_rules
            .contains(&"activated-sacrifice-creature-search-basic-land")
    );
    assert!(
        definition
            .supported_rules
            .contains(&"battlefield-tapped-land-entry")
    );
    assert!(
        definition
            .supported_rules
            .contains(&"policy-submitted-basic-land-search")
    );
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&definition.id));
}

#[test]
fn perilous_forays_pays_a_selected_creature_then_searches_and_shuffles() {
    let mut game = Game::new_with_all_bindings(
        card_definitions(),
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        rav_activated_ability_bindings(),
    )
    .expect("RAV fixture builds");
    let forays = game
        .add_card(PlayerId(0), "RAV-PERILOUS-FORAYS", Zone::Battlefield)
        .expect("Perilous Forays setup");
    let sacrificed = game
        .add_card(PlayerId(0), "RAV-WATCHWOLF", Zone::Battlefield)
        .expect("creature cost setup");
    let forest = game
        .add_card(PlayerId(0), "RAV-FOREST", Zone::Library)
        .expect("Forest setup");
    let opponents_plains = game
        .add_card(PlayerId(1), "RAV-PLAINS", Zone::Library)
        .expect("opponent library setup");
    game.grant_mana(PlayerId(0), Color::Green, 1)
        .expect("ability mana");

    game.activate_ability(
        PlayerId(0),
        AbilityActivation {
            source: forays,
            ability_id: "sacrifice-creature-search-basic-land",
            sacrifice_sources: vec![sacrificed],
            additional_tap_creatures: vec![],
            discard_cards: vec![],
            targets: vec![],
        },
    )
    .expect("Perilous Forays activation");

    assert_eq!(game.zone_of(sacrificed), Some(Zone::Graveyard));
    assert_eq!(game.stack.len(), 1, "ability must expose a response window");
    game.pass_priority(PlayerId(0)).expect("controller passes");
    game.pass_priority(PlayerId(1)).expect("opponent passes");
    let decision = game
        .view_for_player(PlayerId(0))
        .expect("controller view")
        .pending_decision
        .expect("private basic-land search choice");
    game.submit_policy_move(
        PlayerId(0),
        "perilous-forays-full-fidelity-test.v1",
        PolicyAction::SubmitDecision {
            decision: decision.id,
            selection: DecisionSelection::Objects(vec![forest]),
        },
    )
    .expect("controller chooses the available Forest");

    assert_eq!(game.zone_of(forest), Some(Zone::Battlefield));
    assert!(game.object(forest).expect("Forest persists").tapped);
    assert_eq!(game.zone_of(opponents_plains), Some(Zone::Library));
    assert_eq!(game.zone_of(forays), Some(Zone::Battlefield));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::SacrificedAsAbilityCost { player: PlayerId(0), permanent, .. }
            if *permanent == sacrificed
    )));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::LibrarySearchResolved {
            player: PlayerId(0),
            source,
            found: Some(card),
            ..
        } if *source == forays && *card == forest
    )));
    println!("Perilous Forays trace: {:?}", game.canonical_event_log());
    game.validate_invariants()
        .expect("sacrifice, search, stack, and event invariants hold");
}

#[test]
fn perilous_forays_land_entry_waits_for_its_ability_to_finish_before_triggering() {
    let mut game = Game::new_with_all_bindings_and_triggers(
        card_definitions(),
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        rav_activated_ability_bindings(),
        rav_triggered_ability_bindings(),
    )
    .expect("RAV trigger fixture builds");
    let forays = game
        .add_card(PlayerId(0), "RAV-PERILOUS-FORAYS", Zone::Battlefield)
        .expect("Perilous Forays setup");
    let stone_seeder = game
        .add_card(
            PlayerId(0),
            "RAV-STONE-SEEDER-HIEROPHANT",
            Zone::Battlefield,
        )
        .expect("Stone-Seeder setup");
    let sacrificed = game
        .add_card(PlayerId(0), "RAV-WATCHWOLF", Zone::Battlefield)
        .expect("creature cost setup");
    let forest = game
        .add_card(PlayerId(0), "RAV-FOREST", Zone::Library)
        .expect("Forest setup");
    game.grant_mana(PlayerId(0), Color::Green, 1)
        .expect("ability mana");

    game.activate_ability(
        PlayerId(0),
        AbilityActivation {
            source: forays,
            ability_id: "sacrifice-creature-search-basic-land",
            sacrifice_sources: vec![sacrificed],
            additional_tap_creatures: vec![],
            discard_cards: vec![],
            targets: vec![],
        },
    )
    .expect("Perilous Forays activation");
    game.pass_priority(PlayerId(0)).expect("controller passes");
    game.pass_priority(PlayerId(1)).expect("ability resolves");
    let decision = game
        .view_for_player(PlayerId(0))
        .expect("controller view")
        .pending_decision
        .expect("private basic-land search choice");
    game.submit_policy_move(
        PlayerId(0),
        "perilous-forays-landfall-order-test.v1",
        PolicyAction::SubmitDecision {
            decision: decision.id,
            selection: DecisionSelection::Objects(vec![forest]),
        },
    )
    .expect("controller chooses the available Forest");

    let ability_resolved = game
        .event_log
        .iter()
        .position(|event| {
            matches!(
                event,
                GameEvent::AbilityResolved { source, ability, .. }
                    if *source == forays && *ability == "sacrifice-creature-search-basic-land"
            )
        })
        .expect("ability terminal receipt");
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
    assert!(ability_resolved < trigger_stacked);
    assert_eq!(game.zone_of(forest), Some(Zone::Battlefield));
    assert_eq!(
        game.stack.len(),
        1,
        "land-entry trigger follows the ability"
    );
    game.validate_invariants()
        .expect("ability-originated land-entry trigger batch is fully flushed");
}
