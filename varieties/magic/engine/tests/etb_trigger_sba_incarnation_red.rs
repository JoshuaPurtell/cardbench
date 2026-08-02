//! Red regression: an ETB trigger must retain the entering battlefield object,
//! even when an immediate SBA moves that permanent before triggers are stacked.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, CastRequest, Effect, Game, GameEvent, ManaCost, PlayerId,
    TriggerCondition, TriggeredAbility, TriggeredAbilityBinding, Zone,
};

const EPHEMERAL_ETB: &str = "EPHEMERAL-ETB-INCARNATION";
const ETB_ABILITY: &str = "gain-one-on-entry";

fn ephemeral_etb() -> CardDefinition {
    CardDefinition {
        id: EPHEMERAL_ETB,
        name: EPHEMERAL_ETB,
        set_code: "TST",
        mana_cost: ManaCost::new(0),
        colors: BTreeSet::new(),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Creature]),
        is_basic_land: false,
        supported_rules: &["etb-historical-incarnation-probe"],
        power: Some(0),
        toughness: Some(0),
        keywords: vec![],
        effects: vec![],
    }
}

fn advance_to_precombat_main(game: &mut Game) {
    game.begin_game().expect("fixture starts");
    for _ in 0..2 {
        game.pass_priority(PlayerId(0))
            .expect("active player advances the automatic step");
        game.pass_priority(PlayerId(1))
            .expect("opponent advances the automatic step");
    }
}

#[test]
fn zero_toughness_etb_trigger_keeps_its_battlefield_incarnation() {
    let binding = TriggeredAbilityBinding {
        card_definition: EPHEMERAL_ETB,
        ability: TriggeredAbility {
            id: ETB_ABILITY,
            condition: TriggerCondition::EntersBattlefield,
            mana_cost: ManaCost::new(0),
            optional: false,
            targets: vec![],
            effects: vec![Effect::GainLifeController { amount: 1 }],
        },
    };
    let mut game =
        Game::new_with_all_bindings_and_triggers([ephemeral_etb()], 2, [], [], [], [], [binding])
            .expect("trigger-enabled fixture initializes");
    let creature = game
        .add_card(PlayerId(0), EPHEMERAL_ETB, Zone::Hand)
        .expect("creature begins in hand");
    advance_to_precombat_main(&mut game);
    game.cast_spell(
        PlayerId(0),
        CastRequest {
            card: creature,
            targets: vec![],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("zero-cost creature casts");
    game.pass_priority(PlayerId(0))
        .expect("caster passes the creature spell");
    game.pass_priority(PlayerId(1))
        .expect("creature resolves, dies to SBA, and stacks its ETB trigger");

    let entry_move = game
        .event_log
        .iter()
        .position(|event| matches!(event, GameEvent::CardMoved { card, to: Zone::Battlefield } if *card == creature))
        .expect("spell resolution moves the creature to the battlefield");
    let entry_incarnation = game.event_log[entry_move + 1..]
        .iter()
        .find_map(|event| match event {
            GameEvent::ObjectIncarnationAdvanced {
                object,
                incarnation,
            } if *object == creature => Some(*incarnation),
            _ => None,
        })
        .expect("battlefield entry has an incarnation receipt");
    let trigger_incarnation = game
        .event_log
        .iter()
        .find_map(|event| match event {
            GameEvent::TriggeredAbilityStacked {
                source,
                source_incarnation,
                ability,
                ..
            } if *source == creature && *ability == ETB_ABILITY => Some(*source_incarnation),
            _ => None,
        })
        .expect("the ETB trigger is stacked after the SBA boundary");

    eprintln!(
        "ETB/SBA incarnation trace: {:?}",
        game.canonical_event_log(),
    );
    assert_eq!(
        trigger_incarnation, entry_incarnation,
        "the trigger must retain the entry event's battlefield incarnation, not the later graveyard object",
    );
    game.validate_invariants()
        .expect("the historical ETB source must remain auditable");
}
