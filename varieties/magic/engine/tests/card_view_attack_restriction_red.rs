//! Red regression: a policy's attackability projection must agree with the
//! actual attacker-declaration legality boundary.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, ContinuousChange, Duration, Game, GameEvent, Keyword, ManaCost,
    PlayerId, RulesError, Step,
};

const CREATURE: &str = "TST-CARD-VIEW-ATTACK-RESTRICTION";

fn creature() -> CardDefinition {
    CardDefinition {
        id: CREATURE,
        name: CREATURE,
        set_code: "TST",
        mana_cost: ManaCost::new(0),
        colors: BTreeSet::new(),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Creature]),
        is_basic_land: false,
        supported_rules: &["card-view-attack-restriction-red"],
        power: Some(2),
        toughness: Some(2),
        keywords: vec![Keyword::Haste],
        effects: vec![],
    }
}

fn reach_declare_attackers(game: &mut Game) {
    for _ in 0..8 {
        let player = game.priority;
        game.pass_priority(player)
            .expect("priority passes reach attacker declaration");
    }
    assert_eq!(game.step, Step::DeclareAttackers);
}

#[test]
fn card_view_does_not_offer_a_cannot_attack_or_block_creature_as_an_attacker() {
    let player = PlayerId(0);
    let mut game = Game::new([creature()], 2).expect("fixture initializes");
    let creature = game
        .put_on_battlefield(player, CREATURE)
        .expect("hasty creature enters");
    game.begin_game().expect("game begins");
    game.add_continuous_effect(
        creature,
        creature,
        ContinuousChange::AddKeyword(Keyword::CannotAttackOrBlock),
        Duration::EndOfTurn(game.turn),
    )
    .expect("combat restriction installs");
    reach_declare_attackers(&mut game);

    let card = game
        .view_for_player(player)
        .expect("active-player view is available")
        .own_battlefield
        .into_iter()
        .find(|card| card.id == creature)
        .expect("creature is visible to its controller");
    let declaration = game.declare_attackers(player, &[creature]);
    eprintln!(
        "card-view attack restriction red: can_attack={}; declaration={declaration:?}; events={:?}",
        card.can_attack,
        game.canonical_event_log()
    );
    assert!(
        !card.can_attack,
        "policy view must not advertise a creature that the declaration boundary forbids"
    );
    assert!(matches!(
        declaration,
        Err(RulesError::IllegalAction("illegal attacker"))
    ));
    assert!(game.event_log.iter().all(|event| !matches!(
        event,
        GameEvent::AttackersDeclared { attackers, .. } if attackers.contains(&creature)
    )));
    game.validate_invariants()
        .expect("rejected attacker preserves a valid combat state");
}
