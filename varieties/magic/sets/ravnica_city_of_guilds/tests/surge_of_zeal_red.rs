//! Red discovery contract for Surge of Zeal's Radiance haste grant.

use cardbench_magic_engine::{
    CardType, CastRequest, Color, Effect, Game, GameEvent, Keyword, ManaCost, PlayerId, Target,
    TargetRequirement, Zone,
};
use cardbench_magic_rav::{RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions};
use cardbench_magic_rav::{
    rav_activated_ability_bindings, rav_additional_spell_cost_bindings,
    rav_basic_land_type_bindings, rav_mana_ability_bindings,
};

#[test]
fn surge_of_zeal_requires_its_radiance_haste_effect() {
    let surge = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-SURGE-OF-ZEAL")
        .expect("Surge of Zeal definition exists");
    assert_eq!(surge.name, "Surge of Zeal");
    assert_eq!(surge.mana_cost, ManaCost::with_colors(0, [Color::Red]));
    assert_eq!(surge.colors, [Color::Red].into_iter().collect());
    assert_eq!(surge.card_types, [CardType::Instant].into_iter().collect());
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&surge.id));
    assert!(surge.supported_rules.contains(&"radiance-grant-haste"));
    assert_eq!(
        surge.effects,
        vec![Effect::RadianceAddKeywordUntilEndOfTurn {
            keyword: Keyword::Haste,
        }]
    );
    assert_eq!(
        surge.effects[0].target_requirement(),
        Some(TargetRequirement::Creature)
    );
}

#[test]
fn surge_of_zeal_grants_haste_to_the_target_and_shared_color_creatures() {
    let mut game = Game::new_with_all_bindings(
        card_definitions(),
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        rav_activated_ability_bindings(),
    )
    .expect("RAV game builds");
    let target = game
        .put_on_battlefield(PlayerId(0), "RAV-WATCHWOLF")
        .expect("target enters");
    let shared = game
        .put_on_battlefield(PlayerId(1), "RAV-BOROS-RECRUIT")
        .expect("shared-color creature enters");
    let unrelated = game
        .put_on_battlefield(PlayerId(1), "RAV-GOLGARI-THUG")
        .expect("unrelated creature enters");
    let mountain = game
        .put_on_battlefield(PlayerId(0), "RAV-MOUNTAIN")
        .expect("Mountain enters");
    for card in [target, shared, unrelated, mountain] {
        game.set_entered_turn_for_setup(card, 0)
            .expect("prior-turn provenance");
    }
    let surge = game
        .add_card(PlayerId(0), "RAV-SURGE-OF-ZEAL", Zone::Hand)
        .expect("Surge enters hand");
    game.begin_game().expect("game starts");
    for player in [PlayerId(0), PlayerId(1), PlayerId(0), PlayerId(1)] {
        game.pass_priority(player)
            .expect("advance to precombat main");
    }
    game.activate_mana_ability(PlayerId(0), mountain, Color::Red)
        .expect("red mana");
    game.cast_spell(
        PlayerId(0),
        CastRequest {
            card: surge,
            targets: vec![Target::Permanent(target)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("cast Surge of Zeal");
    game.pass_priority(PlayerId(0)).expect("spell pass");
    game.pass_priority(PlayerId(1)).expect("spell resolves");
    for card in [target, shared] {
        assert!(
            game.characteristics(card)
                .expect("creature characteristics")
                .keywords
                .contains(&Keyword::Haste),
            "Radiance target set includes {card:?}"
        );
    }
    assert!(
        !game
            .characteristics(unrelated)
            .expect("unrelated characteristics")
            .keywords
            .contains(&Keyword::Haste)
    );
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::ContinuousEffectCreated { target: effect_target, .. }
            if *effect_target == target
    )));
}
