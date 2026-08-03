//! Red regression: Transmute candidates are hidden until the resolving
//! controller actually searches their library.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, Color, Game, Keyword, ManaCost, PlayerId, Zone,
};

const TRANSMUTER: &str = "TST-TRANSMUTE-HIDDEN-SEARCH";
const MATCH: &str = "TST-TRANSMUTE-HIDDEN-MATCH";
const NONMATCH: &str = "TST-TRANSMUTE-HIDDEN-NONMATCH";

fn definition(
    id: &'static str,
    mana_cost: ManaCost,
    keywords: Vec<Keyword>,
) -> CardDefinition {
    CardDefinition {
        id,
        name: id,
        set_code: "TST",
        mana_cost,
        colors: BTreeSet::from([Color::Blue]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Creature]),
        is_basic_land: false,
        supported_rules: &["transmute-hidden-search-red"],
        power: Some(1),
        toughness: Some(1),
        keywords,
        effects: vec![],
    }
}

fn definitions() -> Vec<CardDefinition> {
    vec![
        definition(
            TRANSMUTER,
            ManaCost::with_colors(1, [Color::Blue, Color::Blue]),
            vec![Keyword::Transmute(ManaCost::with_colors(
                1,
                [Color::Blue, Color::Blue],
            ))],
        ),
        definition(
            MATCH,
            ManaCost::with_colors(1, [Color::Green, Color::Green]),
            vec![],
        ),
        definition(NONMATCH, ManaCost::new(0), vec![]),
    ]
}

#[test]
fn transmute_does_not_reveal_private_library_matches_before_activation() {
    let controller = PlayerId(0);
    let mut game = Game::new(definitions(), 2).expect("fixture initializes");
    game.add_card(controller, TRANSMUTER, Zone::Hand)
        .expect("transmuter enters hand");
    let matching_card = game
        .add_card(controller, MATCH, Zone::Library)
        .expect("matching card remains hidden in library");
    let nonmatching_card = game
        .add_card(controller, NONMATCH, Zone::Library)
        .expect("nonmatching card remains hidden in library");
    game.begin_game().expect("live game begins");

    let view = game
        .view_for_player(controller)
        .expect("controller may request an ordinary policy view");
    eprintln!(
        "pre-activation transmute visibility red: searches={:?}; matching={matching_card:?}; nonmatching={nonmatching_card:?}",
        view.transmute_searches
    );
    assert!(
        view.transmute_searches.is_empty(),
        "an ordinary policy view must not reveal exact hidden library matches before Transmute activates and resolves"
    );
    assert!(
        view.pending_decision.is_none(),
        "no library-search decision exists before a Transmute ability has resolved"
    );
    game.validate_invariants()
        .expect("the visibility regression starts from a valid live state");
}
