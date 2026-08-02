//! Red discovery contract for Conclave's Blessing's dynamic attached modifier.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardType, CastRequest, Color, Game, GameEvent, Keyword, ManaCost, PlayerId, Target, Zone,
};
use cardbench_magic_rav::{RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions};

fn request(
    card: cardbench_magic_engine::ObjectId,
    target: Option<cardbench_magic_engine::ObjectId>,
) -> CastRequest {
    CastRequest {
        card,
        targets: target.into_iter().map(Target::Permanent).collect(),
        convoke: vec![],
        payment_mana_abilities: vec![],
    }
}

fn pass_pair(game: &mut Game, first: PlayerId, second: PlayerId) {
    game.pass_priority(first).expect("first player passes");
    game.pass_priority(second).expect("second player passes");
}

#[test]
fn conclaves_blessing_has_its_exact_convoke_dynamic_aura_contract() {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-CONCLAVES-BLESSING")
        .expect("Conclave's Blessing definition exists");
    assert_eq!(definition.name, "Conclave's Blessing");
    assert_eq!(
        definition.mana_cost,
        ManaCost::with_colors(3, [Color::White])
    );
    assert_eq!(definition.colors, BTreeSet::from([Color::White]));
    assert_eq!(
        definition.card_types,
        BTreeSet::from([CardType::Enchantment])
    );
    assert!(definition.keywords.contains(&Keyword::Convoke));
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&definition.id));
    assert!(
        definition
            .supported_rules
            .contains(&"aura-convoke-dynamic-other-controller-creature-toughness")
    );
}

#[test]
fn conclaves_blessing_recalculates_from_the_enchanted_creatures_controller() {
    let mut game = Game::new(card_definitions(), 2).expect("RAV fixture builds");
    let target = game
        .add_card(PlayerId(0), "RAV-WATCHWOLF", Zone::Battlefield)
        .expect("enchanted creature setup");
    let first_other = game
        .add_card(PlayerId(0), "RAV-CAREGIVER", Zone::Battlefield)
        .expect("first other creature setup");
    let second_other = game
        .add_card(PlayerId(0), "RAV-CAREGIVER", Zone::Battlefield)
        .expect("second other creature setup");
    let blessing = game
        .add_card(PlayerId(0), "RAV-CONCLAVES-BLESSING", Zone::Hand)
        .expect("Conclave's Blessing setup");
    let last_gasp = game
        .add_card(PlayerId(1), "RAV-LAST-GASP", Zone::Hand)
        .expect("opponent removal setup");
    game.grant_mana(PlayerId(0), Color::White, 4)
        .expect("Conclave's Blessing payment exists");
    game.grant_mana(PlayerId(1), Color::Black, 1)
        .expect("Last Gasp payment exists");

    game.cast_spell(PlayerId(0), request(blessing, Some(target)))
        .expect("Conclave's Blessing casts");
    pass_pair(&mut game, PlayerId(0), PlayerId(1));
    assert_eq!(
        game.characteristics(target)
            .expect("enchanted creature characteristics")
            .toughness,
        Some(7),
        "two other controller creatures contribute +0/+4",
    );
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::ContinuousEffectCreated { source, target: effect_target, .. }
            if *source == blessing && *effect_target == target
    )));

    game.pass_priority(PlayerId(0))
        .expect("opponent receives priority");
    game.cast_spell(PlayerId(1), request(last_gasp, Some(first_other)))
        .expect("Last Gasp casts");
    pass_pair(&mut game, PlayerId(1), PlayerId(0));
    assert_eq!(game.zone_of(first_other), Some(Zone::Graveyard));
    assert_eq!(
        game.characteristics(target)
            .expect("dynamic modifier recalculates")
            .toughness,
        Some(5),
        "one remaining other controller creature contributes +0/+2",
    );
    assert_eq!(game.zone_of(second_other), Some(Zone::Battlefield));
    game.validate_invariants()
        .expect("dynamic attached modifier preserves invariant state");
}
