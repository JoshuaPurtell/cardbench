use std::collections::BTreeSet;

use cardbench_magic_engine::{
    AbilityActivation, CardType, Color, Game, GameEvent, Keyword, PlayerId, Target,
};
use cardbench_magic_rav::{
    card_definitions, rav_activated_ability_bindings, rav_additional_spell_cost_bindings,
    rav_basic_land_type_bindings, rav_mana_ability_bindings,
};

#[test]
fn boros_guildmage_is_executable_with_both_keyword_grant_abilities() {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-BOROS-GUILDMAGE")
        .expect("Boros Guildmage exists");
    assert_eq!(definition.name, "Boros Guildmage");
    assert_eq!(
        definition.colors,
        BTreeSet::from([Color::Red, Color::White])
    );
    assert_eq!(definition.card_types, BTreeSet::from([CardType::Creature]));
    assert_eq!((definition.power, definition.toughness), (Some(2), Some(2)));
    assert!(
        definition
            .supported_rules
            .contains(&"activated-grant-haste")
    );
    assert!(
        definition
            .supported_rules
            .contains(&"activated-grant-first-strike")
    );
}

#[test]
fn boros_guildmage_grants_haste_and_first_strike_until_end_of_turn() {
    let mut game = Game::new_with_all_bindings(
        card_definitions(),
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        rav_activated_ability_bindings(),
    )
    .expect("RAV game builds");
    let guildmage = game
        .put_on_battlefield(PlayerId(0), "RAV-BOROS-GUILDMAGE")
        .expect("Guildmage enters");
    let target = game
        .put_on_battlefield(PlayerId(0), "RAV-WATCHWOLF")
        .expect("target creature enters");
    game.begin_game().expect("game starts");
    game.add_mana_from_action(PlayerId(0), Color::Red, 1)
        .expect("red ability mana");
    game.activate_ability(
        PlayerId(0),
        AbilityActivation {
            source: guildmage,
            ability_id: "grant-haste",
            sacrifice_sources: vec![],
            discard_cards: vec![],
            targets: vec![Target::Permanent(target)],
        },
    )
    .expect("haste ability activates");
    game.pass_priority(PlayerId(0))
        .expect("haste activator passes");
    game.pass_priority(PlayerId(1))
        .expect("haste ability resolves");
    assert!(
        game.characteristics(target)
            .expect("target characteristics")
            .keywords
            .contains(&Keyword::Haste)
    );

    game.add_mana_from_action(PlayerId(0), Color::White, 1)
        .expect("white ability mana");
    game.activate_ability(
        PlayerId(0),
        AbilityActivation {
            source: guildmage,
            ability_id: "grant-first-strike",
            sacrifice_sources: vec![],
            discard_cards: vec![],
            targets: vec![Target::Permanent(target)],
        },
    )
    .expect("first-strike ability activates");
    game.pass_priority(PlayerId(0))
        .expect("first-strike activator passes");
    game.pass_priority(PlayerId(1))
        .expect("first-strike ability resolves");
    let keywords = &game
        .characteristics(target)
        .expect("target characteristics")
        .keywords;
    assert!(keywords.contains(&Keyword::Haste));
    assert!(keywords.contains(&Keyword::FirstStrike));
    assert!(
        game.event_log
            .iter()
            .filter(|event| matches!(
                event,
                GameEvent::ContinuousEffectCreated { source, target: effect_target, .. }
                    if *source == guildmage && *effect_target == target
            ))
            .count()
            >= 2
    );
}
