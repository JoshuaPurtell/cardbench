use cardbench_magic_engine::{
    CastRequest, Color, Effect, Game, GameEvent, ManaCost, ManaPaymentSelection, PlayerId, Target,
    Zone,
};
use cardbench_magic_rav::{RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions};

fn setup() -> (
    Game,
    cardbench_magic_engine::ObjectId,
    cardbench_magic_engine::ObjectId,
    cardbench_magic_engine::ObjectId,
    cardbench_magic_engine::ObjectId,
) {
    let mut game = Game::new(card_definitions(), 2).expect("RAV catalog builds");
    let spell = game
        .add_card(PlayerId(0), "RAV-ROLLING-SPOIL", Zone::Hand)
        .expect("Rolling Spoil setup");
    let land = game
        .put_on_battlefield(PlayerId(1), "RAV-SUNHOME-FORTRESS")
        .expect("target land setup");
    let friendly = game
        .put_on_battlefield(PlayerId(0), "RAV-WATCHWOLF")
        .expect("friendly creature setup");
    let opposing = game
        .put_on_battlefield(PlayerId(1), "RAV-GLASS-GOLEM")
        .expect("opposing creature setup");
    for (color, amount) in [(Color::Green, 2), (Color::Black, 1), (Color::Red, 2)] {
        game.grant_mana(PlayerId(0), color, amount)
            .expect("fixture mana");
    }
    game.clear_event_log();
    (game, spell, land, friendly, opposing)
}

fn cast_and_resolve(game: &mut Game, spell: cardbench_magic_engine::ObjectId, generic: Vec<Color>) {
    game.cast_spell_with_mana_spend(
        PlayerId(0),
        CastRequest {
            card: spell,
            targets: vec![Target::Permanent(
                game.player(PlayerId(1)).expect("target player").battlefield[0],
            )],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
        ManaPaymentSelection {
            generic,
            hybrid: vec![],
        },
    )
    .expect("Rolling Spoil cast");
    game.pass_priority(PlayerId(0)).expect("caster passes");
    game.pass_priority(PlayerId(1)).expect("opponent passes");
}

#[test]
fn rolling_spoil_uses_only_its_explicit_black_payment_receipt_for_the_global_batch() {
    let (mut game, spell, land, friendly, opposing) = setup();
    cast_and_resolve(&mut game, spell, vec![Color::Black, Color::Red]);

    println!("Rolling Spoil black-spent trace: {:?}", game.event_log);
    assert_eq!(game.zone_of(land), Some(Zone::Graveyard));
    assert_eq!(
        (
            game.characteristics(friendly).unwrap().power,
            game.characteristics(friendly).unwrap().toughness
        ),
        (Some(2), Some(2)),
    );
    assert_eq!(
        (
            game.characteristics(opposing).unwrap().power,
            game.characteristics(opposing).unwrap().toughness
        ),
        (Some(5), Some(1)),
    );
    let destroyed = game
        .event_log
        .iter()
        .position(|event| matches!(event, GameEvent::CardDestroyed { card, .. } if *card == land))
        .expect("land destruction receipt");
    let modifier = game
        .event_log
        .iter()
        .position(|event| matches!(event, GameEvent::ContinuousEffectCreated { target, .. } if *target == friendly))
        .expect("global modifier receipt");
    assert!(
        destroyed < modifier,
        "land destruction precedes global snapshot"
    );
    game.validate_invariants()
        .expect("black-spent trace remains valid");
}

#[test]
fn rolling_spoil_skips_the_global_batch_when_black_was_not_spent() {
    let (mut game, spell, land, friendly, opposing) = setup();
    cast_and_resolve(&mut game, spell, vec![Color::Red, Color::Red]);

    assert_eq!(game.zone_of(land), Some(Zone::Graveyard));
    assert_eq!(
        (
            game.characteristics(friendly).unwrap().power,
            game.characteristics(friendly).unwrap().toughness
        ),
        (Some(3), Some(3)),
    );
    assert_eq!(
        (
            game.characteristics(opposing).unwrap().power,
            game.characteristics(opposing).unwrap().toughness
        ),
        (Some(6), Some(2)),
    );
    assert!(
        game.event_log
            .iter()
            .all(|event| { !matches!(event, GameEvent::ContinuousEffectCreated { .. }) })
    );
    game.validate_invariants()
        .expect("nonblack-spent trace remains valid");
}

#[test]
fn rolling_spoil_is_positive_manifest_with_the_exact_conditional_effect() {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-ROLLING-SPOIL")
        .expect("Rolling Spoil definition");
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&definition.id));
    assert_eq!(
        definition.mana_cost,
        ManaCost::with_colors(2, [Color::Green, Color::Green])
    );
    assert_eq!(
        definition.effects,
        vec![
            Effect::DestroyTargetLand,
            Effect::ModifyAllCreaturesPtUntilEndOfTurnIfManaColorSpent {
                color: Color::Black,
                power: -1,
                toughness: -1,
            },
        ]
    );
}
