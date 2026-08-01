//! Red regression for the source-only Strands of Undeath Aura chassis.

use cardbench_magic_engine::{
    CardType, CastRequest, Color, Effect, Game, ManaCost, PlayerId, Target, Zone,
};
use cardbench_magic_rav::card_definitions;

#[test]
fn strands_of_undeath_has_its_exact_static_aura_chassis_without_unimplemented_abilities() {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-STRANDS-OF-UNDEATH")
        .expect("Strands of Undeath definition exists");

    assert_eq!(definition.name, "Strands of Undeath");
    assert_eq!(
        definition.mana_cost,
        ManaCost::with_colors(3, [Color::Black])
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
            "etb-discard-and-enchanted-regeneration-not-implemented",
        ]
    );
}

#[test]
fn strands_of_undeath_attaches_without_fabricating_its_unported_trigger_or_activation() {
    let mut game = Game::new(card_definitions(), 2).expect("RAV fixture builds");
    let strands = game
        .add_card(PlayerId(0), "RAV-STRANDS-OF-UNDEATH", Zone::Hand)
        .expect("Aura setup");
    let target = game
        .add_card(PlayerId(0), "RAV-GOLIATH-SPIDER", Zone::Battlefield)
        .expect("target setup");
    game.grant_mana(PlayerId(0), Color::Black, 5)
        .expect("Aura mana");

    game.cast_spell(
        PlayerId(0),
        CastRequest {
            card: strands,
            targets: vec![Target::Permanent(target)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("Aura casts");
    game.pass_priority(PlayerId(0)).expect("caster passes");
    game.pass_priority(PlayerId(1)).expect("Aura resolves");

    assert_eq!(game.zone_of(strands), Some(Zone::Battlefield));
    assert_eq!(
        game.object(strands).expect("Aura remains live").attached_to,
        Some(target)
    );
    game.validate_invariants()
        .expect("zero-modifier Aura attachment remains invariant-safe");
}
