//! Red regression for Dromad Purebred's received-damage life-gain trigger.

use cardbench_magic_engine::{AbilityActivation, Game, GameEvent, PlayerId, Target};
use cardbench_magic_rav::{
    RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, rav_activated_ability_bindings,
    rav_additional_spell_cost_bindings, rav_basic_land_type_bindings, rav_mana_ability_bindings,
    rav_triggered_ability_bindings,
};

#[test]
fn dromad_purebred_requires_its_received_damage_trigger_for_full_fidelity() {
    let dromad = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-DROMAD-PUREBRED")
        .expect("Dromad Purebred definition exists");
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&dromad.id));
    assert!(
        dromad
            .supported_rules
            .contains(&"damage-received-life-gain")
    );
}

#[test]
fn dromad_purebred_gains_one_life_after_receiving_damage() {
    let mut game = Game::new_with_all_bindings_and_triggers(
        card_definitions(),
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        rav_activated_ability_bindings(),
        rav_triggered_ability_bindings(),
    )
    .expect("RAV game builds");
    let dromad = game
        .put_on_battlefield(PlayerId(0), "RAV-DROMAD-PUREBRED")
        .expect("Dromad enters");
    let fangtail = game
        .put_on_battlefield(PlayerId(1), "RAV-VIASHINO-FANGTAIL")
        .expect("Fangtail enters");
    game.set_entered_turn_for_setup(fangtail, 0)
        .expect("fixture source predates measured turn");
    game.begin_game().expect("game starts");
    game.pass_priority(PlayerId(0))
        .expect("opponent receives priority");
    game.activate_ability(
        PlayerId(1),
        AbilityActivation {
            source: fangtail,
            ability_id: "tap-deal-one-to-player-or-creature",
            sacrifice_sources: vec![],
            additional_tap_creatures: vec![],
            discard_cards: vec![],
            targets: vec![Target::Permanent(dromad)],
        },
    )
    .expect("Fangtail targets Dromad");
    game.pass_priority(PlayerId(1))
        .expect("Fangtail controller passes");
    game.pass_priority(PlayerId(0))
        .expect("Fangtail ability resolves");

    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::TriggeredAbilityStacked { source, ability, .. }
            if *source == dromad && *ability == "damage-gain-one-life"
    )));
    assert_eq!(game.player(PlayerId(0)).expect("player exists").life, 20);
    game.pass_priority(PlayerId(0))
        .expect("Dromad controller passes trigger");
    game.pass_priority(PlayerId(1)).expect("trigger resolves");
    println!("Dromad Purebred trace: {:?}", game.canonical_event_log());
    assert_eq!(game.player(PlayerId(0)).expect("player exists").life, 21);
    game.validate_invariants()
        .expect("Dromad trigger trace preserves invariants");
}
