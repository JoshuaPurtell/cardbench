//! Green regression for Votary of the Conclave's targeted regeneration.

use cardbench_magic_engine::{
    AbilityActivation, CastRequest, Color, Game, GameEvent, PlayerId, Target, Zone,
};
use cardbench_magic_rav::{
    RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, rav_activated_ability_bindings,
    rav_additional_spell_cost_bindings, rav_basic_land_type_bindings, rav_mana_ability_bindings,
};

fn resolve_top(game: &mut Game) {
    let first = game.priority;
    game.pass_priority(first).expect("first priority pass");
    let second = game.priority;
    game.pass_priority(second)
        .expect("second priority pass resolves");
}

#[test]
fn votary_regenerates_an_opponents_artifact_creature_from_smash() {
    let mut game = Game::new_with_all_bindings(
        card_definitions(),
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        rav_activated_ability_bindings(),
    )
    .expect("RAV catalog constructs");
    let votary = game
        .put_on_battlefield(PlayerId(0), "RAV-VOTARY-OF-THE-CONCLAVE")
        .expect("Votary begins on the battlefield");
    let glass_golem = game
        .put_on_battlefield(PlayerId(1), "RAV-GLASS-GOLEM")
        .expect("opponent Glass Golem begins on the battlefield");
    let forest = game
        .put_on_battlefield(PlayerId(0), "RAV-FOREST")
        .expect("Forest begins on the battlefield");
    let mountains = (0..3)
        .map(|_| {
            game.put_on_battlefield(PlayerId(0), "RAV-MOUNTAIN")
                .expect("Mountain begins on the battlefield")
        })
        .collect::<Vec<_>>();
    let smash = game
        .add_card(PlayerId(0), "RAV-SMASH", Zone::Hand)
        .expect("Smash begins in hand");
    let drawn = game
        .add_card(PlayerId(0), "RAV-PLAINS", Zone::Library)
        .expect("draw card begins in library");
    game.begin_game().expect("fixture begins game");

    game.activate_mana_ability(PlayerId(0), forest, Color::Green)
        .expect("Forest pays regeneration activation");
    game.activate_ability(
        PlayerId(0),
        AbilityActivation {
            source: votary,
            ability_id: "regenerate-target-creature",
            sacrifice_sources: vec![],
            additional_tap_creatures: vec![],
            discard_cards: vec![],
            targets: vec![Target::Permanent(glass_golem)],
        },
    )
    .expect("Votary can target any creature, including an opponent's");
    resolve_top(&mut game);
    for mountain in mountains {
        game.activate_mana_ability(PlayerId(0), mountain, Color::Red)
            .expect("Mountain produces Smash mana");
    }
    game.cast_spell(
        PlayerId(0),
        CastRequest {
            card: smash,
            targets: vec![Target::Permanent(glass_golem)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("Smash targets the artifact creature");
    resolve_top(&mut game);

    println!("Votary regeneration trace: {:?}", game.event_log);
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&"RAV-VOTARY-OF-THE-CONCLAVE"));
    assert_eq!(game.zone_of(glass_golem), Some(Zone::Battlefield));
    assert!(game.object(glass_golem).expect("Golem remains").tapped);
    assert_eq!(game.zone_of(drawn), Some(Zone::Hand));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::RegenerationShieldCreated { source, target }
            if *source == votary && *target == glass_golem
    )));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::RegenerationShieldUsed { source, target }
            if *source == votary && *target == glass_golem
    )));
    assert!(!game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::CardDestroyed { card, .. } if *card == glass_golem
    )));
    game.validate_invariants()
        .expect("targeted regeneration state remains invariant-valid");
}
