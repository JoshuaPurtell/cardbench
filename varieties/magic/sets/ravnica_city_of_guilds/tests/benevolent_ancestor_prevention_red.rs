//! Red discovery regression for Benevolent Ancestor's prevention activation.

use cardbench_magic_engine::{AbilityActivation, Game, GameEvent, PlayerId, Target};
use cardbench_magic_rav::{
    RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, rav_activated_ability_bindings,
    rav_additional_spell_cost_bindings, rav_basic_land_type_bindings, rav_mana_ability_bindings,
};

fn pass_pair(game: &mut Game) {
    let first = game.priority;
    game.pass_priority(first).expect("first priority pass");
    let second = game.priority;
    game.pass_priority(second).expect("second priority pass");
}

#[test]
fn benevolent_ancestor_taps_to_install_one_targeted_prevention_shield() {
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
        .expect("Benevolent Ancestor setup");
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
            targets: vec![Target::Player(PlayerId(1))],
        },
    )
    .expect("Benevolent Ancestor activation is a legal tapped stack ability");
    assert!(game.object(ancestor).expect("source remains live").tapped);
    assert!(matches!(
        game.stack.last().and_then(|entry| entry.ability_id),
        Some("tap-prevent-one-damage")
    ));

    pass_pair(&mut game);
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::DamageShieldCreated {
            source,
            target: Target::Player(PlayerId(1)),
            amount: 1,
        } if *source == ancestor
    )));
    assert!(
        RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&"RAV-BENEVOLENT-ANCESTOR"),
        "the Defender and exact targeted prevention activation are both required for fidelity"
    );
    game.validate_invariants()
        .expect("Benevolent Ancestor prevention preserves state-machine invariants");
    eprintln!("Benevolent Ancestor trace={:?}", game.canonical_event_log());
}
