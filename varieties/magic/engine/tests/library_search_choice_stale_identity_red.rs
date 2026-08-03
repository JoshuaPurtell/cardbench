//! Red regression: the legacy library-search compatibility action must not
//! resolve a later same-card search after the original spell is returned and
//! recast.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, CastRequest, Color, Effect, Game, LibrarySearchDestination,
    LibrarySearchRequirement, LibrarySearchSelection, ManaCost, PlayerId, PolicyAction, Target,
    Zone,
};

const SEARCH: &str = "TST-STALE-LIBRARY-SEARCH";
const RETURN: &str = "TST-RETURN-STALE-LIBRARY-SEARCH";
const CANDIDATE: &str = "TST-LIBRARY-SEARCH-CANDIDATE";

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
        supported_rules: &["library-search-choice-stale-identity-red"],
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
#[allow(clippy::too_many_lines)] // The complete two-incarnation transcript proves compatibility replay is stale.
fn stale_library_search_compatibility_action_cannot_resolve_a_returned_and_recast_spell() {
    let controller = PlayerId(0);
    let mut game = Game::new(
        [
            definition(
                SEARCH,
                vec![Effect::SearchControllerLibrary {
                    requirement: LibrarySearchRequirement::CardTypes(BTreeSet::from([
                        CardType::Instant,
                    ])),
                    destination: LibrarySearchDestination::Hand,
                    selection: LibrarySearchSelection::PolicySubmitted {
                        may_fail_to_find: true,
                    },
                    reveal_selected: false,
                }],
            ),
            definition(RETURN, vec![Effect::ReturnTargetCardToHand]),
            definition(CANDIDATE, vec![]),
        ],
        2,
    )
    .expect("fixture builds");
    let search = game
        .add_card(controller, SEARCH, Zone::Hand)
        .expect("search spell begins in hand");
    let return_spell = game
        .add_card(controller, RETURN, Zone::Hand)
        .expect("return spell begins in hand");
    game.add_card(controller, CANDIDATE, Zone::Library)
        .expect("a matching candidate begins in library");
    game.begin_game().expect("game begins");

    cast(&mut game, controller, search, vec![]);
    let first_choice = game
        .view_for_player(controller)
        .expect("controller receives first library-search view")
        .library_search_choice
        .expect("first private library-search choice opens");
    assert_eq!(first_choice.source, search);
    assert_eq!(first_choice.cards.len(), 1);
    let first_incarnation = game
        .object(search)
        .expect("search remains stacked")
        .incarnation;
    game.submit_policy_move(
        controller,
        "library-search-choice-stale.first-decline.v1",
        PolicyAction::ChooseLibrarySearchCard {
            source: search,
            selected: None,
        },
    )
    .expect("first may-fail search resolves");
    assert_eq!(game.zone_of(search), Some(Zone::Graveyard));

    cast(
        &mut game,
        controller,
        return_spell,
        vec![Target::Permanent(search)],
    );
    assert_eq!(game.zone_of(search), Some(Zone::Hand));
    let returned_incarnation = game.object(search).expect("search returns").incarnation;
    assert!(returned_incarnation > first_incarnation);

    cast(&mut game, controller, search, vec![]);
    let second_view = game
        .view_for_player(controller)
        .expect("controller receives second library-search view");
    let second_choice = second_view
        .library_search_choice
        .as_ref()
        .expect("second private library-search choice opens");
    assert_eq!(second_choice.source, search);
    assert_eq!(second_choice.cards.len(), 1);
    let second_incarnation = game
        .object(search)
        .expect("search remains stacked")
        .incarnation;
    assert!(second_incarnation > returned_incarnation);
    let stale_result = game.submit_policy_move(
        controller,
        "library-search-choice-stale.replay.v1",
        PolicyAction::ChooseLibrarySearchCard {
            source: search,
            selected: None,
        },
    );

    eprintln!(
        "library-search stale-choice red trace: first_incarnation={first_incarnation}; returned_incarnation={returned_incarnation}; second_incarnation={second_incarnation}; second_view={second_view:?}; result={stale_result:?}; stack={:?}; events={:?}",
        game.stack,
        game.canonical_event_log(),
    );
    assert!(
        stale_result.is_err(),
        "a first search compatibility action must not answer the returned spell's later search"
    );
}
