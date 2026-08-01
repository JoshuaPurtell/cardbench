//! Red regression for Sadistic Augermage's creature-death discard trigger.

use cardbench_magic_engine::{Game, PlayerId};
use cardbench_magic_rav::{
    card_definitions, rav_activated_ability_bindings, rav_additional_spell_cost_bindings,
    rav_basic_land_type_bindings, rav_mana_ability_bindings, rav_triggered_ability_bindings,
};

#[test]
fn sadistic_augermage_queues_after_another_creature_dies() {
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
    let augermage = game
        .put_on_battlefield(PlayerId(0), "RAV-SADISTIC-AUGERMAGE")
        .expect("Sadistic Augermage definition exists");
    game.set_entered_turn_for_setup(augermage, 0)
        .expect("Augermage predates measured turn");
    // The 0/0 creature dies during the first Untap SBA, providing an
    // auditable death boundary without copying printed card text.
    game.put_on_battlefield(PlayerId(1), "RAV-GOLGARI-GRAVE-TROLL")
        .expect("SBA victim enters");

    game.begin_game().expect("fixture starts");
    eprintln!(
        "Sadistic Augermage death events: {:?}; stack={}",
        game.canonical_event_log(),
        game.stack.len()
    );
    assert_eq!(game.stack.len(), 1, "death trigger is queued");
    let definition = cardbench_magic_rav::card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-SADISTIC-AUGERMAGE")
        .expect("definition remains catalogued");
    assert_eq!(definition.supported_rules[0], "full-rules-fidelity");
    game.validate_invariants()
        .expect("trigger boundary is valid");
}
