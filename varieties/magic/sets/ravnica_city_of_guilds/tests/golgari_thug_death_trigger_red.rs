//! Red-to-green full-fidelity contract for Golgari Thug's death trigger.
//!
//! The public operation is a policy-selected creature card from the trigger
//! controller's graveyard moving to the top of its owner's library.  This
//! contains no upstream card prose or hidden fixture data.

use cardbench_magic_engine::{
    CastRequest, Color, Effect, Game, GameEvent, PlayerId, PolicyAction, Step, Target,
    TargetRequirement, TriggerCondition, Zone,
};
use cardbench_magic_rav::{
    RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, rav_activated_ability_bindings,
    rav_additional_spell_cost_bindings, rav_basic_land_type_bindings, rav_mana_ability_bindings,
    rav_triggered_ability_bindings,
};

fn advance_to_precombat_main(game: &mut Game) {
    game.begin_game().expect("fixture starts game");
    while game.step != Step::PrecombatMain {
        let first = game.priority;
        game.pass_priority(first).expect("first step pass");
        let second = game.priority;
        game.pass_priority(second).expect("second step pass");
    }
}

fn resolve_top(game: &mut Game) {
    let first = game.priority;
    game.pass_priority(first).expect("first stack pass");
    let second = game.priority;
    game.pass_priority(second).expect("second stack pass");
}

#[test]
#[allow(clippy::too_many_lines)] // Death trigger, private library choice, and return provenance form one transcript.
fn golgari_thug_death_trigger_is_ability_complete() {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-GOLGARI-THUG")
        .expect("Golgari Thug definition exists");
    assert!(
        RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&definition.id),
        "Golgari Thug cannot be positive-manifest while its death trigger is absent"
    );
    assert_eq!(
        definition.supported_rules,
        [
            "full-rules-fidelity",
            "dredge",
            "base-characteristics",
            "dies-target-creature-card-owner-library-top",
        ]
    );
    let binding = rav_triggered_ability_bindings()
        .into_iter()
        .find(|binding| binding.card_definition == definition.id)
        .expect("Golgari Thug death trigger binding exists");
    assert_eq!(
        binding.ability.id,
        "dies-target-creature-card-owner-library-top"
    );
    assert_eq!(binding.ability.condition, TriggerCondition::Dies);
    assert_eq!(
        binding.ability.targets,
        [TargetRequirement::CreatureCardInControllerGraveyard]
    );
    assert_eq!(binding.ability.effects.len(), 1);

    let mut game = Game::new_with_all_bindings_and_triggers(
        card_definitions(),
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        rav_activated_ability_bindings(),
        rav_triggered_ability_bindings(),
    )
    .expect("RAV game builds");
    let thug = game
        .put_on_battlefield(PlayerId(0), "RAV-GOLGARI-THUG")
        .expect("Thug begins on battlefield");
    let target = game
        .add_card(PlayerId(0), "RAV-WATCHWOLF", Zone::Graveyard)
        .expect("chosen creature card begins in controller graveyard");
    let library_bottom = game
        .add_card(PlayerId(0), "RAV-FOREST", Zone::Library)
        .expect("library bottom exists");
    let removal = game
        .add_card(PlayerId(1), "RAV-LAST-GASP", Zone::Hand)
        .expect("removal begins in opponent hand");
    let swamp = game
        .put_on_battlefield(PlayerId(1), "RAV-SWAMP")
        .expect("opponent mana source exists");
    advance_to_precombat_main(&mut game);

    game.pass_priority(PlayerId(0))
        .expect("active player gives opponent priority");
    game.activate_mana_ability(PlayerId(1), swamp, Color::Black)
        .expect("opponent produces black mana");
    game.cast_spell(
        PlayerId(1),
        CastRequest {
            card: removal,
            targets: vec![Target::Permanent(thug)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("opponent removes Thug");
    resolve_top(&mut game);

    let choice = game
        .view_for_player(PlayerId(0))
        .expect("Thug controller view")
        .triggered_ability_target_choice
        .expect("death trigger requires controller target choice");
    assert_eq!(choice.source, thug);
    assert_eq!(
        choice.ability,
        "dies-target-creature-card-owner-library-top"
    );
    assert_eq!(
        choice.target_options,
        vec![vec![Target::Permanent(thug), Target::Permanent(target)]],
        "both current creature cards in the controller graveyard are legal choices"
    );
    game.submit_policy_move(
        PlayerId(0),
        "test.golgari-thug-target.v1",
        PolicyAction::ChooseTriggeredAbilityTargets {
            source: thug,
            ability: "dies-target-creature-card-owner-library-top",
            targets: vec![Target::Permanent(target)],
        },
    )
    .expect("controller selects the non-source creature card");
    let trigger = game.stack.last().expect("targeted death trigger stacks");
    assert_eq!(
        trigger.effects,
        vec![Effect::PutTargetCreatureCardInControllerGraveyardOnOwnersLibraryTop]
    );
    assert_eq!(trigger.targets, vec![Target::Permanent(target)]);
    resolve_top(&mut game);
    println!(
        "Golgari Thug full-fidelity trace: {:?}",
        game.canonical_event_log()
    );
    assert_eq!(game.zone_of(thug), Some(Zone::Graveyard));
    assert_eq!(game.zone_of(target), Some(Zone::Library));
    assert_eq!(
        game.players[PlayerId(0).0].library,
        vec![library_bottom, target]
    );
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::CardMoved { card, to: Zone::Library } if *card == target
    )));
    game.validate_invariants()
        .expect("Golgari Thug death trigger preserves invariants");
}
