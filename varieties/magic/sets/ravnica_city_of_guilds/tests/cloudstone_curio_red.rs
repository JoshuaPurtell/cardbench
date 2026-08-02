//! Red regression for Cloudstone Curio's controller-choice bounce trigger.
//!
//! Cloudstone Curio is deliberately a non-targeting trigger: after a
//! nonartifact permanent enters under its controller, the controller may
//! choose another controlled permanent sharing a card type and return it to
//! its owner's hand.  The green repair must retain that entering-permanent
//! identity through stack resolution instead of turning the choice into a
//! generic or opponent-facing target.

use cardbench_magic_engine::{
    CardType, CastRequest, Color, DecisionKind, DecisionSelection, Game, GameEvent, ManaCost,
    PlayerId, TriggerCondition, Zone,
};
use cardbench_magic_rav::{
    RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, rav_activated_ability_bindings,
    rav_additional_spell_cost_bindings, rav_basic_land_type_bindings, rav_mana_ability_bindings,
    rav_triggered_ability_bindings,
};

fn definition(id: &str) -> cardbench_magic_engine::CardDefinition {
    card_definitions()
        .into_iter()
        .find(|definition| definition.id == id)
        .unwrap_or_else(|| panic!("{id} definition exists"))
}

#[test]
fn cloudstone_curio_has_the_full_colorless_etb_choice_contract() {
    let curio = definition("RAV-CLOUDSTONE-CURIO");
    assert_eq!(curio.mana_cost, ManaCost::new(3));
    assert_eq!(curio.card_types, [CardType::Artifact].into_iter().collect());
    assert!(
        RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&curio.id),
        "Cloudstone Curio needs the complete optional, source-relative ETB bounce path"
    );
    assert!(
        curio
            .supported_rules
            .contains(&"controlled-nonartifact-etb-may-bounce-another-sharing-card-type")
    );
}

#[test]
fn cloudstone_curio_binds_a_controller_scoped_nonartifact_entry_trigger() {
    let binding = rav_triggered_ability_bindings()
        .into_iter()
        .find(|binding| binding.card_definition == "RAV-CLOUDSTONE-CURIO")
        .expect("Cloudstone Curio trigger binding exists");
    assert_eq!(
        binding.ability.condition,
        TriggerCondition::ControlledNonartifactPermanentEntersBattlefield
    );
    assert!(binding.ability.optional, "the return remains a may choice");
    assert!(
        binding.ability.targets.is_empty(),
        "the printed ability does not target"
    );
}

fn game() -> Game {
    Game::new_with_all_bindings_and_triggers(
        card_definitions(),
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        rav_activated_ability_bindings(),
        rav_triggered_ability_bindings(),
    )
    .expect("RAV fixture builds")
}

fn pass_pair(game: &mut Game) {
    let first = game.priority;
    game.pass_priority(first).expect("first priority pass");
    let second = game.priority;
    game.pass_priority(second).expect("second priority pass");
}

#[test]
fn cloudstone_curio_retains_entry_types_and_bounces_only_a_chosen_other_controlled_permanent() {
    let mut game = game();
    let curio = game
        .add_card(PlayerId(0), "RAV-CLOUDSTONE-CURIO", Zone::Battlefield)
        .expect("Curio setup");
    let compatible = game
        .add_card(PlayerId(0), "RAV-GLASS-GOLEM", Zone::Battlefield)
        .expect("artifact creature setup");
    let entering = game
        .add_card(PlayerId(0), "RAV-WATCHWOLF", Zone::Hand)
        .expect("nonartifact creature setup");
    game.grant_mana(PlayerId(0), Color::Green, 1)
        .expect("green cast mana");
    game.grant_mana(PlayerId(0), Color::White, 1)
        .expect("white cast mana");
    game.cast_spell(
        PlayerId(0),
        CastRequest {
            card: entering,
            targets: vec![],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("Watchwolf casts");
    pass_pair(&mut game);
    assert_eq!(game.zone_of(entering), Some(Zone::Battlefield));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::TriggeredAbilityStacked { source, ability, .. }
            if *source == curio && *ability == "controlled-nonartifact-etb-may-bounce-sharing-card-type"
    )));

    pass_pair(&mut game);
    let decision = game
        .view_for_player(PlayerId(0))
        .expect("Curio controller view")
        .pending_decision
        .expect("Curio choice opens");
    assert_eq!(decision.kind, DecisionKind::TriggeredEffectObject);
    assert_eq!(decision.min_selections, 0, "the controller may decline");
    assert_eq!(decision.max_selections, 1);
    assert_eq!(
        decision.candidates.iter().map(|candidate| candidate.id).collect::<Vec<_>>(),
        vec![compatible],
        "Curio sees only the other controlled permanent sharing Creature"
    );
    game.submit_decision(
        PlayerId(0),
        decision.id,
        DecisionSelection::Objects(vec![compatible]),
    )
    .expect("controller returns compatible permanent");

    println!("Cloudstone Curio return trace: {:?}", game.canonical_event_log());
    assert_eq!(game.zone_of(compatible), Some(Zone::Hand));
    assert_eq!(game.zone_of(entering), Some(Zone::Battlefield));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::AbilityResolved { source, ability, .. }
            if *source == curio && *ability == "controlled-nonartifact-etb-may-bounce-sharing-card-type"
    )));
    game.validate_invariants()
        .expect("Curio chosen-return branch preserves invariants");
}

#[test]
fn cloudstone_curio_controller_can_explicitly_decline_the_optional_return() {
    let mut game = game();
    game.add_card(PlayerId(0), "RAV-CLOUDSTONE-CURIO", Zone::Battlefield)
        .expect("Curio setup");
    let compatible = game
        .add_card(PlayerId(0), "RAV-GLASS-GOLEM", Zone::Battlefield)
        .expect("compatible permanent setup");
    let entering = game
        .add_card(PlayerId(0), "RAV-WATCHWOLF", Zone::Hand)
        .expect("entry setup");
    game.grant_mana(PlayerId(0), Color::Green, 1).unwrap();
    game.grant_mana(PlayerId(0), Color::White, 1).unwrap();
    game.cast_spell(
        PlayerId(0),
        CastRequest { card: entering, targets: vec![], convoke: vec![], payment_mana_abilities: vec![] },
    )
    .expect("Watchwolf casts");
    pass_pair(&mut game);
    pass_pair(&mut game);
    let decision = game
        .view_for_player(PlayerId(0))
        .expect("Curio controller view")
        .pending_decision
        .expect("Curio choice opens");
    game.submit_decision(PlayerId(0), decision.id, DecisionSelection::Objects(vec![]))
        .expect("controller may decline the return");
    assert_eq!(game.zone_of(compatible), Some(Zone::Battlefield));
    assert_eq!(game.zone_of(entering), Some(Zone::Battlefield));
    game.validate_invariants()
        .expect("Curio decline branch preserves invariants");
}
