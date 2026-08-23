//! Red regression for Moldervine Cloak's persistent attachment slice.

use cardbench_magic_engine::{
    CardType, CastRequest, Color, Game, GameEvent, Keyword, ManaCost, PlayerId, Target, Zone,
};
use cardbench_magic_rav::{RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions};

#[test]
fn moldervine_cloak_has_exact_cost_dredge_and_attachment_support() {
    let cloak = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-MOLDERVINE-CLOAK")
        .expect("Moldervine Cloak definition exists");
    assert_eq!(cloak.name, "Moldervine Cloak");
    assert_eq!(cloak.mana_cost, ManaCost::with_colors(2, [Color::Green]));
    assert_eq!(
        cloak.card_types,
        [CardType::Enchantment].into_iter().collect()
    );
    assert!(cloak.keywords.contains(&Keyword::Dredge(2)));
    assert!(cloak.supported_rules.contains(&"aura-attach-and-static-pt"));
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&cloak.id));
}

#[test]
fn moldervine_cloak_resolves_as_a_persistent_plus_three_plus_three_attachment() {
    let mut game = Game::new(card_definitions(), 2).expect("RAV fixture builds");
    let cloak = game
        .add_card(PlayerId(0), "RAV-MOLDERVINE-CLOAK", Zone::Hand)
        .expect("Moldervine Cloak setup");
    let target = game
        .add_card(PlayerId(0), "RAV-GOLGARI-BROWNSCALE", Zone::Battlefield)
        .expect("target creature setup");
    game.grant_mana(PlayerId(0), Color::Green, 3)
        .expect("Cloak mana");

    game.cast_spell(
        PlayerId(0),
        CastRequest {
            card: cloak,
            targets: vec![Target::Permanent(target)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("Moldervine Cloak casts");
    game.pass_priority(PlayerId(0)).expect("caster passes");
    game.pass_priority(PlayerId(1)).expect("Cloak resolves");

    assert_eq!(game.zone_of(cloak), Some(Zone::Battlefield));
    assert_eq!(
        game.object(cloak).expect("Cloak persists").attached_to,
        Some(target)
    );
    let characteristics = game.characteristics(target).expect("target remains live");
    assert_eq!(characteristics.power, Some(5));
    assert_eq!(characteristics.toughness, Some(6));
    game.validate_invariants()
        .expect("persistent attachment preserves invariant-safe layers");
}

#[test]
fn moldervine_cloak_is_cleaned_up_when_its_attached_creature_leaves() {
    let mut game = Game::new(card_definitions(), 2).expect("RAV fixture builds");
    let cloak = game
        .add_card(PlayerId(0), "RAV-MOLDERVINE-CLOAK", Zone::Hand)
        .expect("Moldervine Cloak setup");
    let target = game
        .add_card(PlayerId(0), "RAV-WATCHWOLF", Zone::Battlefield)
        .expect("target setup");
    let removal = game
        .add_card(PlayerId(0), "RAV-PUTREFY", Zone::Hand)
        .expect("removal setup");
    game.grant_mana(PlayerId(0), Color::Green, 4)
        .expect("green mana");
    game.grant_mana(PlayerId(0), Color::Black, 4)
        .expect("black mana");

    game.cast_spell(
        PlayerId(0),
        CastRequest {
            card: cloak,
            targets: vec![Target::Permanent(target)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("Cloak casts");
    game.pass_priority(PlayerId(0)).expect("caster passes");
    game.pass_priority(PlayerId(1)).expect("Cloak resolves");
    game.cast_spell(
        PlayerId(0),
        CastRequest {
            card: removal,
            targets: vec![Target::Permanent(target)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("removal casts");
    game.pass_priority(PlayerId(0)).expect("caster passes");
    game.pass_priority(PlayerId(1)).expect("removal resolves");

    assert_eq!(game.zone_of(target), Some(Zone::Graveyard));
    assert_eq!(game.zone_of(cloak), Some(Zone::Graveyard));
    assert!(
        game.continuous_effects
            .iter()
            .all(|effect| effect.source != cloak)
    );
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::ContinuousEffectExpired { source, target: expired, .. }
            if *source == cloak && *expired == target
    )));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::StateBasedAction { card, reason }
            if *card == cloak && *reason == "Aura is not attached to a battlefield creature"
    )));
    game.validate_invariants()
        .expect("orphaned Aura is removed at the SBA fixed point");
}

#[test]
fn moldervine_cloak_is_countered_if_its_target_leaves_the_battlefield_on_the_stack() {
    let mut game = Game::new(card_definitions(), 2).expect("RAV fixture builds");
    let cloak = game
        .add_card(PlayerId(0), "RAV-MOLDERVINE-CLOAK", Zone::Hand)
        .expect("Moldervine Cloak setup");
    let target = game
        .add_card(PlayerId(0), "RAV-WATCHWOLF", Zone::Battlefield)
        .expect("target setup");
    let removal = game
        .add_card(PlayerId(1), "RAV-PUTREFY", Zone::Hand)
        .expect("response setup");
    game.grant_mana(PlayerId(0), Color::Green, 3)
        .expect("Cloak mana");
    game.grant_mana(PlayerId(1), Color::Green, 1)
        .expect("response green mana");
    game.grant_mana(PlayerId(1), Color::Black, 1)
        .expect("response black mana");

    game.cast_spell(
        PlayerId(0),
        CastRequest {
            card: cloak,
            targets: vec![Target::Permanent(target)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("Cloak casts");
    game.pass_priority(PlayerId(0)).expect("caster passes");
    game.cast_spell(
        PlayerId(1),
        CastRequest {
            card: removal,
            targets: vec![Target::Permanent(target)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("response casts");
    game.pass_priority(PlayerId(1)).expect("responder passes");
    game.pass_priority(PlayerId(0)).expect("removal resolves");
    game.pass_priority(PlayerId(0))
        .expect("caster passes again");
    game.pass_priority(PlayerId(1))
        .expect("Cloak is countered by rules");

    assert_eq!(game.zone_of(target), Some(Zone::Graveyard));
    assert_eq!(game.zone_of(cloak), Some(Zone::Graveyard));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::SpellCounteredByRules { card } if *card == cloak
    )));
    assert!(
        !game
            .event_log
            .iter()
            .any(|event| matches!(event, GameEvent::AuraAttached { aura, .. } if *aura == cloak))
    );
    assert!(
        game.continuous_effects
            .iter()
            .all(|effect| effect.source != cloak)
    );
    game.validate_invariants()
        .expect("countered Aura leaves no attachment state behind");
}
