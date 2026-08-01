//! Red discovery contract for Gate Hound's Aura-conditioned static effect.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardType, CastRequest, Color, Game, Keyword, ManaCost, PlayerId, Target, Zone,
};
use cardbench_magic_rav::{
    RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, rav_activated_ability_bindings,
    rav_additional_spell_cost_bindings, rav_basic_land_type_bindings, rav_mana_ability_bindings,
    rav_static_continuous_effect_bindings,
};

fn game_with_static_bindings() -> Game {
    Game::new_with_all_bindings_and_static_continuous_effects(
        card_definitions(),
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        rav_activated_ability_bindings(),
        rav_static_continuous_effect_bindings(),
    )
    .expect("RAV static-binding game builds")
}

#[test]
fn gate_hound_requires_its_enchanted_controller_vigilance_static_effect() {
    let hound = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-GATE-HOUND")
        .expect("Gate Hound definition exists");

    assert_eq!(hound.name, "Gate Hound");
    assert_eq!(hound.mana_cost, ManaCost::with_colors(2, [Color::White]));
    assert_eq!(hound.colors, BTreeSet::from([Color::White]));
    assert_eq!(hound.card_types, BTreeSet::from([CardType::Creature]));
    assert_eq!((hound.power, hound.toughness), (Some(1), Some(1)));
    assert!(hound.keywords.is_empty());
    assert_eq!(hound.effects, []);
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&hound.id));
    assert!(
        hound
            .supported_rules
            .contains(&"static-controller-vigilance-while-enchanted")
    );
}

#[test]
fn gate_hound_grants_controller_vigilance_only_while_a_live_aura_is_attached() {
    let mut game = game_with_static_bindings();
    let hound = game
        .add_card(PlayerId(0), "RAV-GATE-HOUND", Zone::Battlefield)
        .expect("Gate Hound setup");
    let ally = game
        .add_card(PlayerId(0), "RAV-WATCHWOLF", Zone::Battlefield)
        .expect("friendly creature setup");
    let opponent = game
        .add_card(PlayerId(1), "RAV-WATCHWOLF", Zone::Battlefield)
        .expect("opponent creature setup");
    let cloak = game
        .add_card(PlayerId(0), "RAV-MOLDERVINE-CLOAK", Zone::Hand)
        .expect("Aura setup");
    let removal = game
        .add_card(PlayerId(0), "RAV-SEED-SPARK", Zone::Hand)
        .expect("Aura removal setup");
    game.grant_mana(PlayerId(0), Color::Green, 8)
        .expect("fixture green mana");
    game.grant_mana(PlayerId(0), Color::White, 3)
        .expect("fixture white mana");

    assert!(
        !game
            .characteristics(hound)
            .expect("hound characteristics")
            .keywords
            .contains(&Keyword::Vigilance)
    );
    game.cast_spell(
        PlayerId(0),
        CastRequest {
            card: cloak,
            targets: vec![Target::Permanent(hound)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("Aura targets Gate Hound");
    game.pass_priority(PlayerId(0)).expect("caster passes Aura");
    game.pass_priority(PlayerId(1)).expect("Aura resolves");

    assert_eq!(
        game.object(cloak).expect("Aura persists").attached_to,
        Some(hound)
    );
    for creature in [hound, ally] {
        assert!(
            game.characteristics(creature)
                .expect("friendly creature characteristics")
                .keywords
                .contains(&Keyword::Vigilance),
            "every creature controlled by the enchanted Hound's controller gains Vigilance"
        );
    }
    assert!(
        !game
            .characteristics(opponent)
            .expect("opponent creature characteristics")
            .keywords
            .contains(&Keyword::Vigilance),
        "the static effect does not cross controller boundaries"
    );

    game.cast_spell(
        PlayerId(0),
        CastRequest {
            card: removal,
            targets: vec![Target::Permanent(cloak)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("Seed Spark targets the live Aura");
    game.pass_priority(PlayerId(0))
        .expect("caster passes removal");
    game.pass_priority(PlayerId(1)).expect("removal resolves");

    assert_eq!(game.zone_of(cloak), Some(Zone::Graveyard));
    for creature in [hound, ally, opponent] {
        assert!(
            !game
                .characteristics(creature)
                .expect("creature characteristics after Aura departure")
                .keywords
                .contains(&Keyword::Vigilance),
            "the Aura-conditioned static effect leaves no stale keyword"
        );
    }
    assert!(
        game.canonical_event_log()
            .iter()
            .any(|event| event.contains("CardDestroyed")),
        "the ordinary enchantment-destruction receipt documents the attachment departure"
    );
    game.validate_invariants()
        .expect("Aura-conditioned static effect preserves invariants");
}
