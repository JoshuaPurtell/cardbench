//! Full-fidelity contracts for Golgari Guildmage's paired creature abilities.

use cardbench_magic_engine::{
    AbilityActivation, CardType, Color, Effect, Game, GameEvent, HybridManaSymbol, Keyword,
    ManaCost, PlayerId, Target, TargetRequirement,
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

#[test]
fn golgari_guildmage_has_exact_hybrid_definition_and_one_target_per_ability() {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-GOLGARI-GUILDMAGE")
        .expect("Golgari Guildmage definition exists");
    assert_eq!(
        executable_definition_id_for_collector(248),
        Ok("RAV-GOLGARI-GUILDMAGE")
    );
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&definition.id));
    assert_eq!(
        definition.card_types,
        [CardType::Creature].into_iter().collect()
    );
    assert_eq!(
        definition.mana_cost,
        ManaCost::with_hybrid(
            0,
            [],
            [
                HybridManaSymbol {
                    first: Color::Black,
                    second: Color::Green,
                },
                HybridManaSymbol {
                    first: Color::Black,
                    second: Color::Green,
                },
            ],
        )
    );
    let bindings = rav_activated_ability_bindings();
    let pump = bindings
        .iter()
        .find(|binding| {
            binding.card_definition == definition.id
                && binding.ability.id == "target-pump-and-trample"
        })
        .expect("pump binding exists");
    assert_eq!(pump.ability.targets, [TargetRequirement::Creature]);
    assert_eq!(
        pump.ability.effects,
        [Effect::ModifyTargetPtAndKeywordUntilEndOfTurn {
            power: 1,
            toughness: 1,
            keyword: Keyword::Trample,
        }]
    );
    let regenerate = bindings
        .iter()
        .find(|binding| {
            binding.card_definition == definition.id
                && binding.ability.id == "regenerate-target-creature"
        })
        .expect("regeneration binding exists");
    assert_eq!(regenerate.ability.targets, [TargetRequirement::Creature]);
    assert_eq!(
        regenerate.ability.effects,
        [Effect::RegenerateTargetCreature]
    );
}

#[test]
fn golgari_guildmage_resolves_both_creature_abilities_with_ordered_receipts() {
    let mut game = game_with_rav_bindings();
    let guildmage = game
        .put_on_battlefield(PlayerId(0), "RAV-GOLGARI-GUILDMAGE")
        .expect("Guildmage begins on battlefield");
    let target = game
        .put_on_battlefield(PlayerId(0), "RAV-WATCHWOLF")
        .expect("creature target begins on battlefield");
    let swamp = game
        .put_on_battlefield(PlayerId(0), "RAV-SWAMP")
        .expect("Swamp begins on battlefield");
    let forest = game
        .put_on_battlefield(PlayerId(0), "RAV-FOREST")
        .expect("Forest begins on battlefield");
    let second_swamp = game
        .put_on_battlefield(PlayerId(0), "RAV-SWAMP")
        .expect("second Swamp begins on battlefield");
    let second_forest = game
        .put_on_battlefield(PlayerId(0), "RAV-FOREST")
        .expect("second Forest begins on battlefield");
    let mountain_one = game
        .put_on_battlefield(PlayerId(0), "RAV-MOUNTAIN")
        .expect("first Mountain begins on battlefield");
    let mountain_two = game
        .put_on_battlefield(PlayerId(0), "RAV-MOUNTAIN")
        .expect("second Mountain begins on battlefield");
    game.begin_game().expect("fixture begins game");

    game.activate_mana_ability(PlayerId(0), swamp, Color::Black)
        .expect("Swamp pays pump");
    game.activate_mana_ability(PlayerId(0), forest, Color::Green)
        .expect("Forest pays pump");
    game.activate_ability(
        PlayerId(0),
        AbilityActivation {
            source: guildmage,
            ability_id: "target-pump-and-trample",
            sacrifice_sources: vec![],
            additional_tap_creatures: vec![],
            discard_cards: vec![],
            targets: vec![Target::Permanent(target)],
        },
    )
    .expect("Guildmage pump enters the stack with one target");
    resolve_top(&mut game);
    let characteristics = game.characteristics(target).expect("target remains live");
    assert_eq!(
        (characteristics.power, characteristics.toughness),
        (Some(4), Some(4))
    );
    assert!(characteristics.keywords.contains(&Keyword::Trample));

    for (land, color) in [
        (second_swamp, Color::Black),
        (second_forest, Color::Green),
        (mountain_one, Color::Red),
        (mountain_two, Color::Red),
    ] {
        game.activate_mana_ability(PlayerId(0), land, color)
            .expect("land pays regeneration activation");
    }
    game.activate_ability(
        PlayerId(0),
        AbilityActivation {
            source: guildmage,
            ability_id: "regenerate-target-creature",
            sacrifice_sources: vec![],
            additional_tap_creatures: vec![],
            discard_cards: vec![],
            targets: vec![Target::Permanent(target)],
        },
    )
    .expect("Guildmage regeneration enters the stack");
    resolve_top(&mut game);

    println!("Golgari Guildmage trace: {:#?}", game.canonical_event_log());
    let pump_effects = game
        .event_log
        .iter()
        .filter(|event| {
            matches!(
                event,
                GameEvent::ContinuousEffectCreated { source, target: affected, .. }
                    if *source == guildmage && *affected == target
            )
        })
        .count();
    assert_eq!(
        pump_effects, 2,
        "one target produces exactly two layer receipts"
    );
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::RegenerationShieldCreated { source, target: shielded }
            if *source == guildmage && *shielded == target
    )));
    game.validate_invariants()
        .expect("Guildmage event trace preserves invariants");
}
