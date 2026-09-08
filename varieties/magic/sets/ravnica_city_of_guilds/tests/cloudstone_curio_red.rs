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
fn curio_retains_the_selected_commander_across_the_owners_replacement_choice() {
    for accept in [false, true] {
        let owner = PlayerId(0);
        let mut game = game();
        game.configure_commander_format(40, 21).unwrap();
        let forest = game.put_on_battlefield(owner, "RAV-FOREST").unwrap();
        let plains = game.put_on_battlefield(owner, "RAV-PLAINS").unwrap();
        let commander = game.put_on_battlefield(owner, "RAV-TOLSIMIR-WOLFBLOOD").unwrap();
        game.designate_commander(owner, commander).unwrap();
        let curio = game.put_on_battlefield(owner, "RAV-CLOUDSTONE-CURIO").unwrap();
        let entering = game.add_card(owner, "RAV-WATCHWOLF", Zone::Hand).unwrap();
        for player in [owner, PlayerId(1)] {
            game.add_card(player, "RAV-FOREST", Zone::Library).unwrap();
        }
        game.begin_game().unwrap();
        for _ in 0..8 {
            if game.step == cardbench_magic_engine::Step::PrecombatMain { break; }
            pass_pair(&mut game);
        }
        assert_eq!(game.step, cardbench_magic_engine::Step::PrecombatMain);
        game.activate_mana_ability(owner, forest, Color::Green).unwrap();
        game.activate_mana_ability(owner, plains, Color::White).unwrap();
        game.cast_spell(owner, CastRequest { card: entering, targets: vec![],
            convoke: vec![], payment_mana_abilities: vec![] }).unwrap();
        pass_pair(&mut game);
        pass_pair(&mut game);
        let selection = game.view_for_player(owner).unwrap().pending_decision.unwrap();
        assert_eq!(selection.kind, DecisionKind::TriggeredEffectObject);
        game.submit_decision(owner, selection.id, DecisionSelection::Objects(vec![commander])).unwrap();
        let replacement = game.view_for_player(owner).unwrap().pending_decision.unwrap();
        assert_eq!(replacement.kind, DecisionKind::CommanderZoneReplacement);
        assert_ne!(selection.id, replacement.id);
        assert_eq!(game.zone_of(commander), Some(Zone::Battlefield));
        assert!(game.pass_priority(owner).is_err());
        game.submit_decision(owner, replacement.id,
            DecisionSelection::Objects(if accept { vec![commander] } else { vec![] })).unwrap();
        assert_eq!(game.zone_of(commander), Some(if accept { Zone::Command } else { Zone::Hand }));
        assert_eq!(game.zone_of(entering), Some(Zone::Battlefield));
        assert_eq!(game.event_log.iter().filter(|event| matches!(event,
            GameEvent::AbilityResolved { source, .. } if *source == curio)).count(), 1);
        assert!(game.view_for_player(owner).unwrap().pending_decision.is_none());
        game.validate_invariants().unwrap();
    }
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
        decision
            .candidates
            .iter()
            .map(|candidate| candidate.id)
            .collect::<Vec<_>>(),
        vec![compatible],
        "Curio sees only the other controlled permanent sharing Creature"
    );
    game.submit_decision(
        PlayerId(0),
        decision.id,
        DecisionSelection::Objects(vec![compatible]),
    )
    .expect("controller returns compatible permanent");

    println!(
        "Cloudstone Curio return trace: {:?}",
        game.canonical_event_log()
    );
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
        CastRequest {
            card: entering,
            targets: vec![],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
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
