//! Red regression for one cast action that needs both a mode and a color.
//!
//! `Game::cast_spell_impl` already carries independent `chosen_modal_mode` and
//! `chosen_color` slots.  The public action surface must expose the same
//! atomic choice boundary: Magic does not permit casting a modal spell, then
//! later supplying a color after the card has left the hand.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, CastRequest, Color, Effect, Game, GameEvent, ManaCost, PlayerId,
    PolicyAction, PolicyMoveKind, Target, Zone,
};

const CREATURE: &str = "TST-MODAL-COLOR-CREATURE";
const SPELL: &str = "TST-MODAL-COLOR-SPELL";

fn definition(
    id: &'static str,
    card_type: CardType,
    effects: Vec<Effect>,
    power: Option<i16>,
    toughness: Option<i16>,
) -> CardDefinition {
    CardDefinition {
        id,
        name: id,
        set_code: "TST",
        mana_cost: ManaCost::new(0),
        colors: BTreeSet::from([Color::Blue]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([card_type]),
        is_basic_land: false,
        supported_rules: &["modal-color-choice-composition-red"],
        power,
        toughness,
        keywords: vec![],
        effects,
    }
}

#[test]
#[allow(clippy::too_many_lines)] // The public trace covers both decision receipts and resolution.
fn selected_modal_branch_can_submit_its_required_color_in_the_same_cast() {
    let mut game = Game::new(
        [
            definition(CREATURE, CardType::Creature, vec![], Some(2), Some(2)),
            definition(
                SPELL,
                CardType::Instant,
                vec![Effect::ChooseOneOf(vec![
                    vec![Effect::ReplaceTargetCreatureColorsWithChosenColorUntilEndOfTurn],
                    vec![Effect::GainLifeController { amount: 1 }],
                ])],
                None,
                None,
            ),
        ],
        2,
    )
    .expect("fixture initializes");
    let creature = game
        .put_on_battlefield(PlayerId(1), CREATURE)
        .expect("creature setup");
    let spell = game
        .add_card(PlayerId(0), SPELL, Zone::Hand)
        .expect("spell setup");
    game.begin_game().expect("game begins");

    // A modal action on its own still must not invent a color choice.
    let missing_color = game.cast_spell_with_mode(
        PlayerId(0),
        CastRequest {
            card: spell,
            targets: vec![Target::Permanent(creature)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
        0,
    );
    assert!(
        missing_color.is_err(),
        "modal casts cannot default a card color"
    );
    assert_eq!(game.zone_of(spell), Some(Zone::Hand));
    assert!(game.stack.is_empty());

    game.submit_policy_move(
        PlayerId(0),
        "modal-color-choice-policy",
        PolicyAction::CastWithModeAndColorChoice {
            request: CastRequest {
                card: spell,
                targets: vec![Target::Permanent(creature)],
                convoke: vec![],
                payment_mana_abilities: vec![],
            },
            mode: 0,
            color: Color::Red,
        },
    )
    .expect("policy submits both choices in one legal cast");
    eprintln!(
        "modal color choice zone={:?}; stack={:?}; events={:?}",
        game.zone_of(spell),
        game.stack,
        game.canonical_event_log()
    );
    assert!(
        matches!(
            game.stack.as_slice(),
            [stack]
                if stack.chosen_modal_mode == Some(0)
                    && stack.chosen_color == Some(Color::Red)
        ),
        "the one stack item retains both cast decisions"
    );
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::PolicyMoveSubmitted {
            kind: PolicyMoveKind::CastWithMode,
            ..
        }
    )));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::SpellModeChosen { player, card, mode }
            if *player == PlayerId(0) && *card == spell && *mode == 0
    )));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::SpellColorChosen { player, card, color }
            if *player == PlayerId(0) && *card == spell && *color == Color::Red
    )));
    game.validate_invariants()
        .expect("the live modal stack item audits its materialized color requirement");

    let first = game.priority;
    game.pass_priority(first).expect("first resolution pass");
    let second = game.priority;
    game.pass_priority(second).expect("second resolution pass");
    assert_eq!(
        game.characteristics(creature)
            .expect("creature remains")
            .colors,
        BTreeSet::from([Color::Red]),
        "the selected branch resolves with the selected color"
    );
    game.validate_invariants()
        .expect("combined cast choices retain auditable provenance");
}
