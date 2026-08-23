//! Full-fidelity persistent land-animation contract for Woodwraith Corrupter.

use cardbench_magic_engine::{
    AbilityActivation, CardType, Color, CreatureSubtype, Game, GameEvent, PlayerId, Step, Target,
    Zone,
};
use cardbench_magic_rav::{
    RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, rav_activated_ability_bindings,
    rav_basic_land_type_bindings, rav_mana_ability_bindings,
};

fn pass_pair(game: &mut Game) {
    let first = game.priority;
    game.pass_priority(first).expect("first player passes");
    let second = game.priority;
    game.pass_priority(second).expect("second player passes");
}

fn advance_to_source_controllers_next_main(game: &mut Game) {
    for _ in 0..64 {
        if game.turn == 3 && game.active_player == PlayerId(0) && game.step == Step::PrecombatMain {
            return;
        }
        if game.step == Step::DeclareAttackers
            && !game
                .view_for_player(game.priority)
                .expect("attacker view")
                .attackers_declared
        {
            game.declare_attackers(game.priority, &[])
                .expect("empty attacks advance combat");
        }
        if game.step == Step::DeclareBlockers
            && !game
                .view_for_player(game.priority)
                .expect("blocker view")
                .blockers_declared
        {
            game.declare_blockers(game.priority, &[])
                .expect("empty blocks advance combat");
        }
        if game
            .view_for_player(game.priority)
            .expect("priority view")
            .draw_replacement_pending
        {
            game.resolve_pending_draw(game.priority, None)
                .expect("ordinary draw resolves");
        } else {
            let player = game.priority;
            game.pass_priority(player).expect("current player passes");
        }
    }
    panic!("turn state machine did not reach player zero's next main phase");
}

fn activation(
    source: cardbench_magic_engine::ObjectId,
    target: cardbench_magic_engine::ObjectId,
) -> AbilityActivation {
    AbilityActivation {
        source,
        ability_id: "animate-target-forest",
        sacrifice_sources: vec![],
        additional_tap_creatures: vec![],
        discard_cards: vec![],
        targets: vec![Target::Permanent(target)],
    }
}

#[test]
#[allow(clippy::too_many_lines)] // The event-sequence assertions are the contract under test.
fn woodwraith_corrupter_animates_a_forest_independently_of_its_source_lifetime() {
    let mut game = Game::new_with_all_bindings(
        card_definitions(),
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        [],
        rav_activated_ability_bindings(),
    )
    .expect("RAV catalog constructs");
    let corrputer = game
        .put_on_battlefield(PlayerId(0), "RAV-WOODWRAITH-CORRUPTER")
        .expect("Corrupter begins on the battlefield");
    let payment_forest = game
        .put_on_battlefield(PlayerId(0), "RAV-FOREST")
        .expect("green source begins on the battlefield");
    let payment_swamp = game
        .put_on_battlefield(PlayerId(0), "RAV-SWAMP")
        .expect("black source begins on the battlefield");
    let payment_plains = game
        .put_on_battlefield(PlayerId(0), "RAV-PLAINS")
        .expect("generic source begins on the battlefield");
    let target_forest = game
        .put_on_battlefield(PlayerId(1), "RAV-FOREST")
        .expect("target Forest begins on the battlefield");
    let non_forest = game
        .put_on_battlefield(PlayerId(1), "RAV-SWAMP")
        .expect("non-Forest target probe begins on the battlefield");
    let putrefy = game
        .add_card(PlayerId(1), "RAV-PUTREFY", Zone::Hand)
        .expect("removal spell enters opponent hand");
    for player in [PlayerId(0), PlayerId(1)] {
        for _ in 0..3 {
            game.add_card(player, "RAV-PLAINS", Zone::Library)
                .expect("draw-padding card enters library");
        }
    }
    game.begin_game().expect("fixture begins game");
    advance_to_source_controllers_next_main(&mut game);

    let before = game.canonical_event_log();
    assert!(
        game.activate_ability(PlayerId(0), activation(corrputer, non_forest))
            .is_err(),
        "a non-Forest must be rejected before the activation pays or taps"
    );
    assert_eq!(
        game.canonical_event_log(),
        before,
        "illegal target is atomic"
    );
    assert!(!game.object(corrputer).expect("source exists").tapped);

    game.activate_mana_ability(PlayerId(0), payment_forest, Color::Green)
        .expect("green mana pays the animation");
    game.activate_mana_ability(PlayerId(0), payment_swamp, Color::Black)
        .expect("black mana pays the animation");
    game.activate_mana_ability(PlayerId(0), payment_plains, Color::White)
        .expect("generic mana pays the animation");
    game.activate_ability(PlayerId(0), activation(corrputer, target_forest))
        .expect("typed Forest activation enters the stack");
    pass_pair(&mut game);

    let animated = game
        .characteristics(target_forest)
        .expect("target remains live");
    assert!(animated.card_types.contains(&CardType::Land));
    assert!(animated.card_types.contains(&CardType::Creature));
    assert_eq!(
        animated.colors,
        std::collections::BTreeSet::from([Color::Black, Color::Green])
    );
    assert_eq!((animated.power, animated.toughness), (Some(4), Some(4)));
    assert_eq!(
        animated.creature_subtypes,
        std::collections::BTreeSet::from([CreatureSubtype::Elemental, CreatureSubtype::Horror])
    );

    game.pass_priority(PlayerId(0))
        .expect("source controller passes to removal player");
    game.activate_mana_ability(PlayerId(1), target_forest, Color::Green)
        .expect("animated Forest retains its intrinsic mana ability");
    game.activate_mana_ability(PlayerId(1), non_forest, Color::Black)
        .expect("opponent's Swamp produces black mana");
    game.cast_spell(
        PlayerId(1),
        cardbench_magic_engine::CastRequest {
            card: putrefy,
            targets: vec![Target::Permanent(corrputer)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("opponent may remove the animation source");
    pass_pair(&mut game);

    println!(
        "Woodwraith Corrupter trace: {:?}",
        game.canonical_event_log()
    );
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&"RAV-WOODWRAITH-CORRUPTER"));
    assert_eq!(game.zone_of(corrputer), Some(Zone::Graveyard));
    let after_departure = game
        .characteristics(target_forest)
        .expect("animated land remains live");
    assert!(after_departure.card_types.contains(&CardType::Creature));
    assert_eq!(
        after_departure.colors,
        std::collections::BTreeSet::from([Color::Black, Color::Green])
    );
    assert_eq!(
        (after_departure.power, after_departure.toughness),
        (Some(4), Some(4))
    );
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::ContinuousEffectCreated { source, target, .. }
            if *source == corrputer && *target == target_forest
    )));
    assert!(
        !game.event_log.iter().any(|event| matches!(
            event,
            GameEvent::ContinuousEffectExpired { source, target, .. }
                if *source == corrputer && *target == target_forest
        )),
        "target-lifetime animation must not expire merely because its source leaves"
    );
    game.validate_invariants()
        .expect("persistent animation preserves every state-machine invariant");
}
