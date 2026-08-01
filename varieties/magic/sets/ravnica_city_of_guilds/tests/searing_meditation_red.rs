//! Red discovery contract for Searing Meditation's life-gain trigger.

use cardbench_magic_engine::{
    CardType, CastRequest, Color, Game, GameEvent, ManaCost, PlayerId, Target, Zone,
};
use cardbench_magic_rav::{
    RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, rav_activated_ability_bindings,
    rav_additional_spell_cost_bindings, rav_basic_land_type_bindings, rav_mana_ability_bindings,
    rav_triggered_ability_bindings,
};

#[test]
fn searing_meditation_requires_its_life_gain_trigger() {
    let meditation = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-SEARING-MEDITATION")
        .expect("Searing Meditation definition exists");
    assert_eq!(meditation.name, "Searing Meditation");
    assert_eq!(
        meditation.mana_cost,
        ManaCost::with_colors(1, [Color::Red, Color::White])
    );
    assert_eq!(
        meditation.card_types,
        [CardType::Enchantment].into_iter().collect()
    );
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&meditation.id));
    assert!(meditation.supported_rules.contains(&"life-gain-trigger"));
}

#[test]
#[allow(clippy::too_many_lines)]
fn searing_meditation_pays_two_and_deals_two_after_life_gain() {
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
    let meditation = game
        .put_on_battlefield(PlayerId(0), "RAV-SEARING-MEDITATION")
        .expect("Searing Meditation enters");
    let helix = game
        .add_card(PlayerId(0), "RAV-LIGHTNING-HELIX", Zone::Hand)
        .expect("Lightning Helix enters hand");
    let red_sources = (0..4)
        .map(|_| {
            game.put_on_battlefield(PlayerId(0), "RAV-MOUNTAIN")
                .expect("Mountain enters")
        })
        .collect::<Vec<_>>();
    let white_sources = (0..2)
        .map(|_| {
            game.put_on_battlefield(PlayerId(0), "RAV-PLAINS")
                .expect("Plains enters")
        })
        .collect::<Vec<_>>();
    game.begin_game().expect("game starts");
    for _ in 0..2 {
        game.pass_priority(PlayerId(0)).expect("early pass");
        game.pass_priority(PlayerId(1))
            .expect("early response pass");
    }
    for source in red_sources {
        game.activate_mana_ability(PlayerId(0), source, Color::Red)
            .expect("red mana");
    }
    for source in white_sources {
        game.activate_mana_ability(PlayerId(0), source, Color::White)
            .expect("white mana");
    }
    game.cast_spell(
        PlayerId(0),
        CastRequest {
            card: helix,
            targets: vec![Target::Player(PlayerId(1))],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("Helix casts");
    game.pass_priority(PlayerId(0)).expect("spell pass");
    game.pass_priority(PlayerId(1)).expect("Helix resolves");
    println!(
        "Searing event log after life gain: {:#?}",
        game.canonical_event_log()
    );
    let stacked_index = game
        .event_log
        .iter()
        .position(|event| {
            matches!(
                event,
                GameEvent::TriggeredAbilityStacked { source, ability, .. }
                    if *source == meditation && *ability == "life-gain-deal-two"
            )
        })
        .expect("life-gain trigger stacks");
    assert!(!game.event_log[..stacked_index].iter().any(|event| {
        matches!(
            event,
            GameEvent::AbilityManaPaid { source, ability, .. }
                if *source == meditation && *ability == "life-gain-deal-two"
        )
    }));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::LifeGained {
            player: PlayerId(0),
            amount: 3
        }
    )));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::TriggeredAbilityStacked { source, ability, .. }
            if *source == meditation && *ability == "life-gain-deal-two"
    )));
    game.pass_priority(PlayerId(0)).expect("trigger pass");
    game.pass_priority(PlayerId(1)).expect("trigger resolves");
    println!(
        "Searing event log after trigger: {:#?}",
        game.canonical_event_log()
    );
    let paid_index = game
        .event_log
        .iter()
        .position(|event| {
            matches!(
                event,
                GameEvent::AbilityManaPaid { source, ability, .. }
                    if *source == meditation && *ability == "life-gain-deal-two"
            )
        })
        .expect("life-gain trigger pays on resolution");
    let damage_index = game
        .event_log
        .iter()
        .position(|event| {
            matches!(
                event,
                GameEvent::DamageDealtToPlayer { source, player: PlayerId(1), amount: 2 }
                    if *source == meditation
            )
        })
        .expect("Searing damage receipt");
    assert!(stacked_index < paid_index && paid_index < damage_index);
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::DamageDealtToPlayer { source, player: PlayerId(1), amount: 2 }
            if *source == meditation
    )));
    game.validate_invariants().expect("Searing trace is valid");
}

#[test]
fn searing_meditation_may_decline_when_two_mana_is_unavailable() {
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
    let meditation = game
        .put_on_battlefield(PlayerId(0), "RAV-SEARING-MEDITATION")
        .expect("Searing Meditation enters");
    let helix = game
        .add_card(PlayerId(0), "RAV-LIGHTNING-HELIX", Zone::Hand)
        .expect("Lightning Helix enters hand");
    let red_source = game
        .put_on_battlefield(PlayerId(0), "RAV-MOUNTAIN")
        .expect("Mountain enters");
    let white_source = game
        .put_on_battlefield(PlayerId(0), "RAV-PLAINS")
        .expect("Plains enters");
    game.begin_game().expect("game starts");
    for _ in 0..2 {
        game.pass_priority(PlayerId(0)).expect("early pass");
        game.pass_priority(PlayerId(1))
            .expect("early response pass");
    }
    game.activate_mana_ability(PlayerId(0), red_source, Color::Red)
        .expect("red mana");
    game.activate_mana_ability(PlayerId(0), white_source, Color::White)
        .expect("white mana");
    game.cast_spell(
        PlayerId(0),
        CastRequest {
            card: helix,
            targets: vec![Target::Player(PlayerId(1))],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("Helix casts");
    game.pass_priority(PlayerId(0)).expect("spell pass");
    game.pass_priority(PlayerId(1)).expect("Helix resolves");
    game.pass_priority(PlayerId(0)).expect("trigger pass");
    game.pass_priority(PlayerId(1))
        .expect("trigger resolves without payment");
    assert!(!game.event_log.iter().any(|event| {
        matches!(
            event,
            GameEvent::AbilityManaPaid { source, ability, .. }
                if *source == meditation && *ability == "life-gain-deal-two"
        )
    }));
    assert!(!game.event_log.iter().any(|event| {
        matches!(
            event,
            GameEvent::DamageDealtToPlayer { source, .. } if *source == meditation
        )
    }));
    game.validate_invariants()
        .expect("declined Searing trace is valid");
}

#[test]
fn searing_meditation_triggers_only_for_its_controller_life_gain() {
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
    let own_meditation = game
        .put_on_battlefield(PlayerId(0), "RAV-SEARING-MEDITATION")
        .expect("own Searing Meditation enters");
    let opponent_meditation = game
        .put_on_battlefield(PlayerId(1), "RAV-SEARING-MEDITATION")
        .expect("opponent Searing Meditation enters");
    let helix = game
        .add_card(PlayerId(0), "RAV-LIGHTNING-HELIX", Zone::Hand)
        .expect("Lightning Helix enters hand");
    let red_sources = (0..4)
        .map(|_| {
            game.put_on_battlefield(PlayerId(0), "RAV-MOUNTAIN")
                .expect("Mountain enters before the game")
        })
        .collect::<Vec<_>>();
    let white_sources = (0..2)
        .map(|_| {
            game.put_on_battlefield(PlayerId(0), "RAV-PLAINS")
                .expect("Plains enters before the game")
        })
        .collect::<Vec<_>>();
    game.begin_game().expect("game starts");
    for _ in 0..2 {
        game.pass_priority(PlayerId(0)).expect("early pass");
        game.pass_priority(PlayerId(1))
            .expect("early response pass");
    }
    for source in red_sources {
        game.activate_mana_ability(PlayerId(0), source, Color::Red)
            .expect("red mana");
    }
    for source in white_sources {
        game.activate_mana_ability(PlayerId(0), source, Color::White)
            .expect("white mana");
    }
    game.cast_spell(
        PlayerId(0),
        CastRequest {
            card: helix,
            targets: vec![Target::Player(PlayerId(1))],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("Helix casts");
    game.pass_priority(PlayerId(0)).expect("spell pass");
    game.pass_priority(PlayerId(1)).expect("Helix resolves");
    let trigger_sources = game
        .event_log
        .iter()
        .filter_map(|event| match event {
            GameEvent::TriggeredAbilityStacked {
                source,
                ability: "life-gain-deal-two",
                ..
            } => Some(*source),
            _ => None,
        })
        .collect::<Vec<_>>();
    println!(
        "Searing controller-scope event log: {:#?}",
        game.canonical_event_log()
    );
    assert_eq!(trigger_sources, vec![own_meditation]);
    assert!(!trigger_sources.contains(&opponent_meditation));
    game.validate_invariants()
        .expect("controller-scoped life-gain trigger is valid");
}
