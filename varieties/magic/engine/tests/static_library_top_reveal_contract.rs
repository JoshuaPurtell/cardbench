//! Expansion-neutral contract for live static top-library visibility.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, Color, Game, ManaCost, PlayerId, StaticLibraryTopRevealBinding,
    StaticLibraryTopRevealScope, Zone,
};

const REVEALER: &str = "TST-LIBRARY-REVEALER";
const FIRST: &str = "TST-LIBRARY-FIRST";
const SECOND: &str = "TST-LIBRARY-SECOND";

fn definition(id: &'static str, card_type: CardType) -> CardDefinition {
    let is_creature = card_type == CardType::Creature;
    CardDefinition {
        id,
        name: id,
        set_code: "TST",
        mana_cost: ManaCost::new(0),
        colors: BTreeSet::from([Color::Blue]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([card_type]),
        is_basic_land: false,
        supported_rules: &["test-static-library-reveal"],
        power: is_creature.then_some(1),
        toughness: is_creature.then_some(1),
        keywords: vec![],
        effects: vec![],
    }
}

#[test]
fn static_library_reveal_projects_only_live_tops_to_every_view() {
    let mut game = Game::new(
        vec![
            definition(REVEALER, CardType::Creature),
            definition(FIRST, CardType::Instant),
            definition(SECOND, CardType::Sorcery),
        ],
        2,
    )
    .expect("synthetic fixture builds");

    assert!(
        game.register_static_library_top_reveal_bindings([StaticLibraryTopRevealBinding {
            card_definition: FIRST,
            scope: StaticLibraryTopRevealScope::EveryPlayer,
        }])
        .is_err()
    );
    game.register_static_library_top_reveal_bindings([StaticLibraryTopRevealBinding {
        card_definition: REVEALER,
        scope: StaticLibraryTopRevealScope::EveryPlayer,
    }])
    .expect("permanent source binding registers");
    assert!(
        game.register_static_library_top_reveal_bindings([StaticLibraryTopRevealBinding {
            card_definition: REVEALER,
            scope: StaticLibraryTopRevealScope::EveryPlayer,
        }])
        .is_err()
    );

    let first = game
        .add_card(PlayerId(0), FIRST, Zone::Library)
        .expect("first library card exists");
    let second = game
        .add_card(PlayerId(1), SECOND, Zone::Library)
        .expect("second library card exists");
    assert!(
        game.view_for_player(PlayerId(0))
            .expect("view exists")
            .revealed_library_tops
            .is_empty()
    );

    game.put_on_battlefield(PlayerId(1), REVEALER)
        .expect("reveal source enters");
    for viewer in [PlayerId(0), PlayerId(1)] {
        let actual = game
            .view_for_player(viewer)
            .expect("public view exists")
            .revealed_library_tops
            .into_iter()
            .map(|top| (top.owner, top.card.id))
            .collect::<Vec<_>>();
        assert_eq!(actual, vec![(PlayerId(0), first), (PlayerId(1), second)]);
    }

    game.begin_game().expect("game begins");
    assert!(
        game.register_static_library_top_reveal_bindings([])
            .is_err()
    );
    game.validate_invariants()
        .expect("static top-library reveal stays invariant-valid");
}
