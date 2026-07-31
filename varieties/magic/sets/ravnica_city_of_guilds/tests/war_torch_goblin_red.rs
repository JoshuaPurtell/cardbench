use std::collections::BTreeSet;

use cardbench_magic_engine::{
    AbilityActivation, CardType, Color, CombatBlock, Game, GameEvent, PlayerId, Step, Target, Zone,
};
use cardbench_magic_rav::{
    card_definitions, rav_activated_ability_bindings, rav_additional_spell_cost_bindings,
    rav_basic_land_type_bindings, rav_mana_ability_bindings,
};

#[test]
fn war_torch_goblin_is_executable_with_its_sacrifice_damage_ability() {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-WAR-TORCH-GOBLIN")
        .expect("War-Torch Goblin exists");
    assert_eq!(definition.colors, BTreeSet::from([Color::Red]));
    assert_eq!(definition.card_types, BTreeSet::from([CardType::Creature]));
    assert!(definition.keywords.is_empty());
    assert!(definition.supported_rules.contains(&"sacrifice-source"));
}

#[test]
fn war_torch_sacrifices_source_and_damages_a_declared_blocker() {
    let mut game = Game::new_with_all_bindings(
        card_definitions(),
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        rav_activated_ability_bindings(),
    )
    .expect("RAV game builds");
    let source = game
        .put_on_battlefield(PlayerId(0), "RAV-WAR-TORCH-GOBLIN")
        .expect("source enters");
    let attacker = game
        .put_on_battlefield(PlayerId(0), "RAV-WATCHWOLF")
        .expect("attacker enters");
    let blocker = game
        .put_on_battlefield(PlayerId(1), "RAV-WATCHWOLF")
        .expect("blocker enters");
    for card in [source, attacker, blocker] {
        game.set_entered_turn_for_setup(card, 0)
            .expect("old fixture entry");
    }
    game.begin_game().expect("game starts");
    while game.step != Step::DeclareAttackers {
        let priority = game.priority;
        game.pass_priority(priority).expect("advance turn");
    }
    game.declare_attackers(PlayerId(0), &[attacker])
        .expect("attacker declared");
    game.pass_priority(PlayerId(0)).expect("attacker passes");
    game.pass_priority(PlayerId(1))
        .expect("advance to blockers");
    game.declare_blockers(PlayerId(1), &[CombatBlock { attacker, blocker }])
        .expect("blocker declared");
    game.grant_mana(PlayerId(0), Color::Red, 1)
        .expect("red activation mana");
    game.activate_ability(
        PlayerId(0),
        AbilityActivation {
            source,
            ability_id: "sacrifice-deal-two-to-blocker",
            targets: vec![Target::Permanent(blocker)],
        },
    )
    .expect("sacrifice ability activates");
    assert_eq!(game.zone_of(source), Some(Zone::Graveyard));
    game.pass_priority(PlayerId(0)).expect("activator passes");
    game.pass_priority(PlayerId(1)).expect("ability resolves");
    assert_eq!(game.object(blocker).expect("blocker survives").damage, 2);
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::AbilityResolved { source: resolved, .. } if *resolved == source
    )));
}
