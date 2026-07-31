//! Red discovery contract for Incite Hysteria's Radiance blocker restriction.
//!
//! The probe checks the typed target and shared-color layer effect without
//! copying upstream card prose or art.

use cardbench_magic_engine::{
    CardType, Color, Effect, Game, GameEvent, Keyword, ManaCost, PlayerId, Target, Zone,
};
use cardbench_magic_rav::{
    RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, rav_activated_ability_bindings,
    rav_additional_spell_cost_bindings, rav_basic_land_type_bindings, rav_mana_ability_bindings,
};

fn game() -> Game {
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

#[test]
fn incite_hysteria_definition_declares_radiance_blocker_restriction() {
    let incite = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-INCITE-HYSTERIA")
        .expect("Incite Hysteria definition exists");
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&incite.id));
    assert_eq!(incite.name, "Incite Hysteria");
    assert_eq!(incite.mana_cost, ManaCost::with_colors(2, [Color::Red]));
    assert_eq!(incite.card_types, [CardType::Sorcery].into());
    assert_eq!(
        incite.effects,
        vec![Effect::RadianceAddKeywordUntilEndOfTurn {
            keyword: Keyword::CannotBlock,
        }]
    );
    assert!(incite.supported_rules.contains(&"full-rules-fidelity"));
    assert!(incite.supported_rules.contains(&"radiance-cannot-block"));
}

#[test]
fn incite_hysteria_restricts_target_and_shared_color_only() {
    let mut game = game();
    let incite = game
        .add_card(PlayerId(0), "RAV-INCITE-HYSTERIA", Zone::Hand)
        .expect("Incite Hysteria enters hand");
    let target = game
        .put_on_battlefield(PlayerId(1), "RAV-FRENZIED-GOBLIN")
        .expect("red target enters battlefield");
    let shared = game
        .put_on_battlefield(PlayerId(1), "RAV-GOBLIN-SPELUNKERS")
        .expect("shared-color creature enters battlefield");
    let other = game
        .put_on_battlefield(PlayerId(1), "RAV-GOLGARI-BROWNSCALE")
        .expect("other-color creature enters battlefield");
    game.grant_mana(PlayerId(0), Color::Red, 3)
        .expect("Incite Hysteria mana");
    game.clear_event_log();

    game.cast_spell(
        PlayerId(0),
        cardbench_magic_engine::CastRequest {
            card: incite,
            targets: vec![Target::Permanent(target)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("Incite Hysteria casts at a creature");
    game.pass_priority(PlayerId(0)).expect("caster passes");
    game.pass_priority(PlayerId(1))
        .expect("Incite Hysteria resolves");

    println!("Incite Hysteria trace: {:?}", game.event_log);
    assert!(
        game.characteristics(target)
            .expect("target characteristics")
            .keywords
            .contains(&Keyword::CannotBlock)
    );
    assert!(
        game.characteristics(shared)
            .expect("shared characteristics")
            .keywords
            .contains(&Keyword::CannotBlock)
    );
    assert!(
        !game
            .characteristics(other)
            .expect("other characteristics")
            .keywords
            .contains(&Keyword::CannotBlock)
    );
    assert!(
        game.event_log
            .iter()
            .any(|event| matches!(event, GameEvent::ContinuousEffectCreated { .. }))
    );
    game.validate_invariants()
        .expect("Incite Hysteria trace valid");
}
