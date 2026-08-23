use cardbench_magic_engine::{CardType, Color, Game, GameEvent, Keyword, PlayerId, Step};
use cardbench_magic_rav::card_definitions;

#[test]
fn boros_swiftblade_declares_double_strike_and_assigns_two_damage_steps() {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-BOROS-SWIFTBLADE")
        .expect("Boros Swiftblade exists");
    assert_eq!(
        definition.card_types,
        [CardType::Creature].into_iter().collect()
    );
    assert!(definition.colors.contains(&Color::Red));
    assert!(definition.keywords.contains(&Keyword::DoubleStrike));

    let mut game = Game::new(card_definitions(), 2).expect("RAV catalog builds");
    let swiftblade = game
        .put_on_battlefield(PlayerId(0), "RAV-BOROS-SWIFTBLADE")
        .expect("Swiftblade enters");
    game.set_entered_turn_for_setup(swiftblade, 0)
        .expect("old fixture entry");
    game.begin_game().expect("game starts");
    while game.step != Step::DeclareAttackers {
        let priority = game.priority;
        game.pass_priority(priority).expect("advance turn");
        if game.step == Step::DeclareAttackers {
            break;
        }
    }
    game.declare_attackers(PlayerId(0), &[swiftblade])
        .expect("Swiftblade attacks");
    game.pass_priority(PlayerId(0)).expect("attacker passes");
    game.pass_priority(PlayerId(1))
        .expect("advance to blockers");
    game.declare_blockers(PlayerId(1), &[])
        .expect("no blockers");
    game.pass_priority(PlayerId(0))
        .expect("attacker passes blockers");
    game.pass_priority(PlayerId(1))
        .expect("advance to first strike");
    game.pass_priority(PlayerId(0))
        .expect("first strike priority");
    game.pass_priority(PlayerId(1))
        .expect("advance to normal damage");
    game.pass_priority(PlayerId(0))
        .expect("normal damage priority");
    game.pass_priority(PlayerId(1))
        .expect("advance beyond combat");
    let damage_receipts = game
        .event_log
        .iter()
        .filter(|event| {
            matches!(
                event,
                GameEvent::DamageDealtToPlayer {
                    source,
                    player: PlayerId(1),
                    amount: 1,
                } if *source == swiftblade
            )
        })
        .count();
    assert_eq!(damage_receipts, 2);
}
