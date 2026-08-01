//! Red regression for Vindictive Mob's enter-the-battlefield sacrifice trigger.

use cardbench_magic_engine::{CastRequest, Color, Game, PlayerId, Zone};
use cardbench_magic_rav::{
    RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, rav_activated_ability_bindings,
    rav_additional_spell_cost_bindings, rav_basic_land_type_bindings, rav_mana_ability_bindings,
    rav_triggered_ability_bindings,
};

fn game_with_rav_bindings() -> Game {
    Game::new_with_all_bindings_and_triggers(
        card_definitions(),
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        rav_activated_ability_bindings(),
        rav_triggered_ability_bindings(),
    )
    .expect("RAV game builds")
}

fn advance_to_main(game: &mut Game) {
    for _ in 0..2 {
        game.pass_priority(PlayerId(0)).expect("active passes");
        game.pass_priority(PlayerId(1)).expect("opponent passes");
    }
}

#[test]
fn vindictive_mob_etb_stacks_a_controlled_creature_sacrifice() {
    let mut game = game_with_rav_bindings();
    let mob = game
        .add_card(PlayerId(0), "RAV-VINDICTIVE-MOB", Zone::Hand)
        .expect("Vindictive Mob is executable");
    let sacrifice = game
        .put_on_battlefield(PlayerId(0), "RAV-WATCHWOLF")
        .expect("controlled creature exists");
    let swamps = (0..6)
        .map(|_| {
            game.put_on_battlefield(PlayerId(0), "RAV-SWAMP")
                .expect("Swamp enters before the game")
        })
        .collect::<Vec<_>>();
    game.begin_game().expect("fixture starts");
    advance_to_main(&mut game);
    for swamp in swamps {
        game.activate_mana_ability(PlayerId(0), swamp, Color::Black)
            .expect("black mana");
    }
    game.cast_spell(
        PlayerId(0),
        CastRequest {
            card: mob,
            targets: vec![],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("Mob casts");
    game.pass_priority(PlayerId(0))
        .expect("caster passes spell");
    game.pass_priority(PlayerId(1))
        .expect("opponent passes spell");

    println!(
        "Vindictive Mob red trace: {:#?}",
        game.canonical_event_log()
    );
    assert_eq!(game.stack.len(), 1, "ETB sacrifice trigger is stacked");
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&"RAV-VINDICTIVE-MOB"));
    assert_eq!(game.zone_of(sacrifice), Some(Zone::Battlefield));
    game.validate_invariants()
        .expect("pending Mob trigger is valid");
}
