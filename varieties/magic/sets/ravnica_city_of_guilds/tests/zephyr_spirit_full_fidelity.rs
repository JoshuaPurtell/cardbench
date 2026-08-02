//! Event-log contracts for Zephyr Spirit's source-relative blocking trigger.

use cardbench_magic_engine::{
    CastRequest, Color, CombatBlock, Game, GameEvent, PlayerId, Step, Target, Zone,
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
    .expect("RAV game builds")
}

fn pass_pair(game: &mut Game) {
    let first = game.priority;
    game.pass_priority(first).expect("first priority pass");
    let second = game.priority;
    game.pass_priority(second).expect("second priority pass");
}

fn advance_to_declare_attackers(game: &mut Game) {
    for _ in 0..8 {
        if game.step == Step::DeclareAttackers {
            return;
        }
        pass_pair(game);
    }
    panic!(
        "turn machine did not reach DeclareAttackers; reached {:?}",
        game.step
    );
}

fn declare_one_attack_then_reach_blockers(
    game: &mut Game,
    attacker: cardbench_magic_engine::ObjectId,
) {
    advance_to_declare_attackers(game);
    game.declare_attackers(PlayerId(0), &[attacker])
        .expect("attacker declaration succeeds");
    pass_pair(game);
    assert_eq!(game.step, Step::DeclareBlockers);
}

#[test]
fn zephyr_spirit_blocks_then_uses_the_ordinary_trigger_stack_and_owner_hand_move() {
    let mut game = game_with_rav_bindings();
    let attacker = game
        .put_on_battlefield(PlayerId(0), "RAV-WATCHWOLF")
        .expect("attacker setup");
    let zephyr = game
        .put_on_battlefield(PlayerId(1), "RAV-ZEPHYR-SPIRIT")
        .expect("blocker setup");
    game.set_entered_turn_for_setup(attacker, 0)
        .expect("attacker has prior-turn provenance");
    game.begin_game().expect("game starts");
    game.clear_event_log();

    declare_one_attack_then_reach_blockers(&mut game, attacker);
    game.declare_blockers(
        PlayerId(1),
        &[CombatBlock {
            attacker,
            blocker: zephyr,
        }],
    )
    .expect("Zephyr Spirit is a legal blocker");

    assert_eq!(game.zone_of(zephyr), Some(Zone::Battlefield));
    assert_eq!(
        game.stack.len(),
        1,
        "the blocking trigger reaches the stack"
    );
    let blockers_declared = game
        .event_log
        .iter()
        .position(|event| matches!(event, GameEvent::BlockersDeclared { .. }))
        .expect("blocker declaration receipt");
    let trigger_stacked = game
        .event_log
        .iter()
        .position(|event| {
            matches!(
                event,
                GameEvent::TriggeredAbilityStacked { source, ability, .. }
                    if *source == zephyr && *ability == "blocks-return-source-owner-hand"
            )
        })
        .expect("Zephyr trigger stack receipt");
    assert!(
        blockers_declared < trigger_stacked,
        "blocker commitment precedes the trigger's ordinary stack lifecycle"
    );

    pass_pair(&mut game);
    println!(
        "zephyr_spirit_blocks_trace={:#?}",
        game.canonical_event_log()
    );
    assert_eq!(game.zone_of(zephyr), Some(Zone::Hand));
    let returned = game
        .event_log
        .iter()
        .position(|event| matches!(event, GameEvent::CardMoved { card, to: Zone::Hand } if *card == zephyr))
        .expect("owner-hand move receipt");
    let resolved = game
        .event_log
        .iter()
        .position(|event| {
            matches!(
                event,
                GameEvent::AbilityResolved { source, ability, .. }
                    if *source == zephyr && *ability == "blocks-return-source-owner-hand"
            )
        })
        .expect("trigger resolution receipt");
    assert!(
        trigger_stacked < returned && returned < resolved,
        "the source-relative hand move occurs only while the trigger resolves"
    );
    game.validate_invariants()
        .expect("Zephyr's legal blocker trigger preserves invariants");
}

#[test]
fn zephyr_spirit_blocks_trigger_does_not_follow_a_response_moved_source() {
    let mut game = game_with_rav_bindings();
    let attacker = game
        .put_on_battlefield(PlayerId(0), "RAV-WATCHWOLF")
        .expect("attacker setup");
    let zephyr = game
        .put_on_battlefield(PlayerId(1), "RAV-ZEPHYR-SPIRIT")
        .expect("blocker setup");
    let peel = game
        .add_card(PlayerId(0), "RAV-PEEL-FROM-REALITY", Zone::Hand)
        .expect("response setup");
    let islands = (0..2)
        .map(|_| {
            game.put_on_battlefield(PlayerId(0), "RAV-ISLAND")
                .expect("response mana setup")
        })
        .collect::<Vec<_>>();
    game.set_entered_turn_for_setup(attacker, 0)
        .expect("attacker has prior-turn provenance");
    game.begin_game().expect("game starts");

    declare_one_attack_then_reach_blockers(&mut game, attacker);
    game.declare_blockers(
        PlayerId(1),
        &[CombatBlock {
            attacker,
            blocker: zephyr,
        }],
    )
    .expect("Zephyr Spirit is a legal blocker");
    let trigger_incarnation = game.object(zephyr).expect("Zephyr exists").incarnation;
    for island in islands {
        game.activate_mana_ability(PlayerId(0), island, Color::Blue)
            .expect("response mana");
    }
    game.cast_spell(
        PlayerId(0),
        CastRequest {
            card: peel,
            targets: vec![Target::Permanent(attacker), Target::Permanent(zephyr)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("response returns both legal creature targets");
    pass_pair(&mut game);
    assert_eq!(game.zone_of(zephyr), Some(Zone::Hand));
    assert!(
        game.object(zephyr).expect("Zephyr exists").incarnation > trigger_incarnation,
        "the response creates a new nonbattlefield source incarnation"
    );

    pass_pair(&mut game);
    println!(
        "zephyr_spirit_departed_source_trace={:#?}",
        game.canonical_event_log()
    );
    assert_eq!(game.zone_of(zephyr), Some(Zone::Hand));
    assert_eq!(
        game.event_log
            .iter()
            .filter(|event| matches!(event, GameEvent::CardMoved { card, to: Zone::Hand } if *card == zephyr))
            .count(),
        1,
        "the old trigger cannot move a later source incarnation a second time"
    );
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::AbilityResolved { source, ability, .. }
            if *source == zephyr && *ability == "blocks-return-source-owner-hand"
    )));
    game.validate_invariants()
        .expect("a departed Zephyr trigger remains a stack-auditable no-op");
}

#[test]
fn uncommitted_zephyr_spirit_never_creates_a_blocks_trigger() {
    let mut game = game_with_rav_bindings();
    let attacker = game
        .put_on_battlefield(PlayerId(0), "RAV-WATCHWOLF")
        .expect("attacker setup");
    let zephyr = game
        .put_on_battlefield(PlayerId(1), "RAV-ZEPHYR-SPIRIT")
        .expect("nonblocking Zephyr setup");
    let actual_blocker = game
        .put_on_battlefield(PlayerId(1), "RAV-COURIER-HAWK")
        .expect("other blocker setup");
    game.set_entered_turn_for_setup(attacker, 0)
        .expect("attacker has prior-turn provenance");
    game.begin_game().expect("game starts");

    declare_one_attack_then_reach_blockers(&mut game, attacker);
    game.declare_blockers(
        PlayerId(1),
        &[CombatBlock {
            attacker,
            blocker: actual_blocker,
        }],
    )
    .expect("other blocker is legal");
    assert_eq!(game.stack.len(), 0, "only a committed source may trigger");
    assert!(
        !game.event_log.iter().any(|event| matches!(
            event,
            GameEvent::TriggeredAbilityStacked { source, ability, .. }
                if *source == zephyr && *ability == "blocks-return-source-owner-hand"
        )),
        "an attacker or unrelated battlefield creature cannot fabricate Zephyr's trigger"
    );
    game.validate_invariants()
        .expect("uncommitted source leaves no pending trigger state");
}

#[test]
fn zephyr_spirit_is_positive_manifest() {
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&"RAV-ZEPHYR-SPIRIT"));
}
