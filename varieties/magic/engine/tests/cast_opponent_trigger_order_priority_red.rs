//! Red regression: casting a noncreature spell must preserve an opponent's
//! mandatory APNAP trigger-order decision before either player receives
//! priority.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, CastRequest, Color, DecisionKind, DecisionSelection, Effect, Game,
    GameEvent, ManaCost, PlayerId, TargetRequirement, TriggerCondition, TriggeredAbility,
    TriggeredAbilityBinding, Zone,
};

const SPELL: &str = "TST-CAST-OPPONENT-TRIGGER-ORDER-SPELL";
const OBSERVER: &str = "TST-CAST-OPPONENT-TRIGGER-ORDER-OBSERVER";

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
        colors: BTreeSet::from([Color::Blue]),
        mana_colors: BTreeSet::new(),
        card_types,
        is_basic_land: false,
        supported_rules: &["synthetic-cast-opponent-trigger-order"],
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
#[allow(clippy::too_many_lines)] // One cast-to-APNAP transcript verifies the no-priority handoff atomically.
fn casting_into_an_opponents_simultaneous_triggers_preserves_their_order_decision() {
    let opponent = PlayerId(1);
    let bindings = [
        TriggeredAbilityBinding {
            card_definition: OBSERVER,
            ability: TriggeredAbility {
                id: "first-observer-trigger",
                condition: TriggerCondition::FirstNoncreatureSpellCastEachTurn,
                mana_cost: ManaCost::new(0),
                optional: false,
                targets: vec![TargetRequirement::NoncreatureSpell],
                effects: vec![Effect::CounterTargetNoncreatureSpell],
            },
        },
        TriggeredAbilityBinding {
            card_definition: OBSERVER,
            ability: TriggeredAbility {
                id: "second-observer-trigger",
                condition: TriggerCondition::FirstNoncreatureSpellCastEachTurn,
                mana_cost: ManaCost::new(0),
                optional: false,
                targets: vec![TargetRequirement::NoncreatureSpell],
                effects: vec![Effect::CounterTargetNoncreatureSpell],
            },
        },
    ];
    let mut game = Game::new_with_all_bindings_and_triggers(
        [
            definition(
                SPELL,
                BTreeSet::from([CardType::Instant]),
                vec![Effect::GainLifeController { amount: 1 }],
            ),
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
    let spell = game
        .add_card(PlayerId(0), SPELL, Zone::Hand)
        .expect("caster's instant starts in hand");
    game.put_on_battlefield(opponent, OBSERVER)
        .expect("opponent's two-trigger observer starts on battlefield");
    game.begin_game().expect("game begins");
    advance_to_main_phase(&mut game);
    game.clear_event_log();

    let result = game.cast_spell(
        PlayerId(0),
        CastRequest {
            card: spell,
            targets: vec![],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    );

    println!(
        "cast/opponent-trigger-order red result={result:?}; trace={:?}",
        game.canonical_event_log()
    );
    result.expect(
        "a legal cast must not roll back when an opponent must order simultaneous triggers",
    );
    let decision = game
        .view_for_player(opponent)
        .expect("opponent view remains available")
        .pending_decision
        .expect("opponent must order its two simultaneous triggers before priority");
    assert_eq!(decision.kind, DecisionKind::TriggeredAbilityOrder);
    assert_eq!(game.priority, opponent);
    assert_eq!(
        game.stack.len(),
        1,
        "the cast spell remains below the trigger group"
    );
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::DecisionOpened { player, kind: DecisionKind::TriggeredAbilityOrder, .. }
            if *player == opponent
    )));
    game.submit_decision(
        opponent,
        decision.id,
        DecisionSelection::TriggerOrder(decision.trigger_candidates.clone()),
    )
    .expect("the opponent may order its observed simultaneous triggers");
    assert!(
        game.view_for_player(opponent)
            .expect("opponent view remains available")
            .pending_decision
            .is_none(),
        "the no-priority decision closes before trigger objects are stacked"
    );
    assert_eq!(
        game.stack.len(),
        3,
        "the cast spell remains below both ordered opponent trigger objects"
    );
    game.validate_invariants()
        .expect("the no-priority trigger-order boundary is invariant-valid");
}
