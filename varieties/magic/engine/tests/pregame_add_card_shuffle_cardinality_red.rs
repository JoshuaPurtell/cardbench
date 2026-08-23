//! Red regression: every setup path must preserve exact shuffle receipts.
//!
//! `load_deck_into_library` already rejects a deck that cannot fit in a
//! `LibraryShuffled.cards` receipt.  Direct pregame `add_card` is another
//! public setup path, however, and used to admit one more physical card than
//! that receipt can represent.  A later ordinary library search then recorded
//! a saturated count, which made the canonical event log disagree with live
//! state.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, CastRequest, Effect, Game, GameEvent, LibrarySearchDestination,
    LibrarySearchRequirement, LibrarySearchSelection, ManaCost, PlayerId, RulesError, Zone,
};

const SEARCH: &str = "TST-PREGAME-ADD-CARD-SHUFFLE-SEARCH";
const FILLER: &str = "TST-PREGAME-ADD-CARD-SHUFFLE-FILLER";

fn definition(id: &'static str, card_type: CardType, effects: Vec<Effect>) -> CardDefinition {
    CardDefinition {
        id,
        name: id,
        set_code: "TST",
        mana_cost: ManaCost::new(0),
        colors: BTreeSet::new(),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([card_type]),
        is_basic_land: false,
        supported_rules: &["pregame-add-card-shuffle-cardinality-red"],
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

#[test]
fn pregame_add_card_cannot_fabricate_a_library_that_would_saturate_a_shuffle_receipt() {
    let player = PlayerId(0);
    let mut game = Game::new(
        [
            definition(
                SEARCH,
                CardType::Instant,
                vec![Effect::SearchControllerLibrary {
                    requirement: LibrarySearchRequirement::CardTypes(BTreeSet::from([
                        CardType::Creature,
                    ])),
                    destination: LibrarySearchDestination::Hand,
                    selection: LibrarySearchSelection::DeterministicFirstMatch,
                    reveal_selected: false,
                }],
            ),
            definition(FILLER, CardType::Artifact, vec![]),
        ],
        2,
    )
    .expect("fixture initializes");
    let search = game
        .add_card(player, SEARCH, Zone::Hand)
        .expect("search spell begins in hand");

    let insertion =
        (0..=u16::MAX).try_for_each(|_| game.add_card(player, FILLER, Zone::Library).map(|_| ()));

    if let Err(error) = insertion {
        eprintln!(
            "pregame capacity rejection: {error:?}; library_len={}; events={:?}",
            game.player(player).expect("player exists").library.len(),
            game.canonical_event_log(),
        );
        assert!(matches!(
            error,
            RulesError::IllegalAction("setup card count exceeds event receipt range")
        ));
        assert_eq!(
            game.player(player).expect("player exists").library.len(),
            usize::from(u16::MAX),
            "the rejected card must not enter the setup fixture"
        );
        assert!(
            game.canonical_event_log().is_empty(),
            "a rejected pregame insertion must not create a fake receipt"
        );
        game.validate_invariants()
            .expect("the capacity boundary leaves a valid fixture");
        return;
    }

    game.begin_game().expect("game begins");
    game.cast_spell(
        player,
        CastRequest {
            card: search,
            targets: vec![],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("zero-cost search spell casts");
    pass_pair(&mut game);

    let cards = game
        .event_log
        .iter()
        .rev()
        .find_map(|event| match event {
            GameEvent::LibraryShuffled {
                player: shuffled,
                cards,
            } if *shuffled == player => Some(*cards),
            _ => None,
        })
        .expect("resolving a library search records its shuffle");
    let library_len = game.player(player).expect("player exists").library.len();
    eprintln!(
        "saturated pregame-add-card shuffle: library_len={library_len}; receipt_cards={cards}; events={:?}",
        game.canonical_event_log(),
    );
    assert_eq!(
        usize::from(cards),
        library_len,
        "a canonical LibraryShuffled receipt must state the exact live library size"
    );
}
