//! Red regression: a departed first-strike attacker cannot leave stale combat provenance.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, Color, ContinuousChange, Duration, Game, Keyword, ManaCost, PlayerId,
    Step,
};

const FIRST_STRIKER: &str = "TEST-FIRST-STRIKE-COMBAT-REMOVAL";

fn definition() -> CardDefinition {
    CardDefinition {
        id: FIRST_STRIKER,
        name: "First Strike Combat Removal",
        set_code: "TST",
        mana_cost: ManaCost::new(0),
        colors: BTreeSet::from([Color::White]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Creature]),
        is_basic_land: false,
        supported_rules: &[
            "base-characteristics",
            "haste",
            "first-strike",
            "control-change",
        ],
        power: Some(2),
        toughness: Some(2),
        keywords: vec![Keyword::Haste, Keyword::FirstStrike],
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
fn control_change_after_first_strike_damage_keeps_combat_provenance_auditable() {
    let mut game = Game::new(vec![definition()], 2).expect("fixture game builds");
    let attacker = game
        .put_on_battlefield(PlayerId(0), FIRST_STRIKER)
        .expect("attacker setup");
    game.begin_game().expect("game begins");
    advance_to_declare_attackers(&mut game);
    game.declare_attackers(PlayerId(0), &[attacker])
        .expect("haste permits the declared attack");
    game.pass_priority(PlayerId(0))
        .expect("attacker controller passes to blockers");
    game.pass_priority(PlayerId(1))
        .expect("defender receives blocker declaration");
    assert_eq!(game.step, Step::DeclareBlockers);
    game.declare_blockers(PlayerId(1), &[])
        .expect("empty blockers are legal");
    game.pass_priority(PlayerId(0))
        .expect("attacker controller passes into first strike");
    game.pass_priority(PlayerId(1))
        .expect("first-strike damage resolves");
    assert_eq!(game.step, Step::FirstStrikeCombatDamage);
    assert_eq!(game.player(PlayerId(1)).expect("defender exists").life, 18);

    let result = game.add_continuous_effect(
        attacker,
        attacker,
        ContinuousChange::ChangeController(PlayerId(1)),
        Duration::EndOfTurn(game.turn),
    );
    let event_tail = game
        .canonical_event_log()
        .into_iter()
        .rev()
        .take(5)
        .collect::<Vec<_>>();
    assert!(
        result.is_ok(),
        "removing a first-strike attacker from combat must not leave stale source provenance: {result:?}; event_tail={event_tail:?}"
    );
    assert!(
        game.view_for_player(PlayerId(0))
            .expect("combat view")
            .combat_attackers
            .is_empty(),
        "the control-changed creature leaves combat"
    );
    game.validate_invariants()
        .expect("first-strike provenance remains coherent after combat removal");
}
