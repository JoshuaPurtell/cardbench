//! Red regression: a spell resolving a draw can eliminate its controller.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, CastRequest, Color, Effect, Game, ManaCost, ManaPaymentSelection,
    PlayerId, Zone,
};

const EMPTY_LIBRARY_DRAW: &str = "STACK-DRAW-LOSS-RESOLUTION";

fn draw_instant() -> CardDefinition {
    CardDefinition {
        id: EMPTY_LIBRARY_DRAW,
        name: EMPTY_LIBRARY_DRAW,
        set_code: "TST",
        mana_cost: ManaCost::with_colors(0, [Color::Blue]),
        colors: BTreeSet::from([Color::Blue]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Instant]),
        is_basic_land: false,
        supported_rules: &["stack-draw-loss-resolution-probe"],
        power: None,
        toughness: None,
        keywords: vec![],
        effects: vec![Effect::DrawControllerIfManaColorSpent { color: Color::Blue }],
    }
}

#[test]
fn resolution_completes_when_its_draw_eliminates_its_controller() {
    let caster = PlayerId(0);
    let responder = PlayerId(1);
    let mut game = Game::new([draw_instant()], 2).expect("fixture game initializes");
    let spell = game
        .add_card(caster, EMPTY_LIBRARY_DRAW, Zone::Hand)
        .expect("spell enters the caster's hand");
    game.grant_mana(caster, Color::Blue, 1)
        .expect("fixture mana is available");
    game.cast_spell_with_mana_spend(
        caster,
        CastRequest {
            card: spell,
            targets: vec![],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
        ManaPaymentSelection::default(),
    )
    .expect("draw spell is cast with its explicit blue payment receipt");
    game.pass_priority(caster)
        .expect("caster opens the response window");

    let result = game.pass_priority(responder);
    eprintln!(
        "draw-loss resolution result: {result:?}; stack: {:?}; events: {:?}",
        game.stack,
        game.canonical_event_log(),
    );

    assert!(
        result.is_ok(),
        "player-departure cleanup during an effect must not roll back the completed stack resolution"
    );
    assert!(
        game.stack.is_empty(),
        "the resolving spell leaves the stack"
    );
    assert!(
        game.players[caster.0].lost,
        "the empty-library draw eliminates its controller"
    );
    assert!(
        game.canonical_event_log()
            .iter()
            .any(|event| event.starts_with("GameEnded")),
        "the terminal loss lifecycle is recorded"
    );
    assert_eq!(
        game.canonical_event_log(),
        [
            format!("SpellManaPaid {{ player: {caster:?}, card: {spell:?}, colors: [Blue] }}"),
            format!("SpellCast {{ player: {caster:?}, card: {spell:?} }}"),
            format!("ObjectIncarnationAdvanced {{ object: {spell:?}, incarnation: 2 }}"),
            format!("PriorityPassed {{ player: {caster:?} }}"),
            format!("PriorityPassed {{ player: {responder:?} }}"),
            format!(
                "PlayerLost {{ player: {caster:?}, reason: \"attempted to draw from an empty library\" }}"
            ),
            format!("ObjectLeftGame {{ object: {spell:?}, owner: {caster:?} }}"),
            format!("GameEnded {{ winner: Some({responder:?}) }}"),
        ],
        "source departure closes the cast lifecycle; no later SpellResolved or zone move names a removed object"
    );
    game.validate_invariants()
        .expect("the source-departure terminal lifecycle is auditable");
}
