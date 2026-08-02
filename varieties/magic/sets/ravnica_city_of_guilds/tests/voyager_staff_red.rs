//! Red discovery contract for Voyager Staff's linked-exile lifecycle.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    AbilityActivation, ActivatedAbility, CardType, Color, Effect, Game, GameEvent,
    LinkedExileMemberRole, ManaCost, PlayerId, Step, Target, TargetRequirement, Zone,
};
use cardbench_magic_rav::{
    RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, executable_definition_id_for_collector,
    rav_activated_ability_bindings, rav_additional_spell_cost_bindings,
    rav_basic_land_type_bindings, rav_mana_ability_bindings,
};

#[test]
fn voyager_staff_requires_exact_linked_exile_definition() {
    let staff = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-VOYAGER-STAFF")
        .expect("Voyager Staff definition exists");
    assert_eq!(staff.name, "Voyager Staff");
    assert_eq!(staff.mana_cost, ManaCost::new(1));
    assert_eq!(staff.colors, BTreeSet::<Color>::new());
    assert_eq!(staff.card_types, BTreeSet::from([CardType::Artifact]));
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&staff.id));
    assert!(
        staff
            .supported_rules
            .contains(&"sacrifice-linked-exile-target-creature-until-end-step")
    );
    assert!(rav_activated_ability_bindings().iter().any(|binding| {
        binding.card_definition == staff.id
            && binding.ability
                == ActivatedAbility {
                    id: "sacrifice-linked-exile-target-creature-until-end-step",
                    mana_cost: ManaCost::new(2),
                    tap_cost: false,
                    sorcery_speed: false,
                    additional_tap_creatures: 0,
                    sacrifice_source: true,
                    sacrifice_creatures: 0,
                    sacrifice_lands: 0,
                    discard_cards: 0,
                    targets: vec![TargetRequirement::Creature],
                    effects: vec![Effect::ExileTargetCreatureUntilEndStep],
                }
    }));
}

#[test]
fn voyager_staff_catalog_mapping_names_only_the_typed_full_definition() {
    assert_eq!(
        executable_definition_id_for_collector(274),
        Ok("RAV-VOYAGER-STAFF")
    );
    assert!(
        card_definitions()
            .iter()
            .any(|definition| definition.id == "RAV-VOYAGER-STAFF")
    );
}

fn pass_pair(game: &mut Game) {
    let first = game.priority;
    game.pass_priority(first).expect("first priority pass");
    let second = game.priority;
    game.pass_priority(second).expect("second priority pass");
}

fn advance_until_delayed_return(game: &mut Game) {
    for _ in 0..32 {
        if game
            .event_log
            .iter()
            .any(|event| matches!(event, GameEvent::DelayedActionConsumed { .. }))
        {
            return;
        }
        match game.step {
            Step::DeclareAttackers
                if !game
                    .event_log
                    .iter()
                    .any(|event| matches!(event, GameEvent::AttackersDeclared { .. })) =>
            {
                game.declare_attackers(game.active_player, &[])
                    .expect("empty attackers are declared");
            }
            Step::DeclareBlockers
                if !game
                    .event_log
                    .iter()
                    .any(|event| matches!(event, GameEvent::BlockersDeclared { .. })) =>
            {
                game.declare_blockers(PlayerId(1 - game.active_player.0), &[])
                    .expect("empty blockers are declared");
            }
            _ => {
                let priority = game.priority;
                game.pass_priority(priority)
                    .expect("priority advances delayed action timing");
            }
        }
    }
    panic!("Voyager Staff delayed return did not reach the next end step");
}

#[test]
#[allow(clippy::too_many_lines)] // The exact cost, exile, and delayed-return lifecycle is causal evidence.
fn voyager_staff_sacrifices_then_returns_the_exact_target_exile_incarnation() {
    let mut game = Game::new_with_all_bindings(
        card_definitions(),
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        rav_activated_ability_bindings(),
    )
    .expect("RAV fixture builds");
    let staff = game
        .put_on_battlefield(PlayerId(0), "RAV-VOYAGER-STAFF")
        .expect("Staff begins on battlefield");
    let target = game
        .put_on_battlefield(PlayerId(1), "RAV-WATCHWOLF")
        .expect("opposing target creature begins on battlefield");
    let original_target_incarnation = game
        .object(target)
        .expect("target exists before activation")
        .incarnation;
    game.begin_game().expect("fixture starts game");
    pass_pair(&mut game);
    pass_pair(&mut game);
    assert_eq!(game.step, Step::PrecombatMain);
    game.add_mana_from_action(PlayerId(0), Color::Colorless, 2)
        .expect("generic activation payment is legal");
    game.activate_ability(
        PlayerId(0),
        AbilityActivation {
            source: staff,
            ability_id: "sacrifice-linked-exile-target-creature-until-end-step",
            sacrifice_sources: vec![staff],
            additional_tap_creatures: vec![],
            discard_cards: vec![],
            targets: vec![Target::Permanent(target)],
        },
    )
    .expect("Staff pays its sacrifice cost and stacks the target ability");
    assert_eq!(game.zone_of(staff), Some(Zone::Graveyard));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::SacrificedAsAbilityCost { source, permanent, .. }
            if *source == staff && *permanent == staff
    )));
    pass_pair(&mut game);
    assert_eq!(game.zone_of(target), Some(Zone::Exile));
    let target_exile_incarnation = game
        .object(target)
        .expect("target exists in exile")
        .incarnation;
    assert!(target_exile_incarnation > original_target_incarnation);
    let group = game
        .event_log
        .iter()
        .find_map(|event| match event {
            GameEvent::DelayedActionScheduled { group, members, .. }
                if members
                    == &vec![cardbench_magic_engine::LinkedExileMember {
                        object: target,
                        exile_incarnation: target_exile_incarnation,
                        role: LinkedExileMemberRole::PrimaryCreature,
                    }] =>
            {
                Some(*group)
            }
            _ => None,
        })
        .expect("exact target exile incarnation is scheduled");
    assert_eq!(
        game.linked_exile_group(group)
            .expect("linked group remains typed game state")
            .source,
        staff
    );
    advance_until_delayed_return(&mut game);

    println!("Voyager Staff trace: {:#?}", game.canonical_event_log());
    assert_eq!(game.zone_of(target), Some(Zone::Battlefield));
    assert_eq!(
        game.controller_of(target)
            .expect("target returned under an owner"),
        PlayerId(1)
    );
    assert!(game.object(target).expect("target returns").incarnation > target_exile_incarnation);
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::DelayedActionConsumed { returned, .. } if returned == &vec![target]
    )));
    game.validate_invariants()
        .expect("targeted linked exile preserves delayed-action invariants");
}
