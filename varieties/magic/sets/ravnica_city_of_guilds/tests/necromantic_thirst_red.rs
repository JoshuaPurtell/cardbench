//! Red regression for the source-only Necromantic Thirst Aura chassis.

use cardbench_magic_engine::{
    CardType, CastRequest, Color, Effect, Game, ManaCost, PlayerId, Target, Zone,
};
use cardbench_magic_rav::card_definitions;

#[test]
fn necromantic_thirst_has_its_exact_static_aura_chassis_without_unimplemented_trigger_claims() {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-NECROMANTIC-THIRST")
        .expect("Necromantic Thirst definition exists");

    assert_eq!(definition.name, "Necromantic Thirst");
    assert_eq!(
        definition.mana_cost,
        ManaCost::with_colors(2, [Color::Black, Color::Black])
    );
    assert_eq!(definition.colors, [Color::Black].into_iter().collect());
    assert_eq!(
        definition.card_types,
        [CardType::Enchantment].into_iter().collect()
    );
    assert_eq!(
        definition.effects,
        [Effect::AttachSourceAndModifyTargetPt {
            power: 0,
            toughness: 0,
        }]
    );
    assert_eq!(
        definition.supported_rules,
        [
            "aura-static-attachment-only",
            "combat-damage-trigger-not-implemented"
        ]
    );
}

#[test]
fn necromantic_thirst_attaches_without_fabricating_its_unported_combat_trigger() {
    let mut game = Game::new(card_definitions(), 2).expect("RAV fixture builds");
    let thirst = game
        .add_card(PlayerId(0), "RAV-NECROMANTIC-THIRST", Zone::Hand)
        .expect("Aura setup");
    let target = game
        .add_card(PlayerId(0), "RAV-GOLIATH-SPIDER", Zone::Battlefield)
        .expect("target setup");
    game.grant_mana(PlayerId(0), Color::Black, 6)
        .expect("Aura mana");

    game.cast_spell(
        PlayerId(0),
        CastRequest {
            card: thirst,
            targets: vec![Target::Permanent(target)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("Aura casts");
    game.pass_priority(PlayerId(0)).expect("caster passes");
    game.pass_priority(PlayerId(1)).expect("Aura resolves");

    assert_eq!(game.zone_of(thirst), Some(Zone::Battlefield));
    assert_eq!(
        game.object(thirst).expect("Aura remains live").attached_to,
        Some(target)
    );
    let characteristics = game.characteristics(target).expect("target remains live");
    assert_eq!(characteristics.power, Some(7));
    assert_eq!(characteristics.toughness, Some(6));
    game.validate_invariants()
        .expect("zero-modifier Aura attachment remains invariant-safe");
}
