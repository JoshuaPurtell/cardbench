//! Red milestone for Excruciator's static damage-prevention exception.
//!
//! The test intentionally uses the existing Ordruun Commando shield as a
//! receiver.  Excruciator's combat damage must ignore that shield while the
//! ordered event log still records the full combat damage amount.

use cardbench_magic_engine::{
    CardType, Color, CombatBlock, Game, GameEvent, Keyword, ManaCost, PlayerId, Step, Zone,
};
use cardbench_magic_rav::{
    RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, rav_activated_ability_bindings,
    rav_additional_spell_cost_bindings, rav_basic_land_type_bindings, rav_mana_ability_bindings,
};

fn advance_to_precombat_main(game: &mut Game) {
    game.begin_game().expect("game starts");
    for _ in 0..2 {
        game.pass_priority(PlayerId(0))
            .expect("active player passes");
        game.pass_priority(PlayerId(1)).expect("opponent passes");
    }
    assert_eq!(game.step, Step::PrecombatMain);
}

#[test]
fn excruciator_declares_full_static_damage_exception() {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-EXCRUCIATOR")
        .expect("Excruciator definition exists");
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&definition.id));
    assert_eq!(definition.name, "Excruciator");
    assert_eq!(
        definition.mana_cost,
        ManaCost::with_colors(6, [Color::Red, Color::Red])
    );
    assert_eq!(
        definition.card_types,
        [CardType::Creature].into_iter().collect()
    );
    assert_eq!((definition.power, definition.toughness), (Some(7), Some(7)));
    assert!(
        definition
            .keywords
            .contains(&Keyword::DamageCannotBePrevented)
    );
    assert!(
        definition
            .supported_rules
            .contains(&"damage-cannot-be-prevented")
    );
}

#[test]
fn excruciator_combat_damage_ignores_ordruun_shield() {
    let mut game = Game::new_with_all_bindings(
        card_definitions(),
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        rav_activated_ability_bindings(),
    )
    .expect("RAV game builds");
    let excruciator = game
        .put_on_battlefield(PlayerId(0), "RAV-EXCRUCIATOR")
        .expect("Excruciator enters");
    let commando = game
        .put_on_battlefield(PlayerId(1), "RAV-ORDRUUN-COMMANDO")
        .expect("Ordruun enters");
    let plains = game
        .put_on_battlefield(PlayerId(1), "RAV-PLAINS")
        .expect("Plains enters");
    game.set_entered_turn_for_setup(excruciator, 0)
        .expect("Excruciator is long-controlled");
    game.set_entered_turn_for_setup(commando, 0)
        .expect("Ordruun is long-controlled");

    advance_to_precombat_main(&mut game);
    // Let the defending player use Ordruun's white activation during the
    // active player's precombat main phase, so its end-of-turn shield covers
    // the combat that follows.
    game.pass_priority(PlayerId(0))
        .expect("active player yields");
    game.activate_mana_ability(PlayerId(1), plains, Color::White)
        .expect("white activation mana");
    game.activate_ability(
        PlayerId(1),
        cardbench_magic_engine::AbilityActivation {
            source: commando,
            ability_id: "prevent-one-damage-to-self",
            sacrifice_sources: vec![],
            discard_cards: vec![],
            targets: vec![],
        },
    )
    .expect("Ordruun shield activates");
    game.pass_priority(PlayerId(1))
        .expect("Ordruun controller passes");
    game.pass_priority(PlayerId(0))
        .expect("Ordruun shield resolves");
    assert_eq!(
        game.object(commando).expect("Ordruun exists").damage_shield,
        1
    );
    while game.step != Step::DeclareAttackers {
        game.pass_priority(game.priority)
            .expect("advance to attackers");
    }

    game.declare_attackers(PlayerId(0), &[excruciator])
        .expect("Excruciator attacks");
    game.pass_priority(PlayerId(0)).expect("attacker passes");
    game.pass_priority(PlayerId(1))
        .expect("defender declares blockers");
    game.declare_blockers(
        PlayerId(1),
        &[CombatBlock {
            attacker: excruciator,
            blocker: commando,
        }],
    )
    .expect("Ordruun blocks");
    // Resolve the normal combat-damage step and then let SBAs finish.
    for _ in 0..4 {
        if game.step == Step::DeclareAttackers {
            break;
        }
        game.pass_priority(game.priority)
            .expect("combat priority pass");
        if game.zone_of(commando) == Some(Zone::Graveyard) {
            break;
        }
    }

    println!("Excruciator combat events: {:?}", game.event_log);
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::DamageDealtToPermanent { source, permanent, amount: 7 }
            if *source == excruciator && *permanent == commando
    )));
    assert!(!game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::DamagePrevented { source, target, .. }
            if *source == excruciator && *target == cardbench_magic_engine::Target::Permanent(commando)
    )));
    game.validate_invariants()
        .expect("Excruciator trace is valid");
}
