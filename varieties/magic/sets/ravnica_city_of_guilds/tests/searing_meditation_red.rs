//! Red discovery contract for Searing Meditation's life-gain trigger.

use cardbench_magic_engine::{
    CardType, CastRequest, Color, Game, GameEvent, ManaCost, PlayerId, Target, Zone,
};
use cardbench_magic_rav::{
    card_definitions, rav_activated_ability_bindings, rav_additional_spell_cost_bindings,
    rav_basic_land_type_bindings, rav_mana_ability_bindings, rav_triggered_ability_bindings,
    RAV_FULL_FIDELITY_DEFINITION_IDS,
};

#[test]
fn searing_meditation_requires_its_life_gain_trigger() {
    let meditation = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-SEARING-MEDITATION")
        .expect("Searing Meditation definition exists");
    assert_eq!(meditation.name, "Searing Meditation");
    assert_eq!(meditation.mana_cost, ManaCost::with_colors(1, [Color::Red, Color::White]));
    assert_eq!(meditation.card_types, [CardType::Enchantment].into_iter().collect());
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&meditation.id));
    assert!(meditation.supported_rules.contains(&"life-gain-trigger"));
}

#[test]
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
    game.begin_game().expect("game starts");
    for _ in 0..2 {
        game.pass_priority(PlayerId(0)).expect("early pass");
        game.pass_priority(PlayerId(1)).expect("early response pass");
    }
    game.grant_mana(PlayerId(0), Color::Red, 4)
        .expect("red mana");
    game.grant_mana(PlayerId(0), Color::White, 2)
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
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::LifeGained { player: PlayerId(0), amount: 3 }
    )));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::TriggeredAbilityStacked { source, ability, .. }
            if *source == meditation && *ability == "life-gain-deal-two"
    )));
    game.pass_priority(PlayerId(0)).expect("trigger pass");
    game.pass_priority(PlayerId(1)).expect("trigger resolves");
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::DamageDealtToPlayer { source, player: PlayerId(1), amount: 2 }
            if *source == meditation
    )));
    game.validate_invariants().expect("Searing trace is valid");
}
