//! Green regression for Grave-Shell Scarab's sacrifice-to-draw activation.

use cardbench_magic_engine::{AbilityActivation, Color, Game, GameEvent, PlayerId, Zone};
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
fn grave_shell_scarab_sacrifices_as_cost_then_draws_on_resolution() {
    let mut game = Game::new_with_all_bindings(
        card_definitions(),
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        rav_activated_ability_bindings(),
    )
    .expect("RAV catalog constructs");
    let scarab = game
        .put_on_battlefield(PlayerId(0), "RAV-GRAVE-SHELL-SCARAB")
        .expect("Scarab begins on the battlefield");
    let swamp = game
        .put_on_battlefield(PlayerId(0), "RAV-SWAMP")
        .expect("Swamp begins on the battlefield");
    let forest = game
        .put_on_battlefield(PlayerId(0), "RAV-FOREST")
        .expect("Forest begins on the battlefield");
    let drawn = game
        .add_card(PlayerId(0), "RAV-PLAINS", Zone::Library)
        .expect("draw card begins in library");
    game.begin_game().expect("fixture begins game");
    game.activate_mana_ability(PlayerId(0), swamp, Color::Black)
        .expect("Swamp pays the black symbol");
    game.activate_mana_ability(PlayerId(0), forest, Color::Green)
        .expect("Forest pays the generic symbol");
    game.activate_ability(
        PlayerId(0),
        AbilityActivation {
            source: scarab,
            ability_id: "sacrifice-source-draw",
            sacrifice_sources: vec![scarab],
            additional_tap_creatures: vec![],
            discard_cards: vec![],
            targets: vec![],
        },
    )
    .expect("Scarab activation enters the stack");

    assert_eq!(game.zone_of(scarab), Some(Zone::Graveyard));
    assert_eq!(game.zone_of(drawn), Some(Zone::Library));
    resolve_top(&mut game);

    println!("Grave-Shell Scarab trace: {:?}", game.event_log);
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&"RAV-GRAVE-SHELL-SCARAB"));
    assert_eq!(game.zone_of(drawn), Some(Zone::Hand));
    let sacrifice = game
        .event_log
        .iter()
        .position(|event| {
            matches!(
                event,
                GameEvent::SacrificedAsAbilityCost { source, permanent, .. }
                    if *source == scarab && *permanent == scarab
            )
        })
        .expect("sacrifice receipt is recorded");
    let activated = game
        .event_log
        .iter()
        .position(|event| {
            matches!(
                event,
                GameEvent::AbilityActivated { source, ability, .. }
                    if *source == scarab && *ability == "sacrifice-source-draw"
            )
        })
        .expect("ability enters the stack");
    assert!(sacrifice < activated, "the source is sacrificed as a cost");
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::AbilityResolved { source, ability }
            if *source == scarab && *ability == "sacrifice-source-draw"
    )));
    game.validate_invariants()
        .expect("sacrifice-and-draw state remains invariant-valid");
}
