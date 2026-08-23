//! Red regression: a control change removes a permanent from combat.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, Color, ContinuousChange, Duration, Game, GameEvent, Keyword,
    ManaCost, PlayerId, Step,
};

const HASTY_ATTACKER: &str = "TEST-CONTROL-CHANGE-COMBAT-ATTACKER";

fn definition() -> CardDefinition {
    CardDefinition {
        id: HASTY_ATTACKER,
        name: "Control Change Combat Attacker",
        set_code: "TST",
        mana_cost: ManaCost::new(0),
        colors: BTreeSet::from([Color::Red]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Creature]),
        is_basic_land: false,
        supported_rules: &["base-characteristics", "haste", "control-change"],
        power: Some(3),
        toughness: Some(3),
        keywords: vec![Keyword::Haste],
        effects: vec![],
    }
}

fn advance_to_declare_attackers(game: &mut Game) {
    for _ in 0..20 {
        if game.step == Step::DeclareAttackers {
            return;
        }
        game.pass_priority(game.priority)
            .expect("ordinary priority pass advances the turn");
    }
    panic!("did not reach declare attackers; step={:?}", game.step);
}

#[test]
fn control_change_removes_a_declared_attacker_before_blockers_or_damage() {
    let mut game = Game::new(vec![definition()], 2).expect("fixture game builds");
    let attacker = game
        .put_on_battlefield(PlayerId(0), HASTY_ATTACKER)
        .expect("attacker setup");
    game.begin_game().expect("game begins");
    advance_to_declare_attackers(&mut game);
    game.declare_attackers(PlayerId(0), &[attacker])
        .expect("haste permits the declared attack");

    game.add_continuous_effect(
        attacker,
        attacker,
        ContinuousChange::ChangeController(PlayerId(1)),
        Duration::EndOfTurn(game.turn),
    )
    .expect("the control effect itself is legal");

    assert_eq!(
        game.controller_of(attacker).expect("attacker remains live"),
        PlayerId(1),
        "the layer-two change took effect"
    );
    assert!(
        game.view_for_player(PlayerId(0))
            .expect("attacker-controller view")
            .combat_attackers
            .is_empty(),
        "a creature whose controller changes must be removed from combat; events={:?}",
        game.event_log
    );
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::ControllerChanged { target, from, to, .. }
            if *target == attacker && *from == PlayerId(0) && *to == PlayerId(1)
    )));
    game.pass_priority(PlayerId(0))
        .expect("former attacker controller passes");
    game.pass_priority(PlayerId(1))
        .expect("combat skips blockers and damage after the attacker left combat");
    assert_eq!(game.step, Step::EndOfCombat);
    assert_eq!(
        game.player(PlayerId(1)).expect("defender exists").life,
        20,
        "a control-changed attacker deals no combat damage"
    );
    game.validate_invariants()
        .expect("control change leaves no stale attacker provenance");
}
