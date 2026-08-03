//! Full-fidelity contract for Breath of Fury's Aura-relative combat lifecycle.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardType, CastRequest, Color, DecisionKind, DecisionSelection, Game, GameEvent, ManaCost,
    PlayerId, Step, Target, Zone,
};
use cardbench_magic_rav::{
    RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, executable_definition_id_for_collector,
    rav_activated_ability_bindings, rav_additional_spell_cost_bindings, rav_attachment_bindings,
    rav_basic_land_type_bindings, rav_mana_ability_bindings, rav_triggered_ability_bindings,
};

fn game_with_rav_combat_bindings() -> Game {
    let mut game = Game::new_with_all_bindings_and_triggers(
        card_definitions(),
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        rav_activated_ability_bindings(),
        rav_triggered_ability_bindings(),
    )
    .expect("RAV combat fixture builds");
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

fn advance_to_attackers(game: &mut Game) {
    game.begin_game().expect("game begins");
    for _ in 0..16 {
        if game.step == Step::DeclareAttackers {
            return;
        }
        let player = game.priority;
        game.pass_priority(player)
            .expect("priority advances toward attackers");
    }
    panic!("fixture did not reach declare attackers");
}

#[test]
fn breath_of_fury_has_exact_attached_combat_definition_and_bindings() {
    assert_eq!(
        executable_definition_id_for_collector(116),
        Ok("RAV-BREATH-OF-FURY")
    );
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-BREATH-OF-FURY")
        .expect("Breath of Fury definition exists");
    assert_eq!(definition.name, "Breath of Fury");
    assert_eq!(
        definition.mana_cost,
        ManaCost::with_colors(2, [Color::Red, Color::Red])
    );
    assert_eq!(definition.colors, BTreeSet::from([Color::Red]));
    assert_eq!(
        definition.card_types,
        BTreeSet::from([CardType::Enchantment])
    );
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&definition.id));
    assert!(
        definition
            .supported_rules
            .contains(&"additional-combat-phase")
    );
    assert!(rav_attachment_bindings().iter().any(|binding| {
        binding.card_definition == definition.id
            && binding.target == cardbench_magic_engine::TargetRequirement::ControlledCreature
    }));
    assert!(rav_triggered_ability_bindings().iter().any(|binding| {
        binding.card_definition == definition.id
            && binding.ability.id
                == "attached-creature-combat-damage-sacrifice-reattach-untap-add-combat"
    }));
}

#[test]
fn breath_of_fury_sacrifices_reattaches_untaps_and_inserts_combat() {
    let controller = PlayerId(0);
    let mut game = game_with_rav_combat_bindings();
    let carrier = game
        .put_on_battlefield(controller, "RAV-WATCHWOLF")
        .expect("carrier setup");
    let successor = game
        .put_on_battlefield(controller, "RAV-WATCHWOLF")
        .expect("successor setup");
    let reserve = game
        .put_on_battlefield(controller, "RAV-BOROS-RECRUIT")
        .expect("reserve setup");
    game.set_tapped_for_setup(successor, true)
        .expect("successor begins tapped");
    game.set_tapped_for_setup(reserve, true)
        .expect("reserve begins tapped");
    let aura = game
        .add_card(controller, "RAV-BREATH-OF-FURY", Zone::Hand)
        .expect("Aura setup");
    game.grant_mana(controller, Color::Red, 4)
        .expect("red payment setup");
    game.cast_spell(
        controller,
        CastRequest {
            card: aura,
            targets: vec![Target::Permanent(carrier)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("Aura casts on the carrier");
    pass_pair(&mut game);
    game.set_entered_turn_for_setup(carrier, 0)
        .expect("carrier is long controlled");
    advance_to_attackers(&mut game);
    game.clear_event_log();

    game.declare_attackers(controller, &[carrier])
        .expect("carrier attacks");
    pass_pair(&mut game);
    game.declare_blockers(PlayerId(1), &[])
        .expect("defender declines blocks");
    pass_pair(&mut game);
    pass_pair(&mut game);
    let decision = game
        .view_for_player(controller)
        .expect("controller view")
        .pending_decision
        .expect("reattachment choice opens");
    assert_eq!(decision.kind, DecisionKind::TriggeredEffectObject);
    assert_eq!(
        decision
            .candidates
            .iter()
            .map(|candidate| candidate.id)
            .collect::<BTreeSet<_>>(),
        BTreeSet::from([successor, reserve])
    );
    game.submit_decision(
        controller,
        decision.id,
        DecisionSelection::Objects(vec![successor]),
    )
    .expect("controller reattaches Aura to successor");

    assert_eq!(game.zone_of(carrier), Some(Zone::Graveyard));
    assert_eq!(game.zone_of(aura), Some(Zone::Battlefield));
    assert_eq!(
        game.object(aura).expect("Aura live").attached_to,
        Some(successor)
    );
    assert!(!game.object(successor).expect("successor live").tapped);
    assert!(!game.object(reserve).expect("reserve live").tapped);
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::AdditionalCombatPhaseCreated { source, controller: event_controller, .. }
            if *source == aura && *event_controller == controller
    )));
    pass_pair(&mut game);
    assert_eq!(game.step, Step::EndOfCombat);
    pass_pair(&mut game);
    assert_eq!(game.step, Step::BeginningOfCombat);
    game.validate_invariants()
        .expect("Aura combat lifecycle preserves invariants");
}
