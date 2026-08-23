//! Green regression for Sewerdreg's stack-backed regeneration shield.

use cardbench_magic_engine::{
    AbilityActivation, CastRequest, Color, Game, GameEvent, PlayerId, Target, Zone,
};
use cardbench_magic_rav::{
    card_definitions, rav_activated_ability_bindings, rav_additional_spell_cost_bindings,
    rav_basic_land_type_bindings, rav_mana_ability_bindings,
};

fn resolve_top(game: &mut Game) {
    let first = game.priority;
    game.pass_priority(first).expect("first priority pass");
    let second = game.priority;
    game.pass_priority(second)
        .expect("second priority pass resolves");
}

#[test]
fn sewerdreg_shield_replaces_lethal_damage_and_keeps_event_provenance() {
    let mut game = Game::new_with_all_bindings(
        card_definitions(),
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        rav_activated_ability_bindings(),
    )
    .expect("RAV catalog constructs");
    let sewerdreg = game
        .put_on_battlefield(PlayerId(0), "RAV-SEWERDREG")
        .expect("Sewerdreg begins on the battlefield");
    let swamp = game
        .put_on_battlefield(PlayerId(0), "RAV-SWAMP")
        .expect("Swamp begins on the battlefield");
    let mountains = (0..3)
        .map(|_| {
            game.put_on_battlefield(PlayerId(0), "RAV-MOUNTAIN")
                .expect("Mountain begins on the battlefield")
        })
        .collect::<Vec<_>>();
    let char = game
        .add_card(PlayerId(0), "RAV-CHAR", Zone::Hand)
        .expect("Char begins in hand");
    game.begin_game().expect("fixture begins game");

    game.activate_mana_ability(PlayerId(0), swamp, Color::Black)
        .expect("Swamp pays regeneration activation");
    game.activate_ability(
        PlayerId(0),
        AbilityActivation {
            source: sewerdreg,
            ability_id: "self-regeneration",
            sacrifice_sources: vec![],
            additional_tap_creatures: vec![],
            discard_cards: vec![],
            targets: vec![],
        },
    )
    .expect("Sewerdreg regeneration activates");
    resolve_top(&mut game);

    for mountain in mountains {
        game.activate_mana_ability(PlayerId(0), mountain, Color::Red)
            .expect("Mountain produces Char mana");
    }
    game.cast_spell(
        PlayerId(0),
        CastRequest {
            card: char,
            targets: vec![Target::Permanent(sewerdreg)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("Char targets Sewerdreg");
    resolve_top(&mut game);

    println!("Sewerdreg regeneration trace: {:?}", game.event_log);
    assert_eq!(game.zone_of(sewerdreg), Some(Zone::Battlefield));
    let object = game.object(sewerdreg).expect("Sewerdreg remains an object");
    assert!(object.tapped, "regeneration taps the creature");
    assert_eq!(object.damage, 0, "regeneration clears marked damage");
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::RegenerationShieldCreated { source, target }
            if *source == sewerdreg && *target == sewerdreg
    )));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::RegenerationShieldUsed { source, target }
            if *source == sewerdreg && *target == sewerdreg
    )));
    assert!(!game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::CardMoved { card, to: Zone::Graveyard } if *card == sewerdreg
    )));
    game.validate_invariants()
        .expect("regeneration state remains invariant-valid");
}
