//! Red-to-green full-fidelity contract for Stinkweed Imp.
//!
//! This records only CardBench-authored operations: a combat-damage recipient
//! is captured as trigger provenance, then destroyed by the resulting stack
//! ability.  It deliberately avoids reproducing upstream card text.

use cardbench_magic_engine::{
    CombatBlock, Effect, Game, GameEvent, ManaCost, PlayerId, Step, TriggerCondition, Zone,
};
use cardbench_magic_rav::{
    RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, rav_activated_ability_bindings,
    rav_additional_spell_cost_bindings, rav_basic_land_type_bindings, rav_mana_ability_bindings,
    rav_triggered_ability_bindings,
};

fn advance_to_declare_attackers(game: &mut Game) {
    game.begin_game().expect("fixture starts game");
    for _ in 0..4 {
        game.pass_priority(PlayerId(0))
            .expect("active player passes toward combat");
        game.pass_priority(PlayerId(1))
            .expect("opponent passes toward combat");
    }
    assert_eq!(game.step, Step::DeclareAttackers);
}

#[test]
fn stinkweed_imp_combat_damage_trigger_is_ability_complete() {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-STINKWEED-IMP")
        .expect("Stinkweed Imp definition exists");
    assert!(
        RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&definition.id),
        "Stinkweed Imp cannot be positive-manifest while its combat-damage trigger is absent"
    );
    assert_eq!(
        definition.supported_rules,
        [
            "full-rules-fidelity",
            "dredge",
            "base-characteristics",
            "flying",
            "combat-damage-destroy-recipient",
        ]
    );

    let binding = rav_triggered_ability_bindings()
        .into_iter()
        .find(|binding| binding.card_definition == definition.id)
        .expect("Stinkweed Imp combat trigger binding exists");
    assert_eq!(
        binding.ability.condition,
        TriggerCondition::DealsCombatDamageToCreature
    );
    assert_eq!(binding.ability.mana_cost, ManaCost::new(0));
    assert!(!binding.ability.optional);
    assert!(binding.ability.targets.is_empty());
    assert_eq!(
        binding.ability.effects,
        [Effect::DestroyCombatDamagedCreature]
    );

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
    let imp = game
        .add_card(PlayerId(0), "RAV-STINKWEED-IMP", Zone::Battlefield)
        .expect("Imp begins on battlefield");
    let recipient = game
        .add_card(PlayerId(1), "RAV-COURIER-HAWK", Zone::Battlefield)
        .expect("Flying recipient begins on battlefield");
    let unrelated = game
        .add_card(PlayerId(1), "RAV-WATCHWOLF", Zone::Battlefield)
        .expect("unrelated creature begins on battlefield");
    game.set_entered_turn_for_setup(imp, 0)
        .expect("Imp predates combat");
    game.set_entered_turn_for_setup(recipient, 0)
        .expect("recipient predates combat");
    game.set_entered_turn_for_setup(unrelated, 0)
        .expect("unrelated creature predates combat");
    advance_to_declare_attackers(&mut game);
    game.declare_attackers(PlayerId(0), &[imp])
        .expect("Imp attacks");
    game.pass_priority(PlayerId(0)).expect("pass attackers");
    game.pass_priority(PlayerId(1)).expect("pass attackers");
    game.declare_blockers(
        PlayerId(1),
        &[CombatBlock {
            attacker: imp,
            blocker: recipient,
        }],
    )
    .expect("recipient blocks");
    game.pass_priority(PlayerId(0)).expect("pass blockers");
    game.pass_priority(PlayerId(1))
        .expect("combat damage resolves and trigger stacks");
    assert_eq!(game.zone_of(unrelated), Some(Zone::Battlefield));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::TriggeredAbilityStacked { source, ability, .. }
            if *source == imp && *ability == "destroy-combat-damaged-creature"
    )));
    game.pass_priority(PlayerId(0)).expect("pass trigger");
    game.pass_priority(PlayerId(1)).expect("resolve trigger");
    println!(
        "Stinkweed Imp full-fidelity trace: {:?}",
        game.canonical_event_log()
    );
    assert_eq!(game.zone_of(recipient), Some(Zone::Graveyard));
    assert_eq!(game.zone_of(unrelated), Some(Zone::Battlefield));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::CardDestroyed { source, card } if *source == imp && *card == recipient
    )));
    game.validate_invariants()
        .expect("Stinkweed Imp combat trigger preserves invariants");
}
