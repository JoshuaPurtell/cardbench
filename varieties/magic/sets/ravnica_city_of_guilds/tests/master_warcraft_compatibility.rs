use cardbench_magic_engine::{
    CastRequest, Color, CombatBlock, Game, GameEvent, ManaPaymentSelection, PlayerId, Step, Zone,
};
use cardbench_magic_rav::card_definitions;

fn pass_pair(game: &mut Game) {
    let first = game.priority;
    game.pass_priority(first).expect("first pass");
    let second = game.priority;
    game.pass_priority(second).expect("second pass");
}

#[test]
fn master_warcraft_controller_makes_legal_declarations_for_both_combat_sides() {
    let mut game = Game::new(card_definitions(), 2).expect("fixture builds");
    for player in [PlayerId(0), PlayerId(1)] {
        game.add_card(player, "RAV-FOREST", Zone::Library)
            .expect("draw fixture");
    }
    let attacker = game
        .put_on_battlefield(PlayerId(0), "RAV-GOBLIN-FIRE-FIEND")
        .expect("attacker setup");
    let blocker = game
        .put_on_battlefield(PlayerId(1), "RAV-WATCHWOLF")
        .expect("blocker setup");
    let master = game
        .add_card(PlayerId(1), "RAV-MASTER-WARCRAFT", Zone::Hand)
        .expect("Master Warcraft setup");
    let mountains = (0..4)
        .map(|_| game.put_on_battlefield(PlayerId(1), "RAV-MOUNTAIN"))
        .collect::<Result<Vec<_>, _>>()
        .expect("Master Warcraft mana setup");
    game.begin_game().expect("fixture begins");

    game.pass_priority(PlayerId(0))
        .expect("caster receives upkeep priority");
    for mountain in mountains {
        game.activate_mana_ability(PlayerId(1), mountain, Color::Red)
            .expect("red mana source");
    }
    game.cast_spell_with_mana_spend(
        PlayerId(1),
        CastRequest {
            card: master,
            targets: vec![],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
        ManaPaymentSelection {
            generic: vec![Color::Red, Color::Red],
            hybrid: vec![Color::Red, Color::Red],
        },
    )
    .expect("Master Warcraft casts before attackers");
    pass_pair(&mut game);
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::CombatDeclarationAuthorityCreated { controller, .. }
            if *controller == PlayerId(1)
    )));

    pass_pair(&mut game);
    assert_eq!(game.step, Step::Draw);
    pass_pair(&mut game);
    assert_eq!(game.step, Step::PrecombatMain);
    pass_pair(&mut game);
    assert_eq!(game.step, Step::BeginningOfCombat);
    pass_pair(&mut game);
    assert_eq!(game.step, Step::DeclareAttackers);
    assert_eq!(game.next_policy_player(), PlayerId(1));
    game.declare_attackers(PlayerId(1), &[attacker])
        .expect("Master Warcraft controller chooses active player's attacker");
    pass_pair(&mut game);
    assert_eq!(game.step, Step::DeclareBlockers);
    assert_eq!(game.next_policy_player(), PlayerId(1));
    game.declare_blockers(PlayerId(1), &[CombatBlock { attacker, blocker }])
        .expect("Master Warcraft controller chooses defender's legal block");
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::AttackersDeclared { player, attackers }
            if *player == PlayerId(1) && attackers == &vec![attacker]
    )));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::BlockersDeclared { player, assignments }
            if *player == PlayerId(1) && assignments == &vec![(attacker, blocker)]
    )));
    game.validate_invariants()
        .expect("combat authority is coherent");
}
