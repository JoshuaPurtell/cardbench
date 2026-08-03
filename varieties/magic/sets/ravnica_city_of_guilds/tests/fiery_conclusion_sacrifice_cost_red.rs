//! Public Fiery Conclusion cost, stack, event, and rollback contract.

use cardbench_magic_engine::{
    BasicLandManaAbilityActivation, CardType, CastPaymentManaAbility, CastRequest, Color, Effect,
    Game, GameEvent, ManaCost, PlayerId, RulesError, Target, TargetRequirement, Zone,
};
use cardbench_magic_rav::{
    RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, rav_additional_spell_cost_bindings,
    rav_basic_land_type_bindings, rav_mana_ability_bindings,
};

fn game() -> Game {
    Game::new_with_mana_abilities_basic_land_types_and_additional_spell_costs(
        card_definitions(),
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
    )
    .expect("RAV catalog and Fiery cost binding construct")
}

#[test]
fn fiery_conclusion_definition_declares_the_complete_bound_cost_and_damage_slice() {
    let conclusion = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-FIERY-CONCLUSION")
        .expect("Fiery Conclusion definition");
    assert_eq!(conclusion.mana_cost, ManaCost::with_colors(1, [Color::Red]));
    assert_eq!(conclusion.card_types, [CardType::Instant].into());
    assert_eq!(
        conclusion.supported_rules,
        [
            "full-rules-fidelity",
            "additional-sacrifice-controlled-creature-cost",
            "targeted-creature-damage",
        ]
    );
    assert_eq!(
        conclusion.effects,
        vec![Effect::DealDamage {
            amount: 5,
            target: TargetRequirement::Creature,
        }]
    );
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&conclusion.id));
}

#[test]
fn fiery_conclusion_rejects_a_cast_without_a_controlled_creature_sacrifice() {
    let mut game = game();
    let conclusion = game
        .add_card(PlayerId(0), "RAV-FIERY-CONCLUSION", Zone::Hand)
        .expect("Fiery Conclusion enters hand");
    let target = game
        .put_on_battlefield(PlayerId(1), "RAV-GOLGARI-BROWNSCALE")
        .expect("opponent target enters battlefield");
    game.grant_mana(PlayerId(0), Color::Red, 2)
        .expect("fixture mana");
    game.clear_event_log();

    let result = game.cast_spell(
        PlayerId(0),
        CastRequest {
            card: conclusion,
            targets: vec![Target::Permanent(target)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    );

    println!("Fiery Conclusion missing-sacrifice result: {result:?}");
    println!(
        "Fiery Conclusion missing-sacrifice events: {:?}",
        game.event_log
    );
    assert!(
        matches!(result, Err(RulesError::IllegalAction(_))),
        "Fiery Conclusion must fail closed until its required creature sacrifice is supplied"
    );
    assert_eq!(game.zone_of(conclusion), Some(Zone::Hand));
    assert!(game.stack.is_empty());
    assert!(game.event_log.is_empty());
}

#[test]
fn fiery_conclusion_sacrifices_before_its_single_effect_target_reaches_the_stack() {
    let mut game = game();
    let conclusion = game
        .add_card(PlayerId(0), "RAV-FIERY-CONCLUSION", Zone::Hand)
        .expect("Fiery Conclusion enters hand");
    let target = game
        .put_on_battlefield(PlayerId(1), "RAV-GOLGARI-BROWNSCALE")
        .expect("opponent target enters battlefield");
    let sacrifice = game
        .put_on_battlefield(PlayerId(0), "RAV-FRENZIED-GOBLIN")
        .expect("controlled creature enters battlefield");
    game.grant_mana(PlayerId(0), Color::Red, 2)
        .expect("fixture mana");
    game.clear_event_log();

    game.cast_spell(
        PlayerId(0),
        CastRequest {
            card: conclusion,
            targets: vec![
                Target::Permanent(target),
                Target::SacrificePermanent(sacrifice),
            ],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("chosen controlled creature pays Fiery Conclusion's additional cost");

    println!("Fiery Conclusion cast trace: {:?}", game.event_log);
    assert_eq!(game.zone_of(sacrifice), Some(Zone::Graveyard));
    assert_eq!(game.stack.len(), 1);
    assert_eq!(game.stack[0].targets, vec![Target::Permanent(target)]);
    assert_eq!(
        game.event_log,
        vec![
            GameEvent::SacrificedAsAdditionalSpellCost {
                player: PlayerId(0),
                card: conclusion,
                permanent: sacrifice,
            },
            GameEvent::CardMoved {
                card: sacrifice,
                to: Zone::Graveyard,
            },
            GameEvent::ObjectIncarnationAdvanced {
                object: sacrifice,
                incarnation: 2,
            },
            GameEvent::CreatureCardPutIntoGraveyardFromBattlefieldThisTurn {
                turn: 1,
                player: PlayerId(0),
                card: sacrifice,
                incarnation: 2,
            },
            GameEvent::SpellCast {
                player: PlayerId(0),
                card: conclusion,
            },
        ]
    );
    game.validate_invariants()
        .expect("cost receipt and stack boundary are invariant-valid");
}

#[test]
fn fiery_conclusion_rolls_back_its_sacrifice_when_a_later_payment_activation_fails() {
    let mut game = game();
    let conclusion = game
        .add_card(PlayerId(0), "RAV-FIERY-CONCLUSION", Zone::Hand)
        .expect("Fiery Conclusion enters hand");
    let target = game
        .put_on_battlefield(PlayerId(1), "RAV-GOLGARI-BROWNSCALE")
        .expect("opponent target enters battlefield");
    let sacrifice = game
        .put_on_battlefield(PlayerId(0), "RAV-FRENZIED-GOBLIN")
        .expect("controlled creature enters battlefield");
    let opponent_forest = game
        .put_on_battlefield(PlayerId(1), "RAV-FOREST")
        .expect("opponent typed land enters battlefield");
    game.clear_event_log();

    let result = game.cast_spell(
        PlayerId(0),
        CastRequest {
            card: conclusion,
            targets: vec![
                Target::Permanent(target),
                Target::SacrificePermanent(sacrifice),
            ],
            convoke: vec![],
            payment_mana_abilities: vec![CastPaymentManaAbility::BasicLand(
                BasicLandManaAbilityActivation {
                    land: opponent_forest,
                    color: Color::Green,
                },
            )],
        },
    );

    println!("Fiery Conclusion later-payment rollback result: {result:?}");
    println!(
        "Fiery Conclusion later-payment rollback events: {:?}",
        game.event_log
    );
    assert!(matches!(result, Err(RulesError::IllegalAction(_))));
    assert_eq!(game.zone_of(conclusion), Some(Zone::Hand));
    assert_eq!(game.zone_of(sacrifice), Some(Zone::Battlefield));
    assert!(!game.object(sacrifice).expect("sacrifice object").tapped);
    assert!(game.stack.is_empty());
    assert!(game.event_log.is_empty());
    game.validate_invariants()
        .expect("failed cast leaves a valid pre-cast state");
}
