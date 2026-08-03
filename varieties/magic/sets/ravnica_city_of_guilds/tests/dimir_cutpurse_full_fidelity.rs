//! Live combat and private-choice contract for Dimir Cutpurse.

use cardbench_magic_engine::{
    DecisionSelection, DecisionVisibility, Game, GameEvent, PlayerId, Step, Zone,
};
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
    .expect("RAV fixture constructs")
}

fn advance_to_attackers(game: &mut Game) {
    game.begin_game().expect("fixture begins game");
    while game.step != Step::DeclareAttackers {
        let player = game.priority;
        game.pass_priority(player)
            .expect("fixture priority advances toward combat");
    }
}

fn pass_pair(game: &mut Game) {
    let first = game.priority;
    game.pass_priority(first).expect("first player passes");
    let second = game.priority;
    game.pass_priority(second).expect("second player passes");
}

#[test]
fn dimir_cutpurse_captures_combat_player_for_private_discard_then_controller_draw() {
    let controller = PlayerId(0);
    let recipient = PlayerId(1);
    let mut game = game_with_rav_bindings();
    let cutpurse = game
        .put_on_battlefield(controller, "RAV-DIMIR-CUTPURSE")
        .expect("Cutpurse setup");
    game.set_entered_turn_for_setup(cutpurse, 0)
        .expect("fixture makes Cutpurse eligible to attack");
    let discarded = game
        .add_card(recipient, "RAV-WATCHWOLF", Zone::Hand)
        .expect("recipient hand setup");
    let drawn = game
        .add_card(controller, "RAV-GLASS-GOLEM", Zone::Library)
        .expect("controller library setup");

    advance_to_attackers(&mut game);
    game.declare_attackers(controller, &[cutpurse])
        .expect("Cutpurse attacks");
    pass_pair(&mut game);
    assert_eq!(game.step, Step::DeclareBlockers);
    game.declare_blockers(recipient, &[])
        .expect("recipient declares no blockers");
    pass_pair(&mut game);

    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::DamageDealtToPlayer { source, player, amount }
            if *source == cutpurse && *player == recipient && *amount == 2
    )));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::TriggeredAbilityStacked { source, ability, .. }
            if *source == cutpurse && *ability == "combat-player-discard-then-controller-draw"
    )));

    pass_pair(&mut game);
    let decision = game
        .view_for_player(recipient)
        .expect("recipient view")
        .pending_decision
        .expect("combat recipient receives private discard decision");
    assert_eq!(decision.visibility, DecisionVisibility::Private);
    assert!(
        game.view_for_player(controller)
            .expect("controller view")
            .pending_decision
            .is_none(),
        "the trigger controller cannot inspect the combat recipient's hand"
    );
    game.submit_decision(
        recipient,
        decision.id,
        DecisionSelection::Objects(vec![discarded]),
    )
    .expect("recipient selects one current hand card");

    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&"RAV-DIMIR-CUTPURSE"));
    assert_eq!(game.zone_of(discarded), Some(Zone::Graveyard));
    assert_eq!(game.zone_of(drawn), Some(Zone::Hand));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::CardDiscarded { player, card }
            if *player == recipient && *card == discarded
    )));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::AbilityResolved { source, ability, .. }
            if *source == cutpurse && *ability == "combat-player-discard-then-controller-draw"
    )));
    game.validate_invariants()
        .expect("combat player trigger preserves state-machine invariants");
    eprintln!("Dimir Cutpurse trace={:?}", game.canonical_event_log());
}
