//! Red regression for Divebomber Griffin's sacrifice-damage activation.

use cardbench_magic_engine::{AbilityActivation, Game, GameEvent, PlayerId, Step, Target, Zone};
use cardbench_magic_rav::{
    RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, rav_activated_ability_bindings,
    rav_additional_spell_cost_bindings, rav_basic_land_type_bindings, rav_mana_ability_bindings,
};

#[test]
fn divebomber_griffin_requires_its_sacrifice_damage_activation_for_full_fidelity() {
    let griffin = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-DIVEBOMBER-GRIFFIN")
        .expect("Divebomber Griffin definition exists");
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&griffin.id));
}

#[test]
fn divebomber_griffin_sacrifices_to_damage_a_declared_attacker() {
    let mut game = Game::new_with_all_bindings(
        card_definitions(),
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        rav_activated_ability_bindings(),
    )
    .expect("RAV game builds");
    let attacker = game
        .put_on_battlefield(PlayerId(0), "RAV-WATCHWOLF")
        .expect("attacker enters");
    let griffin = game
        .put_on_battlefield(PlayerId(1), "RAV-DIVEBOMBER-GRIFFIN")
        .expect("Divebomber Griffin enters");
    for card in [attacker, griffin] {
        game.set_entered_turn_for_setup(card, 0)
            .expect("fixture creature predates measured turn");
    }
    game.begin_game().expect("game starts");
    while game.step != Step::DeclareAttackers {
        let priority = game.priority;
        game.pass_priority(priority).expect("advance to combat");
    }
    game.declare_attackers(PlayerId(0), &[attacker])
        .expect("attacker is declared");
    game.pass_priority(PlayerId(0))
        .expect("attacker passes priority");
    game.activate_ability(
        PlayerId(1),
        AbilityActivation {
            source: griffin,
            ability_id: "sacrifice-deal-three-to-attacker-or-blocker",
            sacrifice_sources: vec![griffin],
            additional_tap_creatures: vec![],
            discard_cards: vec![],
            targets: vec![Target::Permanent(attacker)],
        },
    )
    .expect("Divebomber activation stacks");
    assert_eq!(game.zone_of(griffin), Some(Zone::Graveyard));
    assert_eq!(game.object(attacker).expect("attacker remains").damage, 0);
    game.pass_priority(PlayerId(1))
        .expect("controller passes ability");
    game.pass_priority(PlayerId(0)).expect("ability resolves");
    println!("Divebomber Griffin trace: {:?}", game.canonical_event_log());
    assert_eq!(game.object(attacker).expect("attacker survives").damage, 3);
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::AbilityResolved { source, ability }
            if *source == griffin && *ability == "sacrifice-deal-three-to-attacker-or-blocker"
    )));
    game.validate_invariants()
        .expect("Divebomber activation trace preserves invariants");
}
