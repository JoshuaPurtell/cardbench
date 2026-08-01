//! Green regression for Undercity Shade's stack-backed self-pump.

use cardbench_magic_engine::{AbilityActivation, Color, Game, GameEvent, ManaCost, PlayerId};
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
fn undercity_shade_retains_its_cast_cost_and_pumps_on_stack_resolution() {
    let mut game = Game::new_with_all_bindings(
        card_definitions(),
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        rav_activated_ability_bindings(),
    )
    .expect("RAV catalog constructs");
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-UNDERCITY-SHADE")
        .expect("Undercity Shade definition exists");
    assert_eq!(
        definition.mana_cost,
        ManaCost::with_colors(4, [Color::Black])
    );
    let shade = game
        .put_on_battlefield(PlayerId(0), "RAV-UNDERCITY-SHADE")
        .expect("Shade begins on the battlefield");
    let swamp = game
        .put_on_battlefield(PlayerId(0), "RAV-SWAMP")
        .expect("Swamp begins on the battlefield");
    game.begin_game().expect("fixture begins game");
    game.activate_mana_ability(PlayerId(0), swamp, Color::Black)
        .expect("Swamp pays the pump activation");
    game.activate_ability(
        PlayerId(0),
        AbilityActivation {
            source: shade,
            ability_id: "pump-plus-one-plus-one",
            sacrifice_sources: vec![],
            additional_tap_creatures: vec![],
            discard_cards: vec![],
            targets: vec![],
        },
    )
    .expect("Shade pump enters the stack");
    resolve_top(&mut game);

    println!("Undercity Shade trace: {:?}", game.event_log);
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&"RAV-UNDERCITY-SHADE"));
    let characteristics = game.characteristics(shade).expect("Shade is live");
    assert_eq!(
        (characteristics.power, characteristics.toughness),
        (Some(2), Some(2))
    );
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::AbilityManaPaid { source, ability, mana_cost, .. }
            if *source == shade
                && *ability == "pump-plus-one-plus-one"
                && *mana_cost == ManaCost::with_colors(0, [Color::Black])
    )));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::AbilityResolved { source, ability }
            if *source == shade && *ability == "pump-plus-one-plus-one"
    )));
    game.validate_invariants()
        .expect("self-pump state remains invariant-valid");
}
