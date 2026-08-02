//! Red regression: a private targeted-discard decision must suspend a
//! multi-instruction spell at the matching effect, rather than letting the
//! direct effect resolver pick a hidden hand card in object-id order.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, CastRequest, Color, DecisionKind, Effect, Game, ManaCost, PlayerId,
    Target, Zone,
};

const SPELL: &str = "TST-MULTI-INSTRUCTION-TARGETED-DISCARD";
const FIRST_HAND_CARD: &str = "TST-MULTI-INSTRUCTION-FIRST-HAND";
const SECOND_HAND_CARD: &str = "TST-MULTI-INSTRUCTION-SECOND-HAND";

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
        supported_rules: &["multi-instruction-targeted-discard-red"],
        power: None,
        toughness: None,
        keywords: vec![],
        effects,
    }
}

#[test]
fn targeted_player_chooses_hidden_discard_during_a_middle_stack_instruction() {
    let caster = PlayerId(0);
    let recipient = PlayerId(1);
    let mut game = Game::new(
        [
            definition(
                SPELL,
                vec![
                    Effect::GainLifeController { amount: 1 },
                    Effect::DiscardTargetPlayer { count: 1 },
                    Effect::GainLifeController { amount: 2 },
                ],
            ),
            definition(FIRST_HAND_CARD, vec![]),
            definition(SECOND_HAND_CARD, vec![]),
        ],
        2,
    )
    .expect("fixture initializes");
    let spell = game
        .add_card(caster, SPELL, Zone::Hand)
        .expect("spell begins in the caster hand");
    let first = game
        .add_card(recipient, FIRST_HAND_CARD, Zone::Hand)
        .expect("first private card begins in the recipient hand");
    let second = game
        .add_card(recipient, SECOND_HAND_CARD, Zone::Hand)
        .expect("second private card begins in the recipient hand");
    game.begin_game().expect("game starts at upkeep");
    game.cast_spell(
        caster,
        CastRequest {
            card: spell,
            targets: vec![Target::Player(recipient)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("zero-cost targeted spell casts");
    game.pass_priority(caster).expect("caster passes");
    game.pass_priority(recipient)
        .expect("recipient reaches the middle discard instruction");

    let recipient_view = game
        .view_for_player(recipient)
        .expect("recipient receives a private view");
    eprintln!(
        "multi-instruction targeted-discard red trace: stack={:?}; pending={:?}; recipient_hand={:?}; events={:?}",
        game.stack,
        recipient_view.pending_decision,
        game.player(recipient).expect("recipient exists").hand,
        game.canonical_event_log(),
    );
    let decision = recipient_view.pending_decision.expect(
        "a middle targeted-discard instruction must suspend for the target player's private card choice",
    );
    assert_eq!(decision.kind, DecisionKind::ConditionalPrivateDiscard);
    assert_eq!(decision.candidates.len(), 2);
    assert!(decision.candidates.iter().any(|card| card.id == first));
    assert!(decision.candidates.iter().any(|card| card.id == second));
    assert_eq!(game.stack.len(), 1, "the spell remains live during the choice");
    assert_eq!(game.player(caster).expect("caster exists").life, 21);
    assert_eq!(game.player(recipient).expect("recipient exists").hand.len(), 2);
    game.validate_invariants()
        .expect("the private decision boundary remains state-machine valid");
}
