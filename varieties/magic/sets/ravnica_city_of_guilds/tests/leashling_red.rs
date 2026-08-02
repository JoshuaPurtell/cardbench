//! Red discovery contract for Leashling's hand-to-library activation cost.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    AbilityActivation, AbilityCostPayment, CardType, CastRequest, Color, Game, GameEvent,
    GeneralizedAbilityActivation, ManaCost, PlayerId, PolicyAction, Target, Zone,
};
use cardbench_magic_rav::{
    RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, rav_activated_ability_bindings,
    rav_additional_spell_cost_bindings, rav_basic_land_type_bindings,
    rav_generalized_activated_ability_cost_bindings, rav_mana_ability_bindings,
};

fn game() -> Game {
    let mut game = Game::new_with_all_bindings(
        card_definitions(),
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        rav_activated_ability_bindings(),
    )
    .expect("RAV game builds");
    game.register_generalized_activated_ability_cost_bindings(
        rav_generalized_activated_ability_cost_bindings(),
    )
    .expect("Leashling generalized-cost binding registers");
    game
}

#[test]
fn leashling_has_its_full_artifact_creature_definition() {
    let leashling = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-LEASHLING")
        .expect("Leashling definition exists");

    assert_eq!(leashling.name, "Leashling");
    assert_eq!(leashling.mana_cost, ManaCost::new(6));
    assert_eq!(
        leashling.card_types,
        BTreeSet::from([CardType::Artifact, CardType::Creature])
    );
    assert_eq!((leashling.power, leashling.toughness), (Some(3), Some(3)));
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&leashling.id));
    assert!(
        leashling
            .supported_rules
            .contains(&"hand-card-top-library-cost-return-source-owner-hand")
    );
}

#[test]
fn leashling_pays_a_hand_card_to_library_top_then_returns_its_live_source_on_resolution() {
    let mut game = game();
    let leashling = game
        .put_on_battlefield(PlayerId(0), "RAV-LEASHLING")
        .expect("Leashling setup");
    let payment = game
        .add_card(PlayerId(0), "RAV-PLAINS", Zone::Hand)
        .expect("owned hand card setup");
    game.begin_game().expect("game starts");
    game.clear_event_log();

    game.submit_policy_move(
        PlayerId(0),
        "test.leashling.v1",
        PolicyAction::ActivateAbilityWithGeneralizedCosts {
            activation: GeneralizedAbilityActivation {
                activation: AbilityActivation {
                    source: leashling,
                    ability_id: "hand-card-library-top-return-source",
                    sacrifice_sources: vec![],
                    additional_tap_creatures: vec![],
                    discard_cards: vec![],
                    targets: vec![],
                },
                cost_payment: AbilityCostPayment {
                    counter_sources: vec![],
                    return_permanents: vec![],
                    hand_cards_to_library_top: vec![payment],
                    chosen_x: None,
                },
                mana_payment_selection: None,
            },
        },
    )
    .expect("policy submits the exact hand-to-library payment");

    assert_eq!(game.zone_of(payment), Some(Zone::Library));
    assert_eq!(game.players[0].library.last(), Some(&payment));
    assert_eq!(game.zone_of(leashling), Some(Zone::Battlefield));
    assert!(game.event_log.windows(4).any(|events| matches!(
        events,
        [
            GameEvent::HandCardPutOnLibraryTopAsAbilityCost { player: PlayerId(0), source, card },
            GameEvent::CardMoved { card: moved, to: Zone::Library },
            GameEvent::ObjectIncarnationAdvanced { object, .. },
            GameEvent::AbilityActivated { player: PlayerId(0), source: activated, ability: "hand-card-library-top-return-source", .. },
        ] if *source == leashling && *card == payment && *moved == payment
            && *object == payment && *activated == leashling
    )));

    game.pass_priority(PlayerId(0)).expect("controller passes");
    game.pass_priority(PlayerId(1)).expect("opponent passes");
    assert_eq!(game.zone_of(leashling), Some(Zone::Hand));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::AbilityResolved { source, ability: "hand-card-library-top-return-source", .. }
            if *source == leashling
    )));
    eprintln!("Leashling trace={:?}", game.canonical_event_log());
    game.validate_invariants()
        .expect("Leashling cost and source-relative resolution stay valid");
}

#[test]
fn leashling_cannot_return_a_source_that_left_before_its_ability_resolves() {
    let mut game = game();
    let leashling = game
        .put_on_battlefield(PlayerId(0), "RAV-LEASHLING")
        .expect("Leashling setup");
    let payment = game
        .add_card(PlayerId(0), "RAV-PLAINS", Zone::Hand)
        .expect("owned hand card setup");
    let last_gasp = game
        .add_card(PlayerId(1), "RAV-LAST-GASP", Zone::Hand)
        .expect("response setup");
    let swamp = game
        .put_on_battlefield(PlayerId(1), "RAV-SWAMP")
        .expect("response mana setup");
    game.begin_game().expect("game starts");

    game.activate_ability_with_generalized_costs(
        PlayerId(0),
        GeneralizedAbilityActivation {
            activation: AbilityActivation {
                source: leashling,
                ability_id: "hand-card-library-top-return-source",
                sacrifice_sources: vec![],
                additional_tap_creatures: vec![],
                discard_cards: vec![],
                targets: vec![],
            },
            cost_payment: AbilityCostPayment {
                counter_sources: vec![],
                return_permanents: vec![],
                hand_cards_to_library_top: vec![payment],
                chosen_x: None,
            },
            mana_payment_selection: None,
        },
    )
    .expect("Leashling activation stacks");
    game.pass_priority(PlayerId(0))
        .expect("activating player gives the opponent priority");
    game.activate_mana_ability(PlayerId(1), swamp, Color::Black)
        .expect("response black mana");
    game.cast_spell(
        PlayerId(1),
        CastRequest {
            card: last_gasp,
            targets: vec![Target::Permanent(leashling)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("Last Gasp responds");
    game.pass_priority(PlayerId(1)).expect("caster passes");
    game.pass_priority(PlayerId(0))
        .expect("Leashling controller passes");
    assert_eq!(game.zone_of(leashling), Some(Zone::Graveyard));
    game.pass_priority(PlayerId(0))
        .expect("controller passes again");
    game.pass_priority(PlayerId(1))
        .expect("opponent resolves old ability");

    assert_eq!(game.zone_of(leashling), Some(Zone::Graveyard));
    assert_eq!(game.zone_of(payment), Some(Zone::Library));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::AbilityResolved { source, ability: "hand-card-library-top-return-source", .. }
            if *source == leashling
    )));
    game.validate_invariants()
        .expect("departed Leashling source cannot be returned by its old ability");
}
