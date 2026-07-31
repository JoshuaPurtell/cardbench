//! Full-fidelity contracts for Ribbons of Night's paid-color condition.

use cardbench_magic_engine::{
    CastRequest, Color, Effect, Game, GameEvent, ManaCost, ManaPaymentSelection, PlayerId,
    RulesError, Target, TargetRequirement, Zone,
};
use cardbench_magic_rav::{RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions};

fn setup() -> (
    Game,
    PlayerId,
    PlayerId,
    cardbench_magic_engine::ObjectId,
    cardbench_magic_engine::ObjectId,
    cardbench_magic_engine::ObjectId,
) {
    let caster = PlayerId(0);
    let opponent = PlayerId(1);
    let mut game = Game::new(card_definitions(), 2).expect("RAV catalog constructs");
    let ribbons = game
        .add_card(caster, "RAV-RIBBONS-OF-NIGHT", Zone::Hand)
        .expect("Ribbons begins in hand");
    let victim = game
        .put_on_battlefield(opponent, "RAV-GOLGARI-BROWNSCALE")
        .expect("target creature begins on the battlefield");
    let drawn = game
        .add_card(caster, "RAV-WATCHWOLF", Zone::Library)
        .expect("one public library card is drawable");
    for (color, amount) in [(Color::Black, 1), (Color::Blue, 4), (Color::Red, 4)] {
        game.grant_mana(caster, color, amount)
            .expect("fixture mana fits the pool");
    }
    game.clear_event_log();
    (game, caster, opponent, ribbons, victim, drawn)
}

fn cast_and_resolve(
    game: &mut Game,
    caster: PlayerId,
    opponent: PlayerId,
    ribbons: cardbench_magic_engine::ObjectId,
    victim: cardbench_magic_engine::ObjectId,
    generic: Color,
) {
    game.cast_spell_with_mana_spend(
        caster,
        CastRequest {
            card: ribbons,
            targets: vec![Target::Permanent(victim)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
        ManaPaymentSelection {
            generic: vec![generic; 4],
            hybrid: vec![],
        },
    )
    .expect("the selected generic allocation pays the exact cost");
    game.pass_priority(caster).expect("caster passes");
    game.pass_priority(opponent)
        .expect("opponent passes and resolves");
}

#[test]
fn ribbons_records_blue_payment_on_its_stack_object_and_draws_exactly_once() {
    let (mut game, caster, opponent, ribbons, victim, drawn) = setup();
    cast_and_resolve(&mut game, caster, opponent, ribbons, victim, Color::Blue);

    println!("Ribbons blue-payment trace: {:?}", game.event_log);
    assert_eq!(game.zone_of(drawn), Some(Zone::Hand));
    assert_eq!(game.player(caster).expect("caster exists").life, 24);
    assert_eq!(
        &game.event_log[..2],
        vec![
            GameEvent::SpellManaPaid {
                player: caster,
                card: ribbons,
                colors: vec![
                    Color::Black,
                    Color::Blue,
                    Color::Blue,
                    Color::Blue,
                    Color::Blue,
                ],
            },
            GameEvent::SpellCast {
                player: caster,
                card: ribbons,
            },
        ],
        "the replayable spend receipt precedes the stack lifecycle"
    );
    assert!(game.event_log.iter().any(|event| {
        matches!(event, GameEvent::CardMoved { card, to: Zone::Hand } if *card == drawn)
    }));
    game.validate_invariants()
        .expect("blue payment trace remains stack-invariant valid");
}

#[test]
fn ribbons_records_nonblue_payment_and_does_not_draw() {
    let (mut game, caster, opponent, ribbons, victim, drawn) = setup();
    cast_and_resolve(&mut game, caster, opponent, ribbons, victim, Color::Red);

    println!("Ribbons red-payment trace: {:?}", game.event_log);
    assert_eq!(game.zone_of(drawn), Some(Zone::Library));
    assert!(game.event_log.iter().all(|event| {
        !matches!(event, GameEvent::CardMoved { card, to: Zone::Hand } if *card == drawn)
    }));
    game.validate_invariants()
        .expect("nonblue payment trace remains stack-invariant valid");
}

#[test]
fn malformed_explicit_payment_is_atomic() {
    let (mut game, caster, _opponent, ribbons, victim, _drawn) = setup();
    let before_pool = game
        .player(caster)
        .expect("caster exists")
        .mana_pool
        .clone();
    let result = game.cast_spell_with_mana_spend(
        caster,
        CastRequest {
            card: ribbons,
            targets: vec![Target::Permanent(victim)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
        ManaPaymentSelection {
            generic: vec![Color::Red; 3],
            hybrid: vec![],
        },
    );
    println!("Ribbons malformed-payment result: {result:?}");
    assert!(matches!(result, Err(RulesError::Mana(_))));
    assert_eq!(game.zone_of(ribbons), Some(Zone::Hand));
    assert!(game.stack.is_empty());
    assert_eq!(
        game.player(caster).expect("caster exists").mana_pool,
        before_pool
    );
    assert!(game.event_log.is_empty());
    game.validate_invariants()
        .expect("malformed allocation rollback preserves invariants");
}

#[test]
fn ribbons_is_positive_manifest_only_with_the_paid_color_contract() {
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&"RAV-RIBBONS-OF-NIGHT"));
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-RIBBONS-OF-NIGHT")
        .expect("Ribbons definition exists");
    assert_eq!(
        definition.mana_cost,
        ManaCost::with_colors(4, [Color::Black])
    );
    assert_eq!(
        definition.supported_rules,
        [
            "full-rules-fidelity",
            "targeted-creature-damage",
            "life-gain",
            "explicit-spent-mana-color-condition",
        ]
    );
    assert_eq!(
        definition.effects,
        [
            Effect::DealDamage {
                amount: 4,
                target: TargetRequirement::Creature,
            },
            Effect::GainLifeController { amount: 4 },
            Effect::DrawControllerIfManaColorSpent { color: Color::Blue },
        ]
    );
}
