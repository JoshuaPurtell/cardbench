//! Full-fidelity contract for Svogthos's dynamic graveyard-count animation.
//!
//! The public trace deliberately changes the captured controller's graveyard
//! after the land's activated ability resolves. That verifies a layer-7b
//! characteristic value, rather than a resolution-time snapshot.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    AbilityActivation, CardType, CastRequest, Color, CreatureSubtype, Game, GameEvent, Layer,
    ManaAbilityActivation, ManaCost, PlayerId, Step, Target, Zone,
};
use cardbench_magic_rav::{
    RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, executable_definition_id_for_collector,
    rav_activated_ability_bindings, rav_additional_spell_cost_bindings,
    rav_basic_land_type_bindings, rav_mana_ability_bindings,
};

fn game_with_rav_bindings() -> Game {
    Game::new_with_all_bindings(
        card_definitions(),
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        rav_activated_ability_bindings(),
    )
    .expect("RAV game builds")
}

fn pass_pair(game: &mut Game, first: PlayerId, second: PlayerId) {
    game.pass_priority(first).expect("first player passes");
    game.pass_priority(second).expect("second player passes");
}

fn advance_until_animation_expires(game: &mut Game, source: cardbench_magic_engine::ObjectId) {
    for _ in 0..32 {
        if game.event_log.iter().any(|event| matches!(
            event,
            GameEvent::ContinuousEffectExpired { source: expired_source, layer: Layer::PowerToughness, .. }
                if *expired_source == source
        )) {
            return;
        }
        if game.step == Step::DeclareAttackers
            && !game
                .view_for_player(game.active_player)
                .expect("active player view exists")
                .attackers_declared
        {
            game.declare_attackers(game.active_player, &[])
                .expect("empty attacker declaration is legal");
            continue;
        }
        let priority = game.priority;
        game.pass_priority(priority)
            .expect("priority advances toward cleanup");
    }
    panic!("Svogthos animation did not expire during its resolving turn");
}

#[test]
#[allow(clippy::too_many_lines)] // The stack and dynamic-layer transcript is intentionally reviewed as one contract.
fn svogthos_animation_uses_current_captured_controller_graveyard_count() {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-SVOGTHOS-THE-RESTLESS-TOMB")
        .expect("Svogthos definition exists");
    assert_eq!(
        executable_definition_id_for_collector(283),
        Ok("RAV-SVOGTHOS-THE-RESTLESS-TOMB")
    );
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&definition.id));
    assert_eq!(definition.mana_cost, ManaCost::new(0));
    assert_eq!(definition.colors, BTreeSet::new());
    assert_eq!(definition.mana_colors, BTreeSet::from([Color::Colorless]));
    assert_eq!(definition.card_types, BTreeSet::from([CardType::Land]));
    assert_eq!(
        definition.supported_rules,
        [
            "full-rules-fidelity",
            "colorless-mana-ability",
            "activated-dynamic-graveyard-creature-animation",
        ]
    );

    let mut game = game_with_rav_bindings();
    let svogthos = game
        .put_on_battlefield(PlayerId(0), definition.id)
        .expect("Svogthos begins on the battlefield");
    let generic_lands = (0..3)
        .map(|_| {
            game.put_on_battlefield(PlayerId(0), "RAV-PLAINS")
                .expect("generic payment land begins on battlefield")
        })
        .collect::<Vec<_>>();
    let forest = game
        .put_on_battlefield(PlayerId(0), "RAV-FOREST")
        .expect("green payment land begins on battlefield");
    let swamp = game
        .put_on_battlefield(PlayerId(0), "RAV-SWAMP")
        .expect("black payment land begins on battlefield");
    let initial_creatures = [
        game.add_card(PlayerId(0), "RAV-WATCHWOLF", Zone::Graveyard)
            .expect("first creature card begins in controller graveyard"),
        game.add_card(PlayerId(0), "RAV-WATCHWOLF", Zone::Graveyard)
            .expect("second creature card begins in controller graveyard"),
    ];
    let noncreature = game
        .add_card(PlayerId(0), "RAV-FOREST", Zone::Graveyard)
        .expect("noncreature card begins in controller graveyard");
    let changing_creature = game
        .put_on_battlefield(PlayerId(0), "RAV-WATCHWOLF")
        .expect("creature exists to move through ordinary state-based actions");
    let removal = game
        .add_card(PlayerId(1), "RAV-LAST-GASP", Zone::Hand)
        .expect("opponent removal starts in hand");
    let opponent_swamp = game
        .put_on_battlefield(PlayerId(1), "RAV-SWAMP")
        .expect("opponent black payment source begins on battlefield");
    let generic_swamp = game.put_on_battlefield(PlayerId(1), "RAV-SWAMP").unwrap();
    game.begin_game().expect("game starts");
    game.clear_event_log();

    for land in generic_lands {
        game.activate_mana_ability(PlayerId(0), land, Color::White)
            .expect("generic payment mana is available");
    }
    game.activate_mana_ability(PlayerId(0), forest, Color::Green)
        .expect("green payment mana is available");
    game.activate_mana_ability(PlayerId(0), swamp, Color::Black)
        .expect("black payment mana is available");
    game.activate_ability(
        PlayerId(0),
        AbilityActivation {
            source: svogthos,
            ability_id: "animate-self-from-controller-graveyard-creatures",
            sacrifice_sources: vec![],
            additional_tap_creatures: vec![],
            discard_cards: vec![],
            targets: vec![],
        },
    )
    .expect("Svogthos animation enters the stack");
    pass_pair(&mut game, PlayerId(0), PlayerId(1));

    let first_characteristics = game
        .characteristics(svogthos)
        .expect("animated Svogthos has characteristics");
    assert_eq!(
        first_characteristics.card_types,
        BTreeSet::from([CardType::Land, CardType::Creature])
    );
    assert_eq!(
        first_characteristics.colors,
        BTreeSet::from([Color::Black, Color::Green])
    );
    assert_eq!(
        first_characteristics.creature_subtypes,
        BTreeSet::from([CreatureSubtype::Plant, CreatureSubtype::Zombie])
    );
    assert_eq!(
        (first_characteristics.power, first_characteristics.toughness),
        (Some(2), Some(2)),
        "two creature cards, but not the Forest, count immediately after resolution"
    );

    game.pass_priority(PlayerId(0))
        .expect("animation controller gives opponent priority");
    game.activate_mana_ability(PlayerId(1), opponent_swamp, Color::Black)
        .expect("opponent makes removal payment");
    game.activate_mana_ability(PlayerId(1), generic_swamp, Color::Black).unwrap();
    game.cast_spell(
        PlayerId(1),
        CastRequest {
            card: removal,
            targets: vec![Target::Permanent(changing_creature)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("opponent casts removal against the changing creature");
    pass_pair(&mut game, PlayerId(1), PlayerId(0));

    assert_eq!(game.zone_of(changing_creature), Some(Zone::Graveyard));
    assert_eq!(game.zone_of(noncreature), Some(Zone::Graveyard));
    assert!(
        initial_creatures
            .iter()
            .all(|card| game.zone_of(*card) == Some(Zone::Graveyard))
    );
    let second_characteristics = game
        .characteristics(svogthos)
        .expect("Svogthos remains animated");
    assert_eq!(
        (
            second_characteristics.power,
            second_characteristics.toughness
        ),
        (Some(3), Some(3)),
        "the layer reads the now-current captured controller graveyard, not a snapshot"
    );

    advance_until_animation_expires(&mut game, svogthos);
    let expired_characteristics = game
        .characteristics(svogthos)
        .expect("Svogthos remains a land after cleanup");
    assert_eq!(
        expired_characteristics.card_types,
        BTreeSet::from([CardType::Land])
    );
    assert_eq!(expired_characteristics.colors, BTreeSet::new());
    assert_eq!(
        (
            expired_characteristics.power,
            expired_characteristics.toughness
        ),
        (None, None),
        "the temporary animation must not survive cleanup"
    );

    println!(
        "Svogthos dynamic-animation trace: {:#?}",
        game.canonical_event_log()
    );
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::AbilityManaPaid { source, ability, mana_cost, .. }
            if *source == svogthos
                && *ability == "animate-self-from-controller-graveyard-creatures"
                && *mana_cost == ManaCost::with_colors(3, [Color::Black, Color::Green])
    )));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::ContinuousEffectCreated { source, target, layer: Layer::PowerToughness }
            if *source == svogthos && *target == svogthos
    )));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::AbilityResolved { source, ability, .. }
            if *source == svogthos && *ability == "animate-self-from-controller-graveyard-creatures"
    )));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::CardMoved { card, to: Zone::Graveyard } if *card == changing_creature
    )));
    assert_eq!(
        game.event_log
            .iter()
            .filter(|event| matches!(
                event,
                GameEvent::ContinuousEffectExpired { source, target, .. }
                    if *source == svogthos && *target == svogthos
            ))
            .count(),
        5,
        "every temporary type, subtype, color, and P/T layer expires together"
    );
    game.validate_invariants()
        .expect("Svogthos dynamic animation preserves invariants");
}

#[test]
fn svogthos_colorless_mana_uses_the_nonstack_mana_boundary() {
    let mut game = game_with_rav_bindings();
    let svogthos = game
        .put_on_battlefield(PlayerId(0), "RAV-SVOGTHOS-THE-RESTLESS-TOMB")
        .expect("Svogthos begins on battlefield");
    game.begin_game().expect("game starts");
    game.clear_event_log();

    game.activate_bound_mana_ability(
        PlayerId(0),
        ManaAbilityActivation {
            source: svogthos,
            ability_id: "produce-colorless",
            chosen_color: None,
        },
    )
    .expect("Svogthos produces colorless mana without using the stack");

    assert_eq!(game.stack.len(), 0);
    assert_eq!(
        game.player(PlayerId(0))
            .expect("controller exists")
            .mana_pool
            .amount(Color::Colorless),
        1
    );
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::BoundManaAbilityActivated { source, ability, color, amount, .. }
            if *source == svogthos
                && *ability == "produce-colorless"
                && *color == Color::Colorless
                && *amount == 1
    )));
    game.validate_invariants()
        .expect("Svogthos mana activation preserves invariants");
}
