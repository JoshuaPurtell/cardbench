//! Red-to-green contract for Psychic Drain's chosen-X mill and life gain.
//!
//! The ordinary cast path already preserves a policy-declared X value and a
//! targeted player through the stack.  This regression keeps the two derived
//! resolution quantities coupled: the target mills exactly X cards and the
//! resolving controller gains exactly that same X life.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardType, CastRequest, Color, Game, GameEvent, ManaCost, ManaPaymentSelection, PlayerId,
    Target, Zone,
};
use cardbench_magic_rav::{
    RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, executable_definition_id_for_collector,
};

#[test]
fn psychic_drain_has_an_exact_chosen_x_targeted_mill_and_life_definition() {
    assert_eq!(
        executable_definition_id_for_collector(220),
        Ok("RAV-PSYCHIC-DRAIN")
    );
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-PSYCHIC-DRAIN")
        .expect("Psychic Drain definition exists");

    assert_eq!(definition.name, "Psychic Drain");
    assert_eq!(
        definition.mana_cost,
        ManaCost::with_colors(0, [Color::Blue, Color::Black])
    );
    assert_eq!(
        definition.colors,
        BTreeSet::from([Color::Blue, Color::Black])
    );
    assert_eq!(definition.card_types, BTreeSet::from([CardType::Sorcery]));
    assert_eq!((definition.power, definition.toughness), (None, None));
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&definition.id));
    assert!(
        definition
            .supported_rules
            .contains(&"chosen-x-targeted-mill-and-controller-life-gain")
    );
}

#[test]
fn psychic_drain_couples_the_declared_x_for_target_mill_and_controller_life_gain() {
    let mut game = Game::new(card_definitions(), 2).expect("RAV fixture builds");
    let drain = game
        .add_card(PlayerId(0), "RAV-PSYCHIC-DRAIN", Zone::Hand)
        .expect("Psychic Drain setup");
    let milled = [
        game.add_card(PlayerId(1), "RAV-WATCHWOLF", Zone::Library)
            .expect("first target-library card"),
        game.add_card(PlayerId(1), "RAV-GOLIATH-SPIDER", Zone::Library)
            .expect("second target-library card"),
    ];
    let unmilled = game
        .add_card(PlayerId(1), "RAV-BOROS-RECRUIT", Zone::Library)
        .expect("unmilled target-library card");
    game.grant_mana(PlayerId(0), Color::Blue, 2)
        .expect("blue payment setup");
    game.grant_mana(PlayerId(0), Color::Black, 2)
        .expect("black payment setup");

    game.cast_spell_with_x(
        PlayerId(0),
        CastRequest {
            card: drain,
            targets: vec![Target::Player(PlayerId(1))],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
        2,
        ManaPaymentSelection {
            generic: vec![Color::Blue, Color::Black],
            hybrid: vec![],
        },
    )
    .expect("Psychic Drain X=2 casts");
    game.pass_priority(PlayerId(0)).expect("caster passes");
    game.pass_priority(PlayerId(1)).expect("spell resolves");

    assert_eq!(game.players[0].life, 22);
    assert_eq!(game.zone_of(milled[1]), Some(Zone::Graveyard));
    assert_eq!(game.zone_of(milled[0]), Some(Zone::Graveyard));
    assert_eq!(game.zone_of(unmilled), Some(Zone::Library));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::LifeGained { player, amount } if *player == PlayerId(0) && *amount == 2
    )));
    println!("Psychic Drain trace={:?}", game.canonical_event_log());
    game.validate_invariants()
        .expect("Psychic Drain chosen-X coupling preserves invariants");
}
