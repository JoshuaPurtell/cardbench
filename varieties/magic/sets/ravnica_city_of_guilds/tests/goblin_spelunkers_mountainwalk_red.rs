use cardbench_magic_engine::{
    BasicLandType, CombatBlock, Game, GameEvent, Keyword, PlayerId, Step, Zone,
};
use cardbench_magic_rav::{card_definitions, rav_basic_land_type_bindings};

#[test]
fn goblin_spelunkers_declares_mountainwalk() {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-GOBLIN-SPELUNKERS")
        .expect("Goblin Spelunkers exists");
    assert!(definition.keywords.contains(&Keyword::Mountainwalk));
}

#[test]
fn mountainwalk_rejects_a_blocker_while_defender_controls_mountain() {
    let mut game =
        Game::new_with_basic_land_types(card_definitions(), 2, rav_basic_land_type_bindings())
            .expect("RAV catalog builds");
    let attacker = game
        .put_on_battlefield(PlayerId(0), "RAV-GOBLIN-SPELUNKERS")
        .expect("attacker enters");
    game.set_entered_turn_for_setup(attacker, 0)
        .expect("attacker is old");
    game.put_on_battlefield(PlayerId(1), "RAV-MOUNTAIN")
        .expect("Mountain enters");
    let blocker = game
        .put_on_battlefield(PlayerId(1), "RAV-WATCHWOLF")
        .expect("blocker enters");
    game.set_entered_turn_for_setup(blocker, 0)
        .expect("blocker is old");
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
    let before = game.event_log.len();
    let result = game.declare_blockers(PlayerId(1), &[CombatBlock { attacker, blocker }]);
    assert!(result.is_err());
    assert_eq!(game.event_log.len(), before);
    assert!(
        !game
            .event_log
            .iter()
            .any(|event| matches!(event, GameEvent::BlockersDeclared { .. }))
    );
    assert_eq!(game.zone_of(blocker), Some(Zone::Battlefield));
    assert_eq!(
        game.basic_land_type(game.players[1].battlefield[0])
            .expect("typed land"),
        Some(BasicLandType::Mountain)
    );
}
