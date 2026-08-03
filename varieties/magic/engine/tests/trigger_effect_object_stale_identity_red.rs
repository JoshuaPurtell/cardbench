//! Red regression: legacy trigger effect-object actions must not replay across
//! otherwise-identical trigger instances from one persistent permanent.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, Color, Effect, Game, ManaCost, PlayerId, PolicyAction, Step,
    TriggerCondition, TriggeredAbility, TriggeredAbilityBinding, Zone,
};

const SOURCE: &str = "TST-STALE-TRIGGER-EFFECT-OBJECT-SOURCE";
const FILLER: &str = "TST-STALE-TRIGGER-EFFECT-OBJECT-FILLER";
const ABILITY: &str = "upkeep-discard-each";

fn definition(id: &'static str, card_types: BTreeSet<CardType>) -> CardDefinition {
    CardDefinition {
        id,
        name: id,
        set_code: "TST",
        mana_cost: ManaCost::new(0),
        colors: BTreeSet::from([Color::Black]),
        mana_colors: BTreeSet::new(),
        card_types,
        is_basic_land: false,
        supported_rules: &["trigger-effect-object-stale-identity-red"],
        power: (id == SOURCE).then_some(1),
        toughness: (id == SOURCE).then_some(1),
        keywords: vec![],
        effects: vec![],
    }
}

fn pass_pair(game: &mut Game) {
    let first = game.priority;
    game.pass_priority(first).expect("first pass");
    let second = game.priority;
    game.pass_priority(second).expect("second pass");
}

fn select_empty_discard(
    game: &mut Game,
    player: PlayerId,
    choice: &cardbench_magic_engine::TriggeredAbilityEffectObjectChoiceView,
) {
    game.submit_policy_move(
        player,
        "stale-trigger-effect-object-red",
        PolicyAction::ChooseTriggeredAbilityEffectObject {
            decision: choice.decision,
            source: choice.source,
            ability: choice.ability,
            selected: None,
        },
    )
    .expect("an empty hand permits no discard selection");
}

fn advance_to_next_upkeep(game: &mut Game, player: PlayerId) {
    for _ in 0..64 {
        if game.active_player == player && game.turn > 1 && game.step == Step::Upkeep {
            return;
        }
        if game
            .view_for_player(game.active_player)
            .expect("active-player view")
            .draw_replacement_pending
        {
            game.resolve_pending_draw(game.active_player, None)
                .expect("ordinary draw resolves");
            continue;
        }
        match game.step {
            Step::DeclareAttackers
                if !game
                    .view_for_player(game.active_player)
                    .expect("active-player view")
                    .attackers_declared =>
            {
                game.declare_attackers(game.active_player, &[])
                    .expect("empty attackers are legal");
            }
            Step::DeclareBlockers
                if !game
                    .view_for_player(PlayerId(1 - game.active_player.0))
                    .expect("defender view")
                    .blockers_declared =>
            {
                game.declare_blockers(PlayerId(1 - game.active_player.0), &[])
                    .expect("empty blockers are legal");
            }
            _ => {
                let priority = game.priority;
                game.pass_priority(priority)
                    .expect("ordinary priority passes advance the turn");
            }
        }
    }
    panic!("fixture never reached the next upkeep");
}

#[test]
#[allow(clippy::too_many_lines)] // The replay must cross both multi-player trigger lifecycles.
fn stale_trigger_effect_object_action_cannot_answer_a_later_identical_trigger() {
    let controller = PlayerId(0);
    let opponent = PlayerId(1);
    let binding = TriggeredAbilityBinding {
        card_definition: SOURCE,
        ability: TriggeredAbility {
            id: ABILITY,
            condition: TriggerCondition::BeginningOfUpkeep,
            mana_cost: ManaCost::new(0),
            optional: false,
            targets: vec![],
            effects: vec![Effect::DiscardOneCardEachPlayer],
        },
    };
    let mut game = Game::new_with_all_bindings_and_triggers(
        [
            definition(SOURCE, BTreeSet::from([CardType::Creature])),
            definition(FILLER, BTreeSet::from([CardType::Artifact])),
        ],
        2,
        [],
        [],
        [],
        [],
        [binding],
    )
    .expect("fixture initializes");
    let source = game
        .put_on_battlefield(controller, SOURCE)
        .expect("source enters before game begins");
    for owner in [controller, opponent] {
        for _ in 0..3 {
            game.add_card(owner, FILLER, Zone::Library)
                .expect("draw buffer enters before game begins");
        }
    }
    game.begin_game().expect("game begins at controller upkeep");
    pass_pair(&mut game);

    let first = game
        .view_for_player(controller)
        .expect("controller view")
        .triggered_ability_effect_object_choice
        .expect("first trigger requires controller discard choice");
    assert_eq!(first.source, source);
    assert_eq!(first.ability, ABILITY);
    assert!(first.candidates.is_empty());
    let stale_action = PolicyAction::ChooseTriggeredAbilityEffectObject {
        decision: first.decision,
        source: first.source,
        ability: first.ability,
        selected: None,
    };
    game.submit_policy_move(
        controller,
        "stale-trigger-effect-object-red",
        stale_action.clone(),
    )
    .expect("first controller empty-hand choice is accepted");
    let opponent_first = game
        .view_for_player(opponent)
        .expect("opponent view")
        .triggered_ability_effect_object_choice
        .expect("opponent receives the second empty-hand choice");
    select_empty_discard(&mut game, opponent, &opponent_first);
    pass_pair(&mut game);

    advance_to_next_upkeep(&mut game, controller);
    pass_pair(&mut game);
    let second = game
        .view_for_player(controller)
        .expect("controller view")
        .triggered_ability_effect_object_choice
        .expect("second trigger requires controller discard choice");
    assert_eq!(first.source, second.source);
    assert_eq!(first.ability, second.ability);
    assert!(second.candidates.is_empty());
    assert_ne!(first.decision, second.decision);

    let stale_result =
        game.submit_policy_move(controller, "stale-trigger-effect-object-red", stale_action);
    eprintln!(
        "stale trigger effect-object trace: first={first:?}; second={second:?}; \\
         stale_result={stale_result:?}; events={:?}",
        game.canonical_event_log()
    );
    assert!(
        stale_result.is_err(),
        "an action from an earlier identical trigger must not answer the later one"
    );
    assert_eq!(
        game.view_for_player(controller)
            .expect("controller view")
            .triggered_ability_effect_object_choice
            .expect("stale rejection preserves the later choice")
            .decision,
        second.decision
    );
    select_empty_discard(&mut game, controller, &second);
    game.validate_invariants()
        .expect("rejected stale effect-object action preserves a valid trigger state");
}
