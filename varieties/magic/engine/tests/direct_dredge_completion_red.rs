//! Red regression: a public successful Dredge must consume its draw marker.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, Game, Keyword, ManaCost, PlayerId, Step, Zone,
};

const DREDGER: &str = "TST-DIRECT-DREDGER";
const FILLER: &str = "TST-DIRECT-DREDGE-FILLER";

fn definitions() -> [CardDefinition; 2] {
    [
        CardDefinition {
            id: DREDGER,
            name: DREDGER,
            set_code: "TST",
            mana_cost: ManaCost::new(0),
            colors: BTreeSet::new(),
            mana_colors: BTreeSet::new(),
            card_types: BTreeSet::from([CardType::Creature]),
            is_basic_land: false,
            supported_rules: &["test-only-dredge"],
            power: Some(1),
            toughness: Some(1),
            keywords: vec![Keyword::Dredge(2)],
            effects: vec![],
        },
        CardDefinition {
            id: FILLER,
            name: FILLER,
            set_code: "TST",
            mana_cost: ManaCost::new(0),
            colors: BTreeSet::new(),
            mana_colors: BTreeSet::new(),
            card_types: BTreeSet::from([CardType::Artifact]),
            is_basic_land: false,
            supported_rules: &["test-only-library-margin"],
            power: None,
            toughness: None,
            keywords: vec![],
            effects: vec![],
        },
    ]
}

fn advance_to_second_players_draw(game: &mut Game) {
    for _ in 0..64 {
        if game.turn == 2 && game.active_player == PlayerId(1) && game.step == Step::Draw {
            return;
        }
        if game.step == Step::DeclareAttackers
            && !game
                .view_for_player(game.next_policy_player())
                .expect("combat view")
                .attackers_declared
        {
            game.declare_attackers(game.next_policy_player(), &[])
                .expect("empty attack declaration advances the fixture");
            continue;
        }
        if game.step == Step::DeclareBlockers
            && !game
                .view_for_player(game.next_policy_player())
                .expect("combat view")
                .blockers_declared
        {
            game.declare_blockers(game.next_policy_player(), &[])
                .expect("empty block declaration advances the fixture");
            continue;
        }
        game.pass_priority(game.priority)
            .expect("fixture priority pass advances the turn state");
    }
    panic!(
        "fixture did not reach player one's draw; reached turn {} {:?} for {:?}",
        game.turn, game.step, game.active_player
    );
}

#[test]
fn direct_dredge_consumes_its_pending_draw_before_the_next_action() {
    let player = PlayerId(1);
    let mut game = Game::new(definitions(), 2).expect("fixture initializes");
    let dredger = game
        .add_card(player, DREDGER, Zone::Graveyard)
        .expect("Dredge source enters the graveyard");
    for _ in 0..3 {
        game.add_card(player, FILLER, Zone::Library)
            .expect("library margin exists");
    }
    game.begin_game().expect("fixture begins");
    advance_to_second_players_draw(&mut game);

    game.dredge(player, dredger)
        .expect("the public Dredge replacement succeeds");
    game.validate_invariants()
        .expect("a successful public transition must remain audit-valid");
    let view = game.view_for_player(player).expect("draw view remains available");
    assert!(
        !view.draw_replacement_pending,
        "Dredge consumed the card movement but left a second draw available: hand={:?}; library={:?}; events={:?}",
        game.player(player).expect("player exists").hand,
        game.player(player).expect("player exists").library,
        game.canonical_event_log(),
    );
}
