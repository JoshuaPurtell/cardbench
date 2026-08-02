//! Red discovery contract for Ghosts of the Innocent's global damage reduction.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardType, CastRequest, Color, DamageReplacementChoice, Game, GameEvent, ManaCost, PlayerId,
    Target, Zone,
};
use cardbench_magic_rav::{
    RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, executable_definition_id_for_collector,
    rav_damage_replacement_effect_bindings,
};

#[test]
fn ghosts_of_the_innocent_requires_a_global_damage_amount_replacement_slice() {
    let ghosts = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-GHOSTS-OF-THE-INNOCENT")
        .expect("Ghosts of the Innocent definition exists");

    assert_eq!(ghosts.name, "Ghosts of the Innocent");
    assert_eq!(
        ghosts.mana_cost,
        ManaCost::with_colors(5, [Color::White, Color::White])
    );
    assert_eq!(ghosts.colors, BTreeSet::from([Color::White]));
    assert_eq!(ghosts.card_types, BTreeSet::from([CardType::Creature]));
    assert_eq!((ghosts.power, ghosts.toughness), (Some(4), Some(5)));
    assert!(ghosts.keywords.is_empty());
    assert!(ghosts.effects.is_empty());
    assert!(
        ghosts
            .supported_rules
            .contains(&"static-global-damage-amount-halving")
    );
    assert_eq!(
        executable_definition_id_for_collector(20),
        Ok("RAV-GHOSTS-OF-THE-INNOCENT")
    );
    assert!(
        !RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&"RAV-GHOSTS-OF-THE-INNOCENT"),
        "global replacement-order coverage remains an explicit bounded compatibility scope"
    );
}

fn pass_pair(game: &mut Game) {
    let first = game.priority;
    game.pass_priority(first).expect("first priority pass");
    let second = game.priority;
    game.pass_priority(second).expect("second priority pass");
}

#[test]
fn ghosts_halves_player_and_permanent_damage_with_a_provenanced_event_trace() {
    let mut game = Game::new_with_all_bindings(card_definitions(), 2, [], [], [], [])
        .expect("RAV fixture builds");
    game.register_damage_replacement_effect_bindings(rav_damage_replacement_effect_bindings())
        .expect("Ghosts replacement binding registers");
    let ghosts = game
        .put_on_battlefield(PlayerId(0), "RAV-GHOSTS-OF-THE-INNOCENT")
        .expect("Ghosts setup");
    let watchwolf = game
        .put_on_battlefield(PlayerId(0), "RAV-WATCHWOLF")
        .expect("permanent recipient setup");
    let permanent_source = game
        .add_card(PlayerId(0), "RAV-CHAR", Zone::Hand)
        .expect("permanent damage source setup");
    let player_source = game
        .add_card(PlayerId(1), "RAV-CHAR", Zone::Hand)
        .expect("player damage source setup");
    game.grant_mana(PlayerId(0), Color::Red, 3)
        .expect("first damage payment setup");
    game.grant_mana(PlayerId(1), Color::Red, 3)
        .expect("second damage payment setup");

    game.cast_spell(
        PlayerId(0),
        CastRequest {
            card: permanent_source,
            targets: vec![Target::Permanent(watchwolf)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("permanent-targeting damage spell casts");
    pass_pair(&mut game);
    assert_eq!(
        game.object(watchwolf)
            .expect("reduced damage does not destroy Watchwolf")
            .damage,
        2
    );
    // The RAV Char fixture also deals two source-controller damage, which
    // Ghosts reduces to one; retain that independent packet in the baseline.
    assert_eq!(game.player(PlayerId(0)).expect("player exists").life, 19);

    game.pass_priority(PlayerId(0))
        .expect("opponent receives priority for player damage");
    game.cast_spell(
        PlayerId(1),
        CastRequest {
            card: player_source,
            targets: vec![Target::Player(PlayerId(0))],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("player-targeting damage spell casts");
    pass_pair(&mut game);
    assert_eq!(game.player(PlayerId(0)).expect("player exists").life, 17);

    for (source, target) in [
        (permanent_source, Target::Permanent(watchwolf)),
        (player_source, Target::Player(PlayerId(0))),
    ] {
        let replacement_index = game
            .event_log
            .iter()
            .position(|event| {
                matches!(
                    event,
                    GameEvent::DamageAmountReplaced {
                        source: event_source,
                        target: event_target,
                        replacement_source,
                        original_amount: 4,
                        replacement_amount: 2,
                        ..
                    } if *event_source == source
                        && *event_target == target
                        && *replacement_source == ghosts
                )
            })
            .expect("reduced packet receipt exists");
        assert!(matches!(
            game.event_log.get(replacement_index.checked_sub(1).expect("receipt has predecessor")),
            Some(GameEvent::DamageReplacementApplied {
                target: event_target,
                replacement: DamageReplacementChoice::HalveDamage { source, .. },
                ..
            }) if *event_target == target && *source == ghosts
        ));
    }
    eprintln!("Ghosts trace={:?}", game.canonical_event_log());
    game.validate_invariants()
        .expect("Ghosts replacement trace preserves invariant state");
}
