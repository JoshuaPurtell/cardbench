//! Red-to-green contract for Necromantic Thirst's attached combat trigger.

use cardbench_magic_engine::{
    CardType, Game, GameEvent, ManaCost, PlayerId, PolicyAction, Step, Target, TargetRequirement,
    TriggerCondition, Zone,
};
use cardbench_magic_rav::{
    RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, rav_activated_ability_bindings,
    rav_additional_spell_cost_bindings, rav_attachment_bindings,
    rav_attachment_triggered_ability_bindings, rav_basic_land_type_bindings,
    rav_mana_ability_bindings, rav_triggered_ability_bindings,
};

fn rav_game() -> Game {
    let mut triggers = rav_triggered_ability_bindings();
    triggers.extend(rav_attachment_triggered_ability_bindings());
    let mut game = Game::new_with_all_bindings_and_triggers(
        card_definitions(),
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        rav_activated_ability_bindings(),
        triggers,
    )
    .expect("RAV game builds");
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

fn advance_to_declare_attackers(game: &mut Game) {
    game.begin_game().expect("fixture starts game");
    while game.step != Step::DeclareAttackers {
        pass_pair(game);
    }
}

#[test]
fn necromantic_thirst_is_manifested_with_its_public_graveyard_combat_trigger() {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-NECROMANTIC-THIRST")
        .expect("Necromantic Thirst definition exists");

    assert_eq!(definition.name, "Necromantic Thirst");
    assert_eq!(
        definition.mana_cost,
        ManaCost::with_colors(
            2,
            [
                cardbench_magic_engine::Color::Black,
                cardbench_magic_engine::Color::Black
            ]
        )
    );
    assert_eq!(
        definition.card_types,
        [CardType::Enchantment].into_iter().collect()
    );
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&definition.id));
    assert_eq!(
        definition.supported_rules,
        [
            "full-rules-fidelity",
            "aura-attach-and-static-pt",
            "attached-combat-damage-public-graveyard-creature-return",
        ]
    );

    let trigger = rav_attachment_triggered_ability_bindings()
        .into_iter()
        .find(|binding| binding.card_definition == definition.id)
        .expect("attached combat trigger binding exists");
    assert_eq!(
        trigger.ability.condition,
        TriggerCondition::AttachedCreatureDealsCombatDamageToPlayer
    );
    assert!(trigger.ability.optional);
    assert_eq!(
        trigger.ability.targets,
        [TargetRequirement::CreatureCardInGraveyard]
    );
    assert_eq!(trigger.ability.effects.len(), 1);
}

#[test]
fn necromantic_thirst_stacks_a_targeted_public_graveyard_return_after_its_creature_hits_a_player() {
    let controller = PlayerId(0);
    let opponent = PlayerId(1);
    let mut game = rav_game();
    let creature = game
        .put_on_battlefield(controller, "RAV-GOLIATH-SPIDER")
        .expect("attached creature setup");
    let thirst = game
        .add_card(controller, "RAV-NECROMANTIC-THIRST", Zone::Hand)
        .expect("Aura setup");
    game.enter_attachment_without_cast(thirst, creature)
        .expect("Aura attaches during setup");
    let returned = game
        .add_card(opponent, "RAV-WATCHWOLF", Zone::Graveyard)
        .expect("public creature-card target setup");
    game.set_entered_turn_for_setup(creature, 0)
        .expect("creature can attack");

    advance_to_declare_attackers(&mut game);
    game.declare_attackers(controller, &[creature])
        .expect("attached creature attacks");
    pass_pair(&mut game);
    game.declare_blockers(opponent, &[])
        .expect("opponent declares no blockers");
    pass_pair(&mut game);

    let choice = game
        .view_for_player(controller)
        .expect("controller view")
        .triggered_ability_target_choice
        .expect("combat trigger exposes a public graveyard target choice");
    assert_eq!(
        choice.ability,
        "attached-combat-damage-return-creature-card-to-owner-hand"
    );
    assert!(
        choice
            .target_options
            .first()
            .is_some_and(|targets| targets.contains(&Target::Permanent(returned)))
    );
    game.submit_policy_move(
        controller,
        "test.necromantic-thirst-public-graveyard-target.v1",
        PolicyAction::ChooseTriggeredAbilityTargets {
            decision: choice.decision,
            source: thirst,
            ability: "attached-combat-damage-return-creature-card-to-owner-hand",
            targets: vec![Target::Permanent(returned)],
        },
    )
    .expect("controller selects the public creature card");
    pass_pair(&mut game);
    let optional_choice = game
        .view_for_player(controller)
        .expect("controller view")
        .optional_triggered_ability_choice
        .expect("selected combat trigger reaches its optional resolution choice");
    game.submit_policy_move(
        controller,
        "test.necromantic-thirst-accept-return.v1",
        PolicyAction::ResolveOptionalTriggeredAbility {
            decision: optional_choice.decision,
            source: thirst,
            ability: "attached-combat-damage-return-creature-card-to-owner-hand",
            pay: true,
            target: None,
        },
    )
    .expect("controller accepts the selected public graveyard return");

    assert_eq!(game.zone_of(returned), Some(Zone::Hand));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::TriggeredAbilityStacked { source, controller: trigger_controller, ability, .. }
            if *source == thirst
                && *trigger_controller == controller
                && *ability == "attached-combat-damage-return-creature-card-to-owner-hand"
    )));
    println!("necromantic_thirst_trace={:#?}", game.canonical_event_log());
    game.validate_invariants()
        .expect("attached combat graveyard-return trace preserves invariants");
}
