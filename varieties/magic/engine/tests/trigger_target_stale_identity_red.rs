//! Red regression: legacy trigger-target actions must not replay across
//! otherwise-identical trigger instances from one permanent.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, Color, Effect, Game, ManaCost, PlayerId, PolicyAction, Step, Target,
    TargetRequirement, TriggerCondition, TriggeredAbility, TriggeredAbilityBinding, Zone,
};

const SOURCE: &str = "TST-STALE-TRIGGER-TARGET-SOURCE";
const FILLER: &str = "TST-STALE-TRIGGER-TARGET-FILLER";
const ABILITY: &str = "upkeep-target-player";

fn definition(id: &'static str, card_types: BTreeSet<CardType>) -> CardDefinition {
    CardDefinition {
        id,
        name: id,
        set_code: "TST",
        mana_cost: ManaCost::new(0),
        colors: BTreeSet::from([Color::Red]),
        mana_colors: BTreeSet::new(),
        card_types,
        is_basic_land: false,
        supported_rules: &["trigger-target-stale-identity-red"],
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
#[allow(clippy::too_many_lines)] // The replay must cross both full trigger lifecycles.
fn stale_trigger_target_action_cannot_answer_a_later_identical_upkeep_trigger() {
    let controller = PlayerId(0);
    let opponent = PlayerId(1);
    let binding = TriggeredAbilityBinding {
        card_definition: SOURCE,
        ability: TriggeredAbility {
            id: ABILITY,
            condition: TriggerCondition::BeginningOfUpkeep,
            mana_cost: ManaCost::new(0),
            optional: false,
            targets: vec![TargetRequirement::Player],
            effects: vec![Effect::DealDamage {
                amount: 1,
                target: TargetRequirement::Player,
            }],
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

    let first = game
        .view_for_player(controller)
        .expect("controller view")
        .triggered_ability_target_choice
        .expect("first upkeep trigger requires a target");
    assert_eq!(first.source, source);
    let stale_action = PolicyAction::ChooseTriggeredAbilityTargets {
        decision: first.decision,
        source: first.source,
        ability: first.ability,
        targets: vec![Target::Player(opponent)],
    };
    game.submit_policy_move(controller, "stale-trigger-target-red", stale_action.clone())
        .expect("first trigger target is accepted");
    pass_pair(&mut game);
    assert_eq!(game.player(opponent).expect("opponent").life, 19);

    advance_to_next_upkeep(&mut game, controller);
    let second = game
        .view_for_player(controller)
        .expect("controller view")
        .triggered_ability_target_choice
        .expect("second upkeep trigger requires a target");
    assert_eq!(first.source, second.source);
    assert_eq!(first.ability, second.ability);
    assert_eq!(first.target_options, second.target_options);
    assert_ne!(first.decision, second.decision);

    let stale_result =
        game.submit_policy_move(controller, "stale-trigger-target-red", stale_action);
    eprintln!(
        "stale trigger-target trace: first={first:?}; second={second:?}; \\
         stale_result={stale_result:?}; events={:?}",
        game.canonical_event_log()
    );
    assert!(
        stale_result.is_err(),
        "a target action from an earlier identical trigger must not answer the later trigger"
    );
    assert_eq!(
        game.view_for_player(controller)
            .expect("controller view")
            .triggered_ability_target_choice
            .expect("stale rejection preserves the later choice")
            .decision,
        second.decision
    );
    game.submit_policy_move(
        controller,
        "stale-trigger-target-red",
        PolicyAction::ChooseTriggeredAbilityTargets {
            decision: second.decision,
            source: second.source,
            ability: second.ability,
            targets: vec![Target::Player(opponent)],
        },
    )
    .expect("the current decision identity targets the later trigger");
    game.validate_invariants()
        .expect("rejected stale target action preserves a valid trigger state");
}
