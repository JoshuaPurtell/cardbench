//! Contract for a counter/mill effect whose follow-up reads cast-payment
//! provenance after moving the targeted physical spell off the stack.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, CastRequest, Color, Effect, Game, GameEvent, ManaCost,
    ManaPaymentSelection, PlayerId, Target, Zone,
};

const TARGET_SPELL: &str = "TST-COUNTER-MILL-TARGET";
const COUNTER_MILL: &str = "TST-COUNTER-MILL";
const FILLER: &str = "TST-COUNTER-MILL-FILLER";

fn definition(
    id: &'static str,
    card_type: CardType,
    mana_cost: ManaCost,
    effects: Vec<Effect>,
) -> CardDefinition {
    let creature = card_type == CardType::Creature;
    CardDefinition {
        id,
        name: id,
        set_code: "TST",
        mana_cost,
        colors: BTreeSet::new(),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([card_type]),
        is_basic_land: false,
        supported_rules: &["counter-mill-spent-mana-contract"],
        power: creature.then_some(1),
        toughness: creature.then_some(1),
        keywords: vec![],
        effects,
    }
}

fn fixture(
    payment_color: Color,
) -> (
    Game,
    cardbench_magic_engine::ObjectId,
    cardbench_magic_engine::ObjectId,
    [cardbench_magic_engine::ObjectId; 2],
) {
    let caster = PlayerId(0);
    let responder = PlayerId(1);
    let mut game = Game::new(
        [
            definition(TARGET_SPELL, CardType::Creature, ManaCost::new(2), vec![]),
            definition(
                COUNTER_MILL,
                CardType::Instant,
                ManaCost::new(1),
                vec![
                    Effect::CounterTargetPhysicalSpellThenMillItsControllerByManaValueIfManaColorSpent {
                        color: Color::Blue,
                    },
                ],
            ),
            definition(FILLER, CardType::Instant, ManaCost::new(0), vec![]),
        ],
        2,
    )
    .expect("fixture game initializes");
    let target = game
        .add_card(caster, TARGET_SPELL, Zone::Hand)
        .expect("target spell enters hand");
    let counter = game
        .add_card(responder, COUNTER_MILL, Zone::Hand)
        .expect("counter spell enters hand");
    let bottom = game
        .add_card(caster, FILLER, Zone::Library)
        .expect("bottom library card");
    let top = game
        .add_card(caster, FILLER, Zone::Library)
        .expect("top library card");
    game.grant_mana(caster, Color::Colorless, 2)
        .expect("target spell fixture mana is available");
    game.grant_mana(responder, payment_color, 1)
        .expect("fixture mana is available");

    game.cast_spell(
        caster,
        CastRequest {
            card: target,
            targets: vec![],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("target creature spell casts");
    game.pass_priority(caster)
        .expect("caster yields for response");
    game.cast_spell_with_mana_spend(
        responder,
        CastRequest {
            card: counter,
            targets: vec![Target::Spell(target)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
        ManaPaymentSelection {
            generic: vec![payment_color],
            hybrid: vec![],
        },
    )
    .expect("counter/mill spell casts with an explicit payment receipt");
    game.pass_priority(responder)
        .expect("counter controller passes");
    game.pass_priority(caster)
        .expect("counter/mill spell resolves");
    (game, target, counter, [top, bottom])
}

#[test]
fn blue_payment_counters_then_mills_the_captured_target_controller_by_mana_value() {
    let (game, target, counter, [top, bottom]) = fixture(Color::Blue);

    eprintln!(
        "blue_counter_mill_event_log={:#?}",
        game.canonical_event_log()
    );
    assert_eq!(game.zone_of(target), Some(Zone::Graveyard));
    assert_eq!(game.zone_of(top), Some(Zone::Graveyard));
    assert_eq!(game.zone_of(bottom), Some(Zone::Graveyard));
    let counter_index = game
        .event_log
        .iter()
        .position(|event| {
            matches!(
                event,
                GameEvent::SpellCountered { card, source } if *card == target && *source == counter
            )
        })
        .expect("counter receipt is present");
    let target_terminal_move = game
        .event_log
        .iter()
        .position(|event| {
            matches!(
                event,
                GameEvent::CardMoved { card, to: Zone::Graveyard } if *card == target
            )
        })
        .expect("countered target terminal move is present");
    let first_mill = game
        .event_log
        .iter()
        .position(|event| {
            matches!(
                event,
                GameEvent::CardMoved { card, to: Zone::Graveyard } if *card == top
            )
        })
        .expect("first mill move is present");
    assert!(
        counter_index < target_terminal_move && target_terminal_move < first_mill,
        "counter, target terminal move, then captured-controller mill must remain ordered"
    );
    game.validate_invariants()
        .expect("blue spent-mana counter/mill trace preserves invariants");
}

#[test]
fn nonblue_explicit_payment_still_counters_but_does_not_mill() {
    let (game, target, counter, [top, bottom]) = fixture(Color::Red);

    eprintln!(
        "nonblue_counter_mill_event_log={:#?}",
        game.canonical_event_log()
    );
    assert_eq!(game.zone_of(target), Some(Zone::Graveyard));
    assert_eq!(game.zone_of(top), Some(Zone::Library));
    assert_eq!(game.zone_of(bottom), Some(Zone::Library));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::SpellCountered { card, source } if *card == target && *source == counter
    )));
    game.validate_invariants()
        .expect("nonblue gate preserves counter and zone invariants");
}
