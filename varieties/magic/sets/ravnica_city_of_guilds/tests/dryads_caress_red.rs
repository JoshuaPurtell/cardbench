//! Red contract for Dryad's Caress: its graveyard creature count and selected
//! creature-card return must both survive spell targeting and resolution.

use cardbench_magic_engine::{CastRequest, Color, Game, GameEvent, PlayerId, Target, Zone};
use cardbench_magic_rav::{RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions};

#[test]
fn dryads_caress_requires_graveyard_count_and_creature_return_fidelity() {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-DRYADS-CARESS")
        .expect("Dryad's Caress exists");
    assert!(
        definition
            .supported_rules
            .contains(&"graveyard-creature-count-life-gain-and-target-return"),
        "Dryad's Caress must expose both printed graveyard instructions"
    );
    assert!(
        definition.effects.len() >= 2,
        "Dryad's Caress needs a count-based life gain and an exact creature-card return"
    );
    assert!(
        RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&definition.id),
        "Dryad's Caress is not full fidelity until both effects are represented"
    );
}

#[test]
fn dryads_caress_counts_only_controller_creature_cards_then_returns_the_target() {
    let controller = PlayerId(0);
    let mut game = Game::new(card_definitions(), 2).expect("catalog validates");
    let returned = game
        .add_card(controller, "RAV-WATCHWOLF", Zone::Graveyard)
        .expect("returned creature starts in graveyard");
    game.add_card(controller, "RAV-GOLGARI-BROWNSCALE", Zone::Graveyard)
        .expect("second creature starts in graveyard");
    game.add_card(controller, "RAV-CHAR", Zone::Graveyard)
        .expect("noncreature starts in graveyard");
    let caress = game
        .add_card(controller, "RAV-DRYADS-CARESS", Zone::Hand)
        .expect("Caress starts in hand");
    game.grant_mana(controller, Color::Green, 5)
        .expect("spell payment mana");
    game.cast_spell(
        controller,
        CastRequest {
            card: caress,
            targets: vec![Target::Permanent(returned)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("Caress casts with a creature-card target");
    game.pass_priority(PlayerId(0)).expect("caster passes");
    game.pass_priority(PlayerId(1)).expect("spell resolves");

    assert_eq!(game.player(controller).expect("controller").life, 22);
    assert_eq!(game.zone_of(returned), Some(Zone::Hand));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::LifeGained { player, amount: 2 } if *player == controller
    )));
    game.validate_invariants()
        .expect("Dryad's Caress trace is invariant-safe");
}
