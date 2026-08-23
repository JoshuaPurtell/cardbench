//! Red discovery contract for Boros Fury-Shield's conditional combat prevention.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardType, CastRequest, Color, Game, GameEvent, ManaCost, ManaPaymentSelection, PlayerId, Step,
    Target, Zone,
};
use cardbench_magic_rav::{RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions};

#[test]
fn boros_fury_shield_has_its_exact_conditional_combat_prevention_contract() {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-BOROS-FURY-SHIELD")
        .expect("Boros Fury-Shield definition exists");
    assert_eq!(definition.name, "Boros Fury-Shield");
    assert_eq!(
        definition.mana_cost,
        ManaCost::with_colors(2, [Color::White])
    );
    assert_eq!(definition.colors, BTreeSet::from([Color::White]));
    assert_eq!(definition.card_types, BTreeSet::from([CardType::Instant]));
    assert_eq!((definition.power, definition.toughness), (None, None));
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&definition.id));
    assert!(
        definition
            .supported_rules
            .contains(&"prevent-target-creatures-combat-damage-and-red-spend-controller-damage")
    );
}

fn add_opening_library(game: &mut Game, player: PlayerId) {
    for _ in 0..8 {
        game.add_card(player, "RAV-PLAINS", Zone::Library)
            .expect("library fixture card exists");
    }
}

fn advance_to_opponent_declare_attackers(game: &mut Game) {
    while game.active_player != PlayerId(1) || game.step != Step::DeclareAttackers {
        if game
            .view_for_player(game.next_policy_player())
            .expect("public state")
            .draw_replacement_pending
        {
            let player = game.next_policy_player();
            game.resolve_pending_draw(player, None)
                .expect("ordinary draw is legal");
        }
        match game.step {
            Step::DeclareAttackers
                if !game
                    .view_for_player(game.next_policy_player())
                    .expect("combat state")
                    .attackers_declared =>
            {
                let player = game.next_policy_player();
                game.declare_attackers(player, &[])
                    .expect("empty attacker declaration is legal");
            }
            Step::DeclareBlockers
                if !game
                    .view_for_player(game.next_policy_player())
                    .expect("combat state")
                    .blockers_declared =>
            {
                let player = game.next_policy_player();
                game.declare_blockers(player, &[])
                    .expect("empty blocker declaration is legal");
            }
            _ => {
                let player = game.priority;
                game.pass_priority(player).expect("priority passes");
            }
        }
    }
}

fn pass_pair(game: &mut Game) {
    let first = game.priority;
    game.pass_priority(first).expect("first priority pass");
    let second = game.priority;
    game.pass_priority(second).expect("second priority pass");
}

#[test]
#[allow(clippy::too_many_lines)] // The response and combat windows are part of the contract.
fn boros_fury_shield_uses_red_spend_then_prevents_the_targets_combat_damage() {
    let mut game = Game::new(card_definitions(), 2).expect("RAV fixture builds");
    add_opening_library(&mut game, PlayerId(0));
    add_opening_library(&mut game, PlayerId(1));
    let attacker = game
        .put_on_battlefield(PlayerId(1), "RAV-WATCHWOLF")
        .expect("attacking creature setup");
    let shield = game
        .add_card(PlayerId(0), "RAV-BOROS-FURY-SHIELD", Zone::Hand)
        .expect("Boros Fury-Shield setup");
    let plains = game
        .put_on_battlefield(PlayerId(0), "RAV-PLAINS")
        .expect("white mana source setup");
    let first_mountain = game
        .put_on_battlefield(PlayerId(0), "RAV-MOUNTAIN")
        .expect("first red mana source setup");
    let second_mountain = game
        .put_on_battlefield(PlayerId(0), "RAV-MOUNTAIN")
        .expect("second red mana source setup");
    game.begin_game().expect("fixture begins");
    advance_to_opponent_declare_attackers(&mut game);

    game.declare_attackers(PlayerId(1), &[attacker])
        .expect("Watchwolf attacks");
    game.pass_priority(PlayerId(1))
        .expect("attacker controller passes to responder");
    for (land, color) in [
        (plains, Color::White),
        (first_mountain, Color::Red),
        (second_mountain, Color::Red),
    ] {
        game.activate_mana_ability(PlayerId(0), land, color)
            .expect("mana source produces selected spell payment");
    }
    game.cast_spell_with_mana_spend(
        PlayerId(0),
        CastRequest {
            card: shield,
            targets: vec![Target::Permanent(attacker)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
        ManaPaymentSelection {
            generic: vec![Color::Red, Color::Red],
            hybrid: vec![],
        },
    )
    .expect("Boros Fury-Shield casts with red generic payment");
    pass_pair(&mut game);

    assert_eq!(game.players[1].life, 17);
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::CombatDamagePreventionCreated {
            source,
            creature,
            expires_turn,
        } if *source == shield && *creature == attacker && *expires_turn == game.turn
    )));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::DamageDealtToPlayer { source, player, amount: 3 }
            if *source == shield && *player == PlayerId(1)
    )));

    pass_pair(&mut game);
    assert_eq!(game.step, Step::DeclareBlockers);
    game.declare_blockers(PlayerId(0), &[])
        .expect("no blockers declared");
    pass_pair(&mut game);
    assert_eq!(game.players[0].life, 20);
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::CombatDamagePrevented {
            source,
            prevented_by,
            target: Target::Player(player),
            amount: 3,
        } if *source == attacker && *prevented_by == shield && *player == PlayerId(0)
    )));
    eprintln!("Boros Fury-Shield trace={:?}", game.canonical_event_log());
    game.validate_invariants()
        .expect("Boros Fury-Shield preserves invariant state");
}
