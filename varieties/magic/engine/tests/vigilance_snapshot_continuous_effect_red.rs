//! Red regression: combat retains declaration-time vigilance provenance.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, Color, ContinuousChange, DeckEntry, DeckList, Duration, Game,
    GameEvent, Keyword, ManaCost, PlayerId, Step,
};

const ATTACKER: &str = "TEST-VIGILANCE-SNAPSHOT-ATTACKER";

fn definition() -> CardDefinition {
    CardDefinition {
        id: ATTACKER,
        name: "Vigilance Snapshot Attacker",
        set_code: "TST",
        mana_cost: ManaCost::new(0),
        colors: BTreeSet::from([Color::White]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Creature]),
        is_basic_land: false,
        supported_rules: &["base-characteristics", "continuous-keyword"],
        power: Some(2),
        toughness: Some(2),
        keywords: vec![],
        effects: vec![],
    }
}

fn deck() -> DeckList {
    DeckList {
        mainboard: vec![DeckEntry {
            card: ATTACKER.to_owned(),
            count: 12,
        }],
        sideboard: vec![],
    }
}

fn advance_to_declare_attackers(game: &mut Game) {
    for _ in 0..100 {
        if game.turn >= 3
            && game.active_player == PlayerId(0)
            && game.step == Step::DeclareAttackers
        {
            return;
        }
        let view = game
            .view_for_player(game.next_policy_player())
            .expect("public game view");
        if game.step == Step::DeclareAttackers && !view.attackers_declared {
            game.declare_attackers(game.next_policy_player(), &[])
                .expect("empty attacker declaration is legal");
            continue;
        }
        if game.step == Step::DeclareBlockers && !view.blockers_declared {
            game.declare_blockers(game.next_policy_player(), &[])
                .expect("empty blocker declaration is legal");
            continue;
        }
        if view.draw_replacement_pending {
            let player = game.next_policy_player();
            game.resolve_pending_draw(player, None)
                .expect("ordinary draw is legal");
        }
        game.pass_priority(game.priority)
            .expect("each automatic step transition is legal");
    }
    panic!(
        "did not reach later player-zero declare attackers; turn={}, active={:?}, step={:?}",
        game.turn, game.active_player, game.step
    );
}

#[test]
fn losing_vigilance_after_attack_declaration_does_not_invalidate_combat_snapshot() {
    let mut game = Game::new(vec![definition()], 2).expect("fixture game builds");
    game.load_deck_into_library(PlayerId(0), &deck())
        .expect("first deck setup");
    game.load_deck_into_library(PlayerId(1), &deck())
        .expect("second deck setup");
    let attacker = game
        .put_on_battlefield(PlayerId(0), ATTACKER)
        .expect("attacker setup");
    game.begin_game().expect("game begins");
    game.add_continuous_effect(
        attacker,
        attacker,
        ContinuousChange::AddKeyword(Keyword::Vigilance),
        Duration::Permanent,
    )
    .expect("vigilance is live before attackers are declared");

    advance_to_declare_attackers(&mut game);
    game.declare_attackers(PlayerId(0), &[attacker])
        .expect("the vigilant attacker is legal and remains untapped");
    assert!(
        !game.object(attacker).expect("attacker exists").tapped,
        "vigilance prevents tapping at declaration time"
    );

    let result = game.add_continuous_effect(
        attacker,
        attacker,
        ContinuousChange::RemoveKeyword(Keyword::Vigilance),
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
        "a legal post-declaration keyword change must not reject the already declared attacker: {result:?}; event_tail={event_tail:?}",
    );
    assert!(
        !game
            .characteristics(attacker)
            .expect("attacker remains live")
            .keywords
            .contains(&Keyword::Vigilance),
        "the later effect genuinely removed vigilance"
    );
    assert!(
        !game.object(attacker).expect("attacker exists").tapped,
        "losing vigilance after declaration cannot retroactively tap an attacker"
    );
    assert_eq!(
        game.event_log
            .iter()
            .filter(|event| matches!(
                event,
                GameEvent::ContinuousEffectCreated { source, target, .. }
                    if *source == attacker && *target == attacker
            ))
            .count(),
        2,
        "the committed trace records both the declaration-time grant and later keyword removal"
    );
    game.validate_invariants()
        .expect("combat keeps declaration-time vigilance provenance");
}
