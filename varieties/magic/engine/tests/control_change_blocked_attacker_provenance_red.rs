//! Red regression: pruning a blocked attacker retires its live blocker provenance.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, Color, CombatBlock, ContinuousChange, Duration, Game, Keyword,
    ManaCost, PlayerId, Step,
};

const FLYING_ATTACKER: &str = "TEST-CONTROL-CHANGE-FLYING-ATTACKER";
const FLYING_BLOCKER: &str = "TEST-CONTROL-CHANGE-FLYING-BLOCKER";

fn definition(id: &'static str, color: Color, keywords: Vec<Keyword>) -> CardDefinition {
    CardDefinition {
        id,
        name: id,
        set_code: "TST",
        mana_cost: ManaCost::new(0),
        colors: BTreeSet::from([color]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Creature]),
        is_basic_land: false,
        supported_rules: &["base-characteristics", "haste", "flying", "control-change"],
        power: Some(3),
        toughness: Some(3),
        keywords,
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
fn control_change_of_blocked_flying_attacker_retires_blocker_qualification_provenance() {
    let mut game = Game::new(
        vec![
            definition(
                FLYING_ATTACKER,
                Color::Blue,
                vec![Keyword::Haste, Keyword::Flying],
            ),
            definition(FLYING_BLOCKER, Color::White, vec![Keyword::Flying]),
        ],
        2,
    )
    .expect("fixture game builds");
    let attacker = game
        .put_on_battlefield(PlayerId(0), FLYING_ATTACKER)
        .expect("attacker setup");
    let blocker = game
        .put_on_battlefield(PlayerId(1), FLYING_BLOCKER)
        .expect("blocker setup");
    game.begin_game().expect("game begins");
    advance_to_declare_attackers(&mut game);
    game.declare_attackers(PlayerId(0), &[attacker])
        .expect("haste permits the declared attack");
    game.pass_priority(PlayerId(0))
        .expect("attacker controller passes to blockers");
    game.pass_priority(PlayerId(1))
        .expect("defender receives blocker declaration");
    game.declare_blockers(PlayerId(1), &[CombatBlock { attacker, blocker }])
        .expect("the flying blocker may block the flying attacker");

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
        .take(4)
        .collect::<Vec<_>>();
    assert!(
        result.is_ok(),
        "removing the blocked attacker must retire its live blocker provenance: {result:?}; event_tail={event_tail:?}"
    );
    assert!(
        game.view_for_player(PlayerId(0))
            .expect("combat view")
            .combat_attackers
            .is_empty(),
        "the control-changed attacker left combat"
    );
    game.pass_priority(PlayerId(0))
        .expect("former attacker controller passes");
    game.pass_priority(PlayerId(1))
        .expect("combat skips damage after the attacker left combat");
    assert_eq!(game.step, Step::EndOfCombat);
    assert_eq!(
        game.player(PlayerId(1)).expect("defender exists").life,
        20,
        "the formerly blocked attacker deals no combat damage"
    );
    game.validate_invariants()
        .expect("no blocker qualification provenance survives its attacker removal");
}
