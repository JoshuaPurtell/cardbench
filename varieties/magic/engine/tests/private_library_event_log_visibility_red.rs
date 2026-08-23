//! Red regression: a private library inspection must not serialize the hidden
//! card identities into the public canonical event log.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, CastRequest, Color, Effect, Game, ManaCost, PlayerId, Zone,
};

const SPELL: &str = "TST-PRIVATE-LOOK";
const HIDDEN: &str = "TST-HIDDEN-LIBRARY-CARD";

fn definition(id: &'static str, effects: Vec<Effect>) -> CardDefinition {
    CardDefinition {
        id,
        name: id,
        set_code: "TST",
        mana_cost: ManaCost::new(0),
        colors: BTreeSet::from([Color::Black]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Instant]),
        is_basic_land: false,
        supported_rules: &["private-library-event-log-visibility-red"],
        power: None,
        toughness: None,
        keywords: vec![],
        effects,
    }
}

#[test]
fn private_library_look_does_not_publish_hidden_card_ids_in_canonical_log() {
    let controller = PlayerId(0);
    let opponent = PlayerId(1);
    let mut game = Game::new(
        [
            definition(
                SPELL,
                vec![Effect::LookAtTopCardsChooseForLifeOrGraveyard {
                    count: 1,
                    life_per_card: 1,
                }],
            ),
            definition(HIDDEN, vec![]),
        ],
        2,
    )
    .expect("fixture initializes");
    let spell = game
        .add_card(controller, SPELL, Zone::Hand)
        .expect("spell starts in hand");
    let hidden = game
        .add_card(controller, HIDDEN, Zone::Library)
        .expect("hidden card starts in library");
    game.begin_game().expect("game starts");
    game.cast_spell(
        controller,
        CastRequest {
            card: spell,
            targets: vec![],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("spell casts");
    game.pass_priority(controller).expect("controller passes");
    game.pass_priority(opponent)
        .expect("opponent pass opens the private inspection");

    let opponent_view = game.view_for_player(opponent).expect("opponent view");
    let canonical_log = game.canonical_event_log();
    println!("private inspection canonical log: {canonical_log:#?}");
    assert!(
        opponent_view.private_library_choice.is_none(),
        "the opponent policy view must not receive private candidates"
    );
    assert!(
        !canonical_log
            .iter()
            .any(|event| event.contains(&format!("{hidden:?}"))),
        "the public canonical event log must not reveal a private library card identity"
    );
    game.validate_invariants()
        .expect("the private inspection boundary is otherwise invariant-valid");
}
