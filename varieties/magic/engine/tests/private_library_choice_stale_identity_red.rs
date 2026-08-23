//! Red regression: a private library response must not survive its spell's
//! graveyard-to-hand round trip and answer a later stack incarnation.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, CastRequest, Color, Effect, Game, ManaCost, PlayerId, PolicyAction,
    Target, Zone,
};

const LOOK: &str = "TST-PRIVATE-LOOK";
const RETURN: &str = "TST-RETURN-LOOK";
const FIRST_TOP: &str = "TST-FIRST-TOP";
const SECOND_TOP: &str = "TST-SECOND-TOP";

fn definition(id: &'static str, effects: Vec<Effect>) -> CardDefinition {
    CardDefinition {
        id,
        name: id,
        set_code: "TST",
        mana_cost: ManaCost::new(0),
        colors: BTreeSet::from([Color::Blue]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Instant]),
        is_basic_land: false,
        supported_rules: &["private-library-choice-stale-identity-red"],
        power: None,
        toughness: None,
        keywords: vec![],
        effects,
    }
}

fn pass_pair(game: &mut Game) {
    let first = game.priority;
    game.pass_priority(first).expect("first player passes");
    let second = game.priority;
    game.pass_priority(second).expect("second player passes");
}

fn cast(
    game: &mut Game,
    controller: PlayerId,
    card: cardbench_magic_engine::ObjectId,
    targets: Vec<Target>,
) {
    game.cast_spell(
        controller,
        CastRequest {
            card,
            targets,
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("zero-cost instant casts");
    pass_pair(game);
}

#[test]
#[allow(clippy::too_many_lines)] // The complete two-incarnation transcript proves the stale action crosses no other boundary.
fn stale_private_library_selection_cannot_resolve_a_returned_and_recast_spell() {
    let controller = PlayerId(0);
    let mut game = Game::new(
        [
            definition(
                LOOK,
                vec![Effect::LookAtTopCardsChooseForLifeOrGraveyard {
                    count: 1,
                    life_per_card: 1,
                }],
            ),
            definition(RETURN, vec![Effect::ReturnTargetCardToHand]),
            definition(FIRST_TOP, vec![]),
            definition(SECOND_TOP, vec![]),
        ],
        2,
    )
    .expect("fixture builds");
    let look = game
        .add_card(controller, LOOK, Zone::Hand)
        .expect("look spell begins in hand");
    let return_spell = game
        .add_card(controller, RETURN, Zone::Hand)
        .expect("return spell begins in hand");
    game.add_card(controller, SECOND_TOP, Zone::Library)
        .expect("second top begins below first");
    let first_top = game
        .add_card(controller, FIRST_TOP, Zone::Library)
        .expect("first top begins on top");
    game.begin_game().expect("game begins");

    cast(&mut game, controller, look, vec![]);
    let first_view = game
        .view_for_player(controller)
        .expect("controller receives first private view");
    let first_choice = first_view
        .private_library_choice
        .expect("first private choice is visible only to its controller");
    assert_eq!(first_choice.cards.len(), 1);
    assert_eq!(first_choice.cards[0].id, first_top);
    let first_incarnation = game.object(look).expect("look remains stacked").incarnation;
    game.submit_policy_move(
        controller,
        "private-library-choice-stale.first-empty.v1",
        PolicyAction::ChoosePrivateLibraryCards {
            decision: first_choice.decision,
            spell: look,
            selected: vec![],
        },
    )
    .expect("first private choice resolves");
    assert_eq!(game.zone_of(look), Some(Zone::Graveyard));

    cast(
        &mut game,
        controller,
        return_spell,
        vec![Target::Permanent(look)],
    );
    assert_eq!(game.zone_of(look), Some(Zone::Hand));
    let returned_incarnation = game.object(look).expect("look returns").incarnation;
    assert!(returned_incarnation > first_incarnation);

    cast(&mut game, controller, look, vec![]);
    let second_view = game
        .view_for_player(controller)
        .expect("controller receives second private view");
    let second_choice = second_view
        .private_library_choice
        .as_ref()
        .expect("second private choice is visible only to its controller");
    assert_eq!(second_choice.cards.len(), 1);
    let second_decision = second_choice.decision;
    assert_ne!(
        first_choice.decision, second_decision,
        "a new suspended resolution receives a fresh decision identity"
    );
    let stale_result = game.submit_policy_move(
        controller,
        "private-library-choice-stale.replay.v1",
        PolicyAction::ChoosePrivateLibraryCards {
            decision: first_choice.decision,
            spell: look,
            selected: vec![],
        },
    );

    eprintln!(
        "private-library stale-choice red trace: first_incarnation={first_incarnation}; returned_incarnation={returned_incarnation}; second_view={second_view:?}; result={stale_result:?}; stack={:?}; events={:?}",
        game.stack,
        game.canonical_event_log(),
    );
    assert!(
        stale_result.is_err(),
        "a first-incarnation private choice must not answer the returned spell's new stack incarnation"
    );
    game.submit_policy_move(
        controller,
        "private-library-choice-stale.second-empty.v1",
        PolicyAction::ChoosePrivateLibraryCards {
            decision: second_decision,
            spell: look,
            selected: vec![],
        },
    )
    .expect("the current private choice resolves with its own identity");
    game.validate_invariants()
        .expect("current private choice preserves engine invariants");
}
