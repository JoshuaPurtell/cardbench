//! Red regression: an APNAP trigger-order decision must return priority to
//! the player who held it before the no-priority placement boundary.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, Color, DecisionKind, DecisionSelection, Effect, Game, ManaCost,
    PlayerId, TriggerCondition, TriggeredAbility, TriggeredAbilityBinding, Zone,
};

const LAND: &str = "TST-TRIGGER-ORDER-HANDOFF-LAND";
const OBSERVER: &str = "TST-TRIGGER-ORDER-HANDOFF-OBSERVER";

fn definition(
    id: &'static str,
    card_types: BTreeSet<CardType>,
    effects: Vec<Effect>,
) -> CardDefinition {
    CardDefinition {
        id,
        name: id,
        set_code: "TST",
        mana_cost: ManaCost::new(0),
        colors: BTreeSet::new(),
        mana_colors: if id == LAND {
            BTreeSet::from([Color::Green])
        } else {
            BTreeSet::new()
        },
        card_types,
        is_basic_land: id == LAND,
        supported_rules: &["synthetic-trigger-order-priority-handoff"],
        power: (id == OBSERVER).then_some(1),
        toughness: (id == OBSERVER).then_some(1),
        keywords: vec![],
        effects,
    }
}

fn advance_to_main_phase(game: &mut Game) {
    for player in [PlayerId(0), PlayerId(1), PlayerId(0), PlayerId(1)] {
        game.pass_priority(player)
            .expect("advance to active player's precombat main phase");
    }
}

#[test]
fn resolving_an_opponents_landfall_order_returns_priority_to_the_land_player() {
    let active = PlayerId(0);
    let opponent = PlayerId(1);
    let bindings = [
        TriggeredAbilityBinding {
            card_definition: OBSERVER,
            ability: TriggeredAbility {
                id: "first-landfall",
                condition: TriggerCondition::LandEntersBattlefield,
                mana_cost: ManaCost::new(0),
                optional: false,
                targets: vec![],
                effects: vec![Effect::GainLifeController { amount: 1 }],
            },
        },
        TriggeredAbilityBinding {
            card_definition: OBSERVER,
            ability: TriggeredAbility {
                id: "second-landfall",
                condition: TriggerCondition::LandEntersBattlefield,
                mana_cost: ManaCost::new(0),
                optional: false,
                targets: vec![],
                effects: vec![Effect::GainLifeController { amount: 1 }],
            },
        },
    ];
    let mut game = Game::new_with_all_bindings_and_triggers(
        [
            definition(LAND, BTreeSet::from([CardType::Land]), vec![]),
            definition(OBSERVER, BTreeSet::from([CardType::Creature]), vec![]),
        ],
        2,
        [],
        [],
        [],
        [],
        bindings,
    )
    .expect("fixture initializes");
    let land = game
        .add_card(active, LAND, Zone::Hand)
        .expect("active player's land starts in hand");
    game.put_on_battlefield(opponent, OBSERVER)
        .expect("opponent's two-trigger observer starts on battlefield");
    game.begin_game().expect("game begins");
    advance_to_main_phase(&mut game);

    game.play_land(active, land)
        .expect("land enters and opens the opponent's APNAP order decision");
    let decision = game
        .view_for_player(opponent)
        .expect("opponent view remains available")
        .pending_decision
        .expect("opponent must order its simultaneous landfall triggers");
    assert_eq!(decision.kind, DecisionKind::TriggeredAbilityOrder);
    game.submit_decision(
        opponent,
        decision.id,
        DecisionSelection::TriggerOrder(decision.trigger_candidates.clone()),
    )
    .expect("opponent orders its simultaneous triggers");

    println!(
        "trigger-order handoff red priority={:?}; trace={:?}",
        game.priority,
        game.canonical_event_log()
    );
    assert_eq!(
        game.priority, active,
        "APNAP ordering is not a priority action; the pre-decision land player receives priority after trigger placement"
    );
    assert_eq!(game.stack.len(), 2, "both triggers remain on the stack");
    game.validate_invariants()
        .expect("the restored priority handoff is invariant-valid");
}
