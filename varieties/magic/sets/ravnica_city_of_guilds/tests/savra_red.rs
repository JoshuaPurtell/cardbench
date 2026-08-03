//! Red contract for Savra's sacrifice-color trigger boundary.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    AbilityActivation, CardType, Color, DecisionSelection, Game, GameEvent, ManaCost, PlayerId,
    PolicyAction, Target, Zone,
};
use cardbench_magic_rav::{
    RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, executable_definition_id_for_collector,
    rav_activated_ability_bindings, rav_additional_spell_cost_bindings,
    rav_basic_land_type_bindings, rav_mana_ability_bindings, rav_triggered_ability_bindings,
};

const BLACK_TRIGGER: &str =
    "controller-sacrifices-black-creature-pay-two-life-each-opponent-sacrifices";
const GREEN_TRIGGER: &str = "controller-sacrifices-green-creature-may-gain-two-life";

fn game_with_rav_bindings() -> Game {
    Game::new_with_all_bindings_and_triggers(
        card_definitions(),
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        rav_activated_ability_bindings(),
        rav_triggered_ability_bindings(),
    )
    .expect("RAV game builds")
}

fn resolve_top(game: &mut Game) {
    let first = game.priority;
    game.pass_priority(first).expect("first priority pass");
    let second = game.priority;
    game.pass_priority(second)
        .expect("second priority pass resolves");
}

fn activate_rotwurm_sacrificing(
    game: &mut Game,
    rotwurm: cardbench_magic_engine::ObjectId,
    victim: cardbench_magic_engine::ObjectId,
    swamp: cardbench_magic_engine::ObjectId,
) {
    game.begin_game().expect("fixture begins");
    game.activate_mana_ability(PlayerId(0), swamp, Color::Black)
        .expect("Swamp produces black mana");
    game.activate_ability(
        PlayerId(0),
        AbilityActivation {
            source: rotwurm,
            ability_id: "sacrifice-creature-target-player-life-loss",
            sacrifice_sources: vec![victim],
            additional_tap_creatures: vec![],
            discard_cards: vec![],
            targets: vec![Target::Player(PlayerId(1))],
        },
    )
    .expect("Rotwurm activation pays the selected creature sacrifice");
    resolve_top(game);
}

fn resolve_optional_trigger(
    game: &mut Game,
    source: cardbench_magic_engine::ObjectId,
    ability: &'static str,
    pay: bool,
) {
    let choice = game
        .view_for_player(PlayerId(0))
        .expect("Savra controller view")
        .optional_triggered_ability_choice
        .expect("Savra optional trigger choice");
    assert_eq!(choice.source, source);
    assert_eq!(choice.ability, ability);
    game.submit_policy_move(
        PlayerId(0),
        "test.savra-resolve-optional-trigger.v1",
        PolicyAction::ResolveOptionalTriggeredAbility {
            decision: choice.decision,
            source,
            ability,
            pay,
            target: None,
        },
    )
    .expect("Savra controller resolves optional trigger");
}

#[test]
fn savra_has_an_executable_full_fidelity_definition() {
    assert_eq!(
        executable_definition_id_for_collector(225),
        Ok("RAV-SAVRA-QUEEN-OF-THE-GOLGARI")
    );
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-SAVRA-QUEEN-OF-THE-GOLGARI")
        .expect("Savra definition exists");
    assert_eq!(definition.name, "Savra, Queen of the Golgari");
    assert_eq!(
        definition.mana_cost,
        ManaCost::with_colors(2, [Color::Black, Color::Green])
    );
    assert_eq!(
        definition.colors,
        BTreeSet::from([Color::Black, Color::Green])
    );
    assert_eq!(definition.card_types, BTreeSet::from([CardType::Creature]));
    assert_eq!(definition.power, Some(2));
    assert_eq!(definition.toughness, Some(2));
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&definition.id));
}

#[test]
fn savra_black_sacrifice_pays_life_then_each_opponent_selects_a_creature() {
    let mut game = game_with_rav_bindings();
    let savra = game
        .put_on_battlefield(PlayerId(0), "RAV-SAVRA-QUEEN-OF-THE-GOLGARI")
        .expect("Savra setup");
    let rotwurm = game
        .put_on_battlefield(PlayerId(0), "RAV-GOLGARI-ROTWURM")
        .expect("Rotwurm setup");
    let black_victim = game
        .put_on_battlefield(PlayerId(0), "RAV-DIMIR-CUTPURSE")
        .expect("black creature setup");
    let opponent_creature = game
        .put_on_battlefield(PlayerId(1), "RAV-WATCHWOLF")
        .expect("opponent creature setup");
    let swamp = game
        .put_on_battlefield(PlayerId(0), "RAV-SWAMP")
        .expect("Swamp setup");

    activate_rotwurm_sacrificing(&mut game, rotwurm, black_victim, swamp);
    assert_eq!(game.zone_of(black_victim), Some(Zone::Graveyard));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::TriggeredAbilityStacked { source, ability, .. }
            if *source == savra && *ability == BLACK_TRIGGER
    )));

    resolve_optional_trigger(&mut game, savra, BLACK_TRIGGER, true);
    assert_eq!(game.players[0].life, 18);
    let opponent_choice = game
        .view_for_player(PlayerId(1))
        .expect("opponent view")
        .triggered_ability_effect_object_choice
        .expect("each opponent chooses a creature to sacrifice");
    assert_eq!(opponent_choice.candidates.len(), 1);
    game.submit_policy_move(
        PlayerId(1),
        "test.savra-opponent-sacrifice.v1",
        PolicyAction::ChooseTriggeredAbilityEffectObject {
            decision: opponent_choice.decision,
            source: savra,
            ability: BLACK_TRIGGER,
            selected: Some(opponent_creature),
        },
    )
    .expect("opponent sacrifices its selected creature");

    println!(
        "Savra black-trigger trace: {:#?}",
        game.canonical_event_log()
    );
    assert_eq!(game.zone_of(opponent_creature), Some(Zone::Graveyard));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::LifePaid { source, player: PlayerId(0), amount: 2 }
            if *source == savra
    )));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::SacrificedByEffect { source, player: PlayerId(1), permanent }
            if *source == savra && *permanent == opponent_creature
    )));
    game.validate_invariants()
        .expect("Savra black-trigger transition preserves invariants");
}

#[test]
fn savra_black_sacrifice_can_decline_without_paying_life_or_forcing_a_sacrifice() {
    let mut game = game_with_rav_bindings();
    let savra = game
        .put_on_battlefield(PlayerId(0), "RAV-SAVRA-QUEEN-OF-THE-GOLGARI")
        .expect("Savra setup");
    let rotwurm = game
        .put_on_battlefield(PlayerId(0), "RAV-GOLGARI-ROTWURM")
        .expect("Rotwurm setup");
    let black_victim = game
        .put_on_battlefield(PlayerId(0), "RAV-DIMIR-CUTPURSE")
        .expect("black creature setup");
    let opponent_creature = game
        .put_on_battlefield(PlayerId(1), "RAV-WATCHWOLF")
        .expect("opponent creature setup");
    let swamp = game
        .put_on_battlefield(PlayerId(0), "RAV-SWAMP")
        .expect("Swamp setup");

    activate_rotwurm_sacrificing(&mut game, rotwurm, black_victim, swamp);
    resolve_optional_trigger(&mut game, savra, BLACK_TRIGGER, false);

    assert_eq!(game.players[0].life, 20);
    assert_eq!(game.zone_of(opponent_creature), Some(Zone::Battlefield));
    assert!(!game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::LifePaid { source, .. } if *source == savra
    )));
    assert!(!game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::SacrificedByEffect { source, .. } if *source == savra
    )));
    game.validate_invariants()
        .expect("Savra black-trigger decline preserves invariants");
}

#[test]
fn savra_green_sacrifice_policy_can_gain_or_decline_two_life_without_paying_life() {
    for (pay, expected_life) in [(true, 22_i64), (false, 20_i64)] {
        let mut game = game_with_rav_bindings();
        let savra = game
            .put_on_battlefield(PlayerId(0), "RAV-SAVRA-QUEEN-OF-THE-GOLGARI")
            .expect("Savra setup");
        let rotwurm = game
            .put_on_battlefield(PlayerId(0), "RAV-GOLGARI-ROTWURM")
            .expect("Rotwurm setup");
        let green_victim = game
            .put_on_battlefield(PlayerId(0), "RAV-WATCHWOLF")
            .expect("green creature setup");
        let swamp = game
            .put_on_battlefield(PlayerId(0), "RAV-SWAMP")
            .expect("Swamp setup");

        activate_rotwurm_sacrificing(&mut game, rotwurm, green_victim, swamp);
        resolve_optional_trigger(&mut game, savra, GREEN_TRIGGER, pay);
        assert_eq!(game.players[0].life, expected_life);
        assert!(
            game.event_log.iter().any(|event| matches!(
                event,
                GameEvent::LifeGained { player: PlayerId(0), amount: 2 } if pay
            )) == pay
        );
        assert!(!game.event_log.iter().any(|event| matches!(
            event,
            GameEvent::LifePaid { source, .. } if *source == savra
        )));
        game.validate_invariants()
            .expect("Savra green-trigger transition preserves invariants");
    }
}

#[test]
fn savra_multicolored_sacrifice_observes_black_and_green_independently() {
    let mut game = game_with_rav_bindings();
    let savra = game
        .put_on_battlefield(PlayerId(0), "RAV-SAVRA-QUEEN-OF-THE-GOLGARI")
        .expect("Savra setup");
    let rotwurm = game
        .put_on_battlefield(PlayerId(0), "RAV-GOLGARI-ROTWURM")
        .expect("Rotwurm setup");
    let multicolored_victim = game
        .put_on_battlefield(PlayerId(0), "RAV-GOLGARI-ROTWURM")
        .expect("black and green victim setup");
    let swamp = game
        .put_on_battlefield(PlayerId(0), "RAV-SWAMP")
        .expect("Swamp setup");
    game.begin_game().expect("fixture begins");
    game.activate_mana_ability(PlayerId(0), swamp, Color::Black)
        .expect("black mana");
    game.activate_ability(
        PlayerId(0),
        AbilityActivation {
            source: rotwurm,
            ability_id: "sacrifice-creature-target-player-life-loss",
            sacrifice_sources: vec![multicolored_victim],
            additional_tap_creatures: vec![],
            discard_cards: vec![],
            targets: vec![Target::Player(PlayerId(1))],
        },
    )
    .expect("Rotwurm sacrifices the multicolored creature");

    let ordering = game
        .view_for_player(PlayerId(0))
        .expect("Savra controller receives APNAP ordering view")
        .pending_decision
        .expect("simultaneous Savra triggers require controller ordering");
    assert_eq!(ordering.trigger_candidates.len(), 2);
    game.submit_policy_move(
        PlayerId(0),
        "test.savra-multicolor-trigger-order.v1",
        PolicyAction::SubmitDecision {
            decision: ordering.id,
            selection: DecisionSelection::TriggerOrder(ordering.trigger_candidates.clone()),
        },
    )
    .expect("Savra controller orders the independently observed triggers");

    println!(
        "Savra multicolor trigger trace: {:#?}",
        game.canonical_event_log()
    );
    for ability in [BLACK_TRIGGER, GREEN_TRIGGER] {
        assert!(
            game.event_log.iter().any(|event| matches!(
                event,
                GameEvent::TriggeredAbilityStacked { source, ability: observed, .. }
                    if *source == savra && *observed == ability
            )),
            "multicolored sacrifice must stack {ability}"
        );
    }
    game.validate_invariants()
        .expect("Savra multicolored observation preserves invariants");
}
