//! Event-log contract for Grayscaled Gharial's static Islandwalk.

use cardbench_magic_engine::{BasicLandType, CombatBlock, Game, GameEvent, PlayerId, Step, Zone};
use cardbench_magic_rav::{card_definitions, rav_basic_land_type_bindings};

#[test]
fn islandwalk_rejects_blocking_without_emitting_a_blocker_receipt() {
    let mut game =
        Game::new_with_basic_land_types(card_definitions(), 2, rav_basic_land_type_bindings())
            .expect("RAV catalog builds");
    let attacker = game
        .put_on_battlefield(PlayerId(0), "RAV-GRAYSCALED-GHARIAL")
        .expect("Gharial enters");
    let island = game
        .put_on_battlefield(PlayerId(1), "RAV-ISLAND")
        .expect("defender Island enters");
    let blocker = game
        .put_on_battlefield(PlayerId(1), "RAV-WATCHWOLF")
        .expect("blocker enters");
    for creature in [attacker, blocker] {
        game.set_entered_turn_for_setup(creature, 0)
            .expect("creature predates the measured turn");
    }
    game.begin_game().expect("game starts");
    while game.step != Step::DeclareAttackers {
        let priority = game.priority;
        game.pass_priority(priority).expect("advance to combat");
    }
    game.declare_attackers(PlayerId(0), &[attacker])
        .expect("Islandwalking attacker declares");
    game.pass_priority(PlayerId(0)).expect("attacker passes");
    game.pass_priority(PlayerId(1))
        .expect("advance to blockers");

    let before = game.event_log.len();
    assert!(
        game.declare_blockers(PlayerId(1), &[CombatBlock { attacker, blocker }])
            .is_err(),
        "a defender controlling an Island cannot block the Islandwalking attacker"
    );
    assert_eq!(game.event_log.len(), before);
    assert_eq!(game.zone_of(blocker), Some(Zone::Battlefield));
    assert_eq!(
        game.basic_land_type(island)
            .expect("registered typed Island"),
        Some(BasicLandType::Island)
    );
    assert!(
        !game
            .event_log
            .iter()
            .any(|event| matches!(event, GameEvent::BlockersDeclared { .. }))
    );
    println!(
        "grayscaled_gharial_event_log={:?}",
        game.canonical_event_log()
    );
    game.validate_invariants()
        .expect("Islandwalk combat trace preserves invariants");
}
