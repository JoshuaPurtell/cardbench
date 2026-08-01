//! Full-fidelity contracts for Helldozer's conditional land-destruction tap ability.

use cardbench_magic_engine::{
    AbilityActivation, Color, Effect, Game, GameEvent, ManaCost, PlayerId, Target,
    TargetRequirement, Zone,
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

fn resolve_top(game: &mut Game) {
    let first = game.priority;
    game.pass_priority(first).expect("first priority pass");
    let second = game.priority;
    game.pass_priority(second)
        .expect("second priority pass resolves");
}

fn activate_helldozer(
    game: &mut Game,
    source: cardbench_magic_engine::ObjectId,
    swamps: &[cardbench_magic_engine::ObjectId],
    target: cardbench_magic_engine::ObjectId,
) {
    for swamp in swamps {
        game.activate_mana_ability(PlayerId(0), *swamp, Color::Black)
            .expect("Swamp pays one black symbol");
    }
    game.activate_ability(
        PlayerId(0),
        AbilityActivation {
            source,
            ability_id: "destroy-target-land",
            sacrifice_sources: vec![],
            additional_tap_creatures: vec![],
            discard_cards: vec![],
            targets: vec![Target::Permanent(target)],
        },
    )
    .expect("Helldozer activation enters the stack");
}

#[test]
fn helldozer_definition_and_effect_keep_the_triple_black_conditional_instruction() {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-HELLDOZER")
        .expect("Helldozer definition exists");
    assert_eq!(
        executable_definition_id_for_collector(88),
        Ok("RAV-HELLDOZER")
    );
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&definition.id));
    assert_eq!(
        definition.mana_cost,
        ManaCost::with_colors(3, [Color::Black, Color::Black, Color::Black])
    );
    assert_eq!((definition.power, definition.toughness), (Some(6), Some(5)));
    let ability = &rav_activated_ability_bindings()
        .into_iter()
        .find(|binding| {
            binding.card_definition == definition.id && binding.ability.id == "destroy-target-land"
        })
        .expect("Helldozer ability exists")
        .ability;
    assert_eq!(
        ability.mana_cost,
        ManaCost::with_colors(0, [Color::Black, Color::Black, Color::Black])
    );
    assert!(ability.tap_cost);
    assert_eq!(ability.targets, [TargetRequirement::Land]);
    assert_eq!(
        ability.effects,
        [Effect::DestroyTargetLandAndUntapSourceIfNonbasic]
    );
}

#[test]
fn helldozer_untaps_only_after_destroying_a_nonbasic_land() {
    let mut game = game_with_rav_bindings();
    let helldozer = game
        .put_on_battlefield(PlayerId(0), "RAV-HELLDOZER")
        .expect("Helldozer begins on battlefield");
    let target = game
        .put_on_battlefield(PlayerId(1), "RAV-SUNHOME-FORTRESS")
        .expect("nonbasic target begins on battlefield");
    let swamps = (0..3)
        .map(|_| {
            game.put_on_battlefield(PlayerId(0), "RAV-SWAMP")
                .expect("Swamp begins on battlefield")
        })
        .collect::<Vec<_>>();
    game.set_entered_turn_for_setup(helldozer, 0)
        .expect("Helldozer predates the measured turn");
    game.begin_game().expect("fixture begins game");

    activate_helldozer(&mut game, helldozer, &swamps, target);
    assert!(
        game.object(helldozer)
            .expect("source remains allocated")
            .tapped,
        "tap cost is paid before the stack resolves"
    );
    resolve_top(&mut game);

    println!(
        "Helldozer nonbasic trace: {:#?}",
        game.canonical_event_log()
    );
    assert_eq!(game.zone_of(target), Some(Zone::Graveyard));
    assert!(
        !game
            .object(helldozer)
            .expect("source remains allocated")
            .tapped,
        "nonbasic target untaps the source after destruction"
    );
    let destroyed = game
        .event_log
        .iter()
        .position(|event| {
            matches!(
                event,
                GameEvent::CardDestroyed { source, card } if *source == helldozer && *card == target
            )
        })
        .expect("destroy receipt exists");
    let untapped = game
        .event_log
        .iter()
        .position(|event| matches!(
            event,
            GameEvent::PermanentUntapped { source, card } if *source == helldozer && *card == helldozer
        ))
        .expect("conditional untap receipt exists");
    assert!(destroyed < untapped);
    game.validate_invariants()
        .expect("nonbasic Helldozer trace preserves invariants");
}

#[test]
fn helldozer_stays_tapped_after_destroying_a_basic_land() {
    let mut game = game_with_rav_bindings();
    let helldozer = game
        .put_on_battlefield(PlayerId(0), "RAV-HELLDOZER")
        .expect("Helldozer begins on battlefield");
    let target = game
        .put_on_battlefield(PlayerId(1), "RAV-FOREST")
        .expect("basic target begins on battlefield");
    let swamps = (0..3)
        .map(|_| {
            game.put_on_battlefield(PlayerId(0), "RAV-SWAMP")
                .expect("Swamp begins on battlefield")
        })
        .collect::<Vec<_>>();
    game.set_entered_turn_for_setup(helldozer, 0)
        .expect("Helldozer predates the measured turn");
    game.begin_game().expect("fixture begins game");

    activate_helldozer(&mut game, helldozer, &swamps, target);
    resolve_top(&mut game);

    println!("Helldozer basic trace: {:#?}", game.canonical_event_log());
    assert_eq!(game.zone_of(target), Some(Zone::Graveyard));
    assert!(
        game.object(helldozer)
            .expect("source remains allocated")
            .tapped,
        "basic target does not satisfy the source-untap condition"
    );
    assert!(!game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::PermanentUntapped { source, card } if *source == helldozer && *card == helldozer
    )));
    game.validate_invariants()
        .expect("basic Helldozer trace preserves invariants");
}
