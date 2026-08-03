//! Red-to-green contract for Mindleech Mass's captured combat-discard trigger.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardType, Color, DecisionSelection, DecisionVisibility, Game, GameEvent, Keyword, ManaCost,
    PlayerId, Step, Zone,
};
use cardbench_magic_rav::{
    RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, executable_definition_id_for_collector,
    rav_activated_ability_bindings, rav_additional_spell_cost_bindings, rav_attachment_bindings,
    rav_basic_land_type_bindings, rav_mana_ability_bindings, rav_triggered_ability_bindings,
};

fn game_with_rav_triggers() -> Game {
    let mut game = Game::new_with_all_bindings_and_triggers(
        card_definitions(),
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        rav_activated_ability_bindings(),
        rav_triggered_ability_bindings(),
    )
    .expect("RAV trigger fixture builds");
    game.register_attachment_bindings(rav_attachment_bindings())
        .expect("RAV attachment bindings register");
    game
}

fn pass_pair(game: &mut Game) {
    let first = game.priority;
    game.pass_priority(first).expect("first priority pass");
    let second = game.priority;
    game.pass_priority(second).expect("second priority pass");
}

fn advance_to_attackers(game: &mut Game) {
    game.begin_game().expect("game begins");
    for _ in 0..16 {
        if game.step == Step::DeclareAttackers {
            return;
        }
        let player = game.priority;
        game.pass_priority(player)
            .expect("priority advances toward attackers");
    }
    panic!("fixture did not reach declare attackers");
}

#[test]
fn mindleech_mass_requires_exact_combat_player_discard_definition() {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-MINDLEECH-MASS")
        .expect("Mindleech Mass definition exists");

    assert_eq!(
        executable_definition_id_for_collector(215),
        Ok("RAV-MINDLEECH-MASS")
    );
    assert_eq!(definition.name, "Mindleech Mass");
    assert_eq!(
        definition.mana_cost,
        ManaCost::with_colors(5, [Color::Blue, Color::Black])
    );
    assert_eq!(definition.colors, BTreeSet::from([Color::Blue, Color::Black]));
    assert_eq!(definition.card_types, BTreeSet::from([CardType::Creature]));
    assert_eq!((definition.power, definition.toughness), (Some(6), Some(6)));
    assert_eq!(definition.keywords, vec![Keyword::Trample]);
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&definition.id));
    assert!(
        definition
            .supported_rules
            .contains(&"combat-player-trigger-recipient-private-three-card-discard")
    );
    assert!(rav_triggered_ability_bindings().iter().any(|binding| {
        binding.card_definition == definition.id
            && binding.ability.id == "combat-player-recipient-private-three-card-discard"
    }));
}

#[test]
fn mindleech_mass_captures_combat_recipient_for_private_three_card_discard() {
    let controller = PlayerId(0);
    let recipient = PlayerId(1);
    let mut game = game_with_rav_triggers();
    let mass = game
        .put_on_battlefield(controller, "RAV-MINDLEECH-MASS")
        .expect("Mindleech Mass setup");
    game.set_entered_turn_for_setup(mass, 0)
        .expect("fixture makes Mindleech Mass eligible to attack");
    let discarded = [
        game.add_card(recipient, "RAV-WATCHWOLF", Zone::Hand)
            .expect("first recipient hand setup"),
        game.add_card(recipient, "RAV-GLASS-GOLEM", Zone::Hand)
            .expect("second recipient hand setup"),
        game.add_card(recipient, "RAV-SNAPPING-DRAKE", Zone::Hand)
            .expect("third recipient hand setup"),
    ];

    advance_to_attackers(&mut game);
    game.declare_attackers(controller, &[mass])
        .expect("Mindleech Mass attacks");
    pass_pair(&mut game);
    assert_eq!(game.step, Step::DeclareBlockers);
    game.declare_blockers(recipient, &[])
        .expect("recipient declares no blockers");
    pass_pair(&mut game);
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::DamageDealtToPlayer { source, player, amount: 6 }
            if *source == mass && *player == recipient
    )));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::TriggeredAbilityStacked { source, ability, .. }
            if *source == mass
                && *ability == "combat-player-recipient-private-three-card-discard"
    )));

    pass_pair(&mut game);
    let decision = game
        .view_for_player(recipient)
        .expect("recipient view")
        .pending_decision
        .expect("combat recipient receives private discard decision");
    assert_eq!(decision.visibility, DecisionVisibility::Private);
    assert_eq!((decision.min_selections, decision.max_selections), (3, 3));
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
        DecisionSelection::Objects(discarded.to_vec()),
    )
    .expect("recipient selects all three current hand cards");

    for card in discarded {
        assert_eq!(game.zone_of(card), Some(Zone::Graveyard));
    }
    assert_eq!(
        game.event_log
            .iter()
            .filter(|event| matches!(event, GameEvent::CardDiscarded { player, .. } if *player == recipient))
            .count(),
        3
    );
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::AbilityResolved { source, ability, .. }
            if *source == mass
                && *ability == "combat-player-recipient-private-three-card-discard"
    )));
    game.validate_invariants()
        .expect("captured combat discard preserves engine invariants");
    eprintln!("mindleech_mass_trace={:?}", game.canonical_event_log());
}
