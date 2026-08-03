//! Red discovery contract for Phytohydra's self-damage replacement.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardType, CastRequest, Color, CombatBlock, CounterKind, DamageReplacementChoice, Game,
    GameEvent, ManaCost, PlayerId, Step, Target, Zone,
};
use cardbench_magic_rav::{
    RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, executable_definition_id_for_collector,
    rav_damage_replacement_effect_bindings,
};

#[test]
fn phytohydra_requires_a_self_damage_prevention_and_counter_replacement() {
    let phytohydra = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-PHYTOHYDRA")
        .expect("Phytohydra definition exists");

    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&phytohydra.id));
    assert_eq!(phytohydra.name, "Phytohydra");
    assert_eq!(
        phytohydra.mana_cost,
        ManaCost::with_colors(2, [Color::Green, Color::White, Color::White])
    );
    assert_eq!(
        phytohydra.colors,
        BTreeSet::from([Color::Green, Color::White])
    );
    assert_eq!(phytohydra.card_types, BTreeSet::from([CardType::Creature]));
    assert_eq!((phytohydra.power, phytohydra.toughness), (Some(1), Some(1)));
    assert!(
        phytohydra
            .supported_rules
            .contains(&"self-damage-prevention-plus-one-counters")
    );
    assert_eq!(
        executable_definition_id_for_collector(218),
        Ok("RAV-PHYTOHYDRA")
    );
    assert!(
        rav_damage_replacement_effect_bindings()
            .iter()
            .any(|binding| binding.source_definition == phytohydra.id)
    );
}

fn pass_pair(game: &mut Game) {
    let first = game.priority;
    game.pass_priority(first).expect("first priority pass");
    let second = game.priority;
    game.pass_priority(second).expect("second priority pass");
}

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
fn phytohydra_prevents_its_damage_then_places_matching_counters() {
    let mut game = Game::new_with_all_bindings(card_definitions(), 2, [], [], [], [])
        .expect("RAV fixture builds");
    game.register_damage_replacement_effect_bindings(rav_damage_replacement_effect_bindings())
        .expect("RAV replacement bindings register");
    let phytohydra = game
        .put_on_battlefield(PlayerId(0), "RAV-PHYTOHYDRA")
        .expect("Phytohydra setup");
    let char = game
        .add_card(PlayerId(1), "RAV-CHAR", Zone::Hand)
        .expect("damage source setup");
    game.grant_mana(PlayerId(1), Color::Red, 3)
        .expect("damage payment setup");

    game.pass_priority(PlayerId(0))
        .expect("Phytohydra controller yields priority");
    game.cast_spell(
        PlayerId(1),
        CastRequest {
            card: char,
            targets: vec![Target::Permanent(phytohydra)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("damage spell casts");
    pass_pair(&mut game);

    println!("phytohydra_event_log={:?}", game.canonical_event_log());
    assert_eq!(
        game.object(phytohydra)
            .expect("Phytohydra remains live")
            .damage,
        0,
        "the replaced packet cannot mark damage"
    );
    assert_eq!(
        game.characteristics(phytohydra)
            .expect("Phytohydra characteristics")
            .power,
        Some(5),
        "four prevented damage produces four +1/+1 counters"
    );
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::DamagePreventedWithPlusOneCounters {
            source,
            permanent,
            amount: 4,
            ..
        } if *source == char && *permanent == phytohydra
    )));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::CounterPlaced {
            source,
            card,
            counter: CounterKind::PlusOnePlusOne,
            amount: 4,
        } if *source == phytohydra && *card == phytohydra
    )));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::DamageReplacementApplied {
            target: Target::Permanent(target),
            replacement:
                DamageReplacementChoice::PreventSelfDamageAndAddPlusOneCounters {
                    source,
                    ..
                },
            ..
        } if *target == phytohydra && *source == phytohydra
    )));
    assert!(!game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::DamageDealtToPermanent { source, permanent, .. }
            if *source == char && *permanent == phytohydra
    )));
    game.validate_invariants()
        .expect("Phytohydra replacement trace preserves invariants");
}

#[test]
fn damage_that_cannot_be_prevented_bypasses_phytohydras_replacement() {
    let mut game = Game::new_with_all_bindings(card_definitions(), 2, [], [], [], [])
        .expect("RAV fixture builds");
    game.register_damage_replacement_effect_bindings(rav_damage_replacement_effect_bindings())
        .expect("RAV replacement bindings register");
    let excruciator = game
        .put_on_battlefield(PlayerId(0), "RAV-EXCRUCIATOR")
        .expect("Excruciator setup");
    let phytohydra = game
        .put_on_battlefield(PlayerId(1), "RAV-PHYTOHYDRA")
        .expect("Phytohydra setup");
    game.set_entered_turn_for_setup(excruciator, 0)
        .expect("Excruciator is long-controlled");
    game.set_entered_turn_for_setup(phytohydra, 0)
        .expect("Phytohydra is long-controlled");
    advance_to_precombat_main(&mut game);
    while game.step != Step::DeclareAttackers {
        game.pass_priority(game.priority)
            .expect("advance to attackers");
    }
    game.declare_attackers(PlayerId(0), &[excruciator])
        .expect("Excruciator attacks");
    game.pass_priority(PlayerId(0)).expect("attacker passes");
    game.pass_priority(PlayerId(1))
        .expect("advance to blockers");
    game.declare_blockers(
        PlayerId(1),
        &[CombatBlock {
            attacker: excruciator,
            blocker: phytohydra,
        }],
    )
    .expect("Phytohydra blocks");
    for _ in 0..4 {
        if game.zone_of(phytohydra) == Some(Zone::Graveyard) {
            break;
        }
        game.pass_priority(game.priority)
            .expect("advance through combat damage");
    }

    println!(
        "phytohydra_unpreventable_event_log={:?}",
        game.canonical_event_log()
    );
    assert_eq!(game.zone_of(phytohydra), Some(Zone::Graveyard));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::DamageDealtToPermanent { source, permanent, amount: 7 }
            if *source == excruciator && *permanent == phytohydra
    )));
    assert!(!game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::DamagePreventedWithPlusOneCounters { source, permanent, .. }
            if *source == excruciator && *permanent == phytohydra
    )));
    game.validate_invariants()
        .expect("unpreventable damage remains a valid state-machine trace");
}
