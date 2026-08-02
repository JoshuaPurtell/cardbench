//! Red regression for receipt ordering in a revealed multi-card library search.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, CastRequest, DecisionSelection, Effect, Game,
    LibrarySearchCardinality, LibrarySearchDestination, LibrarySearchRequirement,
    LibrarySearchSelection, ManaCost, PlayerId, Zone,
};

const SEARCH: &str = "TST-REVEALED-MULTI-SEARCH";
const FIRST: &str = "TST-REVEALED-MULTI-FIRST";
const SECOND: &str = "TST-REVEALED-MULTI-SECOND";

fn definition(id: &'static str, card_type: CardType, effects: Vec<Effect>) -> CardDefinition {
    let creature = card_type == CardType::Creature;
    CardDefinition {
        id,
        name: id,
        set_code: "TST",
        mana_cost: ManaCost::new(0),
        colors: BTreeSet::new(),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([card_type]),
        is_basic_land: false,
        supported_rules: &["revealed-multi-library-search-contract"],
        power: creature.then_some(1),
        toughness: creature.then_some(1),
        keywords: vec![],
        effects,
    }
}

#[test]
fn revealed_multi_search_keeps_each_reveal_with_its_following_zone_move() {
    let mut game = Game::new(
        vec![
            definition(
                SEARCH,
                CardType::Sorcery,
                vec![Effect::SearchControllerLibraryMany {
                    requirement: LibrarySearchRequirement::CardTypes(BTreeSet::from([
                        CardType::Creature,
                    ])),
                    destination: LibrarySearchDestination::Hand,
                    cardinality: LibrarySearchCardinality::ZeroOrMore { maximum: 2 },
                    selection: LibrarySearchSelection::PolicySubmitted {
                        may_fail_to_find: false,
                    },
                    reveal_selected: true,
                }],
            ),
            definition(FIRST, CardType::Creature, vec![]),
            definition(SECOND, CardType::Creature, vec![]),
        ],
        2,
    )
    .expect("synthetic fixture builds");
    let search = game
        .add_card(PlayerId(0), SEARCH, Zone::Hand)
        .expect("search spell setup");
    let first = game
        .add_card(PlayerId(0), FIRST, Zone::Library)
        .expect("first selected card setup");
    let second = game
        .add_card(PlayerId(0), SECOND, Zone::Library)
        .expect("second selected card setup");
    game.cast_spell(
        PlayerId(0),
        CastRequest {
            card: search,
            targets: vec![],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("search casts");
    game.pass_priority(PlayerId(0))
        .expect("controller passes search");
    game.pass_priority(PlayerId(1))
        .expect("search opens a decision");
    let decision = game
        .view_for_player(PlayerId(0))
        .expect("controller view")
        .pending_decision
        .expect("private search decision opens");
    game.submit_decision(
        PlayerId(0),
        decision.id,
        DecisionSelection::Objects(vec![first, second]),
    )
    .expect("revealed two-card search resolves");
    assert_eq!(game.zone_of(first), Some(Zone::Hand));
    assert_eq!(game.zone_of(second), Some(Zone::Hand));
    game.validate_invariants()
        .expect("revealed multi-search receipt order is valid");
}
