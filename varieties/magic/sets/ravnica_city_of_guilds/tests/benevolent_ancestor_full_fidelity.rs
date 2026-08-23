//! Green full-fidelity contract for Benevolent Ancestor's creature target path.

use cardbench_magic_engine::{AbilityActivation, Game, GameEvent, PlayerId, Target};
use cardbench_magic_rav::{
    card_definitions, rav_activated_ability_bindings, rav_additional_spell_cost_bindings,
    rav_basic_land_type_bindings, rav_mana_ability_bindings,
};

fn pass_pair(game: &mut Game) {
    let first = game.priority;
    game.pass_priority(first).expect("first priority pass");
    let second = game.priority;
    game.pass_priority(second).expect("second priority pass");
}

#[test]
fn benevolent_ancestor_can_shield_a_creature_target_without_losing_defender() {
    let mut game = Game::new_with_all_bindings(
        card_definitions(),
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        rav_activated_ability_bindings(),
    )
    .expect("RAV fixture constructs");
    let ancestor = game
        .put_on_battlefield(PlayerId(0), "RAV-BENEVOLENT-ANCESTOR")
        .expect("Ancestor setup");
    let target = game
        .put_on_battlefield(PlayerId(1), "RAV-WATCHWOLF")
        .expect("creature target setup");
    game.set_entered_turn_for_setup(ancestor, 0)
        .expect("fixture makes Ancestor long-controlled");
    game.begin_game().expect("fixture begins game");

    game.activate_ability(
        PlayerId(0),
        AbilityActivation {
            source: ancestor,
            ability_id: "tap-prevent-one-damage",
            sacrifice_sources: vec![],
            additional_tap_creatures: vec![],
            discard_cards: vec![],
            targets: vec![Target::Permanent(target)],
        },
    )
    .expect("Ancestor may choose a creature target");
    pass_pair(&mut game);

    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::DamageShieldCreated {
            source,
            target: Target::Permanent(shielded),
            amount: 1,
        } if *source == ancestor && *shielded == target
    )));
    game.validate_invariants()
        .expect("creature-target prevention keeps the shield lifecycle valid");
    eprintln!(
        "Benevolent Ancestor creature trace={:?}",
        game.canonical_event_log()
    );
}
