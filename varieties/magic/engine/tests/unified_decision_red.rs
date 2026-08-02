//! Red regression for stale-safe, typed policy decision identities.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, Color, Effect, Game, ManaCost, PlayerId, PolicyAction, Step,
    TriggerCondition, TriggeredAbility, TriggeredAbilityBinding, Zone,
};

const SOURCE: &str = "TST-DECISION-SOURCE";
const CREATURE: &str = "TST-DECISION-CREATURE";

fn definitions() -> Vec<CardDefinition> {
    [SOURCE, CREATURE]
        .into_iter()
        .map(|id| CardDefinition {
            id,
            name: id,
            set_code: "TST",
            mana_cost: ManaCost::new(0),
            colors: BTreeSet::from([Color::Green]),
            mana_colors: BTreeSet::new(),
            card_types: BTreeSet::from([CardType::Creature]),
            is_basic_land: false,
            supported_rules: &["test-only"],
            power: Some(1),
            toughness: Some(1),
            keywords: vec![],
            effects: vec![],
        })
        .collect()
}

fn pass_pair(game: &mut Game) {
    let first = game.priority;
    game.pass_priority(first).expect("first player passes");
    let second = game.priority;
    game.pass_priority(second).expect("second player passes");
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
    panic!("the fixture never reached the next upkeep");
}

#[test]
fn stale_public_sacrifice_submission_cannot_be_distinguished_from_a_new_prompt() {
    let binding = TriggeredAbilityBinding {
        card_definition: SOURCE,
        ability: TriggeredAbility {
            id: "upkeep-sacrifice",
            condition: TriggerCondition::BeginningOfUpkeep,
            mana_cost: ManaCost::new(0),
            optional: false,
            targets: vec![],
            effects: vec![Effect::SacrificeControllerCreature],
        },
    };
    let player = PlayerId(0);
    let mut game =
        Game::new_with_all_bindings_and_triggers(definitions(), 2, [], [], [], [], [binding])
            .expect("synthetic decision fixture builds");
    let source = game
        .put_on_battlefield(player, SOURCE)
        .expect("trigger source enters before the game");
    let first_sacrifice = game
        .put_on_battlefield(player, CREATURE)
        .expect("first public sacrifice candidate enters");
    let stale_selection = game
        .put_on_battlefield(player, CREATURE)
        .expect("second public sacrifice candidate enters");
    for owner in [PlayerId(0), PlayerId(1)] {
        for _ in 0..3 {
            game.add_card(owner, CREATURE, Zone::Library)
                .expect("draw buffer enters before the game");
        }
    }
    game.begin_game().expect("game begins at upkeep");

    pass_pair(&mut game);
    game.submit_policy_move(
        player,
        "test.first-prompt.v1",
        PolicyAction::ChooseTriggeredAbilityEffectObject {
            source,
            ability: "upkeep-sacrifice",
            selected: Some(first_sacrifice),
        },
    )
    .expect("the first prompt resolves through the legacy specialized action");
    assert_eq!(game.zone_of(first_sacrifice), Some(Zone::Graveyard));

    advance_to_next_upkeep(&mut game, player);
    pass_pair(&mut game);
    assert_eq!(game.zone_of(stale_selection), Some(Zone::Battlefield));

    // This action has exactly the same public shape as the first prompt. It
    // originated before this later upkeep decision existed, but the current
    // API contains no monotonic decision identity with which to reject it.
    let result = game.submit_policy_move(
        player,
        "test.stale-prompt.v1",
        PolicyAction::ChooseTriggeredAbilityEffectObject {
            source,
            ability: "upkeep-sacrifice",
            selected: Some(stale_selection),
        },
    );
    println!(
        "stale unified-decision probe: result={result:?}; state={:#?}; events={:#?}",
        game.view_for_player(player),
        game.canonical_event_log()
    );
    assert!(
        result.is_err(),
        "a submission produced for an earlier prompt must be rejected once a later decision opens"
    );
    assert_eq!(game.zone_of(stale_selection), Some(Zone::Battlefield));
}
