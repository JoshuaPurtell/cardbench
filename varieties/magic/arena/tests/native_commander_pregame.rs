//! Integration checks over executable RAV cards, not benchmark grading worlds.
//! This deliberately tiny metadata slice is test input, not an Oracle snapshot.
use std::collections::{BTreeMap, BTreeSet};
use cardbench_magic_engine::{
    Color, CommanderCardRules, CommanderCatalog, CommanderDeck, DeckEntry, DeckList,
    LondonPregame, MulliganChoice, PlayerId, PregamePrompt,
};
use cardbench_magic_policies::planner::{Board, CardIndex};
use cardbench_magic_rav::{card_definitions, new_rav_game};
mod common;

fn setup() -> (cardbench_magic_engine::Game, CommanderCatalog) {
    let rules = |name: &str, identity: &[Color], commander: bool| CommanderCardRules {
        name: name.into(), color_identity: identity.iter().copied().collect(),
        basic_land_colors: if commander { BTreeSet::new() } else { identity.iter().copied().collect() },
        maximum_copies: if commander { Some(1) } else { None },
        legal: true, implemented: true, can_be_commander: commander,
        co_commanders: BTreeSet::new(),
    };
    let metadata = CommanderCatalog {
        revision: "integration-test-only-rav-three-card-slice-v1".into(),
        cards: BTreeMap::from([
            ("RAV-TOLSIMIR-WOLFBLOOD".into(), rules("Tolsimir Wolfblood", &[Color::Green, Color::White], true)),
            ("RAV-FOREST".into(), rules("Forest", &[Color::Green], false)),
            ("RAV-PLAINS".into(), rules("Plains", &[Color::White], false)),
        ]),
    };
    let deck = CommanderDeck {
        commanders: vec!["RAV-TOLSIMIR-WOLFBLOOD".into()],
        deck: DeckList { mainboard: vec![
            DeckEntry { card: "RAV-TOLSIMIR-WOLFBLOOD".into(), count: 1 },
            DeckEntry { card: "RAV-FOREST".into(), count: 50 },
            DeckEntry { card: "RAV-PLAINS".into(), count: 49 },
        ], sideboard: vec![] },
    };
    let mut game = new_rav_game(4).unwrap();
    game.set_shuffle_seed(7321).unwrap();
    for seat in 0..4 { game.load_commander_deck(PlayerId(seat), &deck, &metadata).unwrap(); }
    (game, metadata)
}

#[test]
fn four_real_full_decks_start_through_strict_london_admission() {
    let (game, metadata) = setup();
    let mut pregame = LondonPregame::for_commander(game, PlayerId(2), &metadata).unwrap();
    for seat in [2, 3, 0, 1] {
        assert_eq!(pregame.prompt(), PregamePrompt::Choose { player: PlayerId(seat), round: 0, may_mulligan: true });
        pregame.choose(PlayerId(seat), 0, MulliganChoice::Keep).unwrap();
    }
    let game = pregame.finish().unwrap();
    for seat in 0..4 {
        let view = game.view_for_player(PlayerId(seat)).unwrap();
        assert_eq!(view.own_life, 40);
        assert_eq!(view.hand.len(), 7);
        assert_eq!(view.command_zone.len(), 4);
        assert_eq!(game.command_zone(PlayerId(seat)).len(), 1);
    }
    game.validate_invariants().unwrap();
}

#[test]
fn strict_admission_rechecks_metadata_against_executable_cards() {
    let (game, mut metadata) = setup();
    metadata.cards.get_mut("RAV-TOLSIMIR-WOLFBLOOD").unwrap().name = "wrong canonical name".into();
    assert!(LondonPregame::for_commander(game, PlayerId(0), &metadata).is_err());
}

#[test]
fn policy_projection_separates_commanders_and_adds_tax_only_to_cast_cost() {
    let (game, metadata) = setup();
    let pregame = LondonPregame::for_commander(game, PlayerId(0), &metadata).unwrap();
    let mut view = pregame.view_for_player(PlayerId(0)).unwrap();
    let commander = view.command_zone.iter().find(|card| card.controller == PlayerId(0)).unwrap().id;
    // Projection-level input: no battlefield or engine transition is fabricated.
    view.commander_taxes.insert(commander, 4);
    let index = CardIndex::build(&card_definitions(), &[], &[], &[], &[], &[]);
    let board = Board::from_view(&view, &index);
    assert_eq!(board.commanders.len(), 1);
    assert_eq!(board.commanders[0].object, commander);
    assert!(!board.hand.iter().any(|card| card.object == commander));
    assert_eq!(board.commanders[0].facts.cost.generic, 8);
    assert_eq!(board.commanders[0].facts.mana_value, 6);
}

#[test]
fn legal_commander_setup_can_finish_a_native_four_seat_game() {
    let (game, metadata) = setup();
    let mut pregame = LondonPregame::for_commander(game, PlayerId(0), &metadata).unwrap();
    for seat in 0..4 { pregame.choose(PlayerId(seat), 0, MulliganChoice::Keep).unwrap(); }
    // This three-card slice checks plumbing only, not benchmark difficulty or
    // arbitrary-card EDH conformance. No creatures are pre-seated in play.
    common::finish_native_game(pregame.finish().unwrap(), &[cardbench_magic_policies::Archetype::Midrange; 4]);
}
