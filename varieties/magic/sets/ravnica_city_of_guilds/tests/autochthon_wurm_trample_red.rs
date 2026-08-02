//! Red regression for Autochthon Wurm's multi-block Trample fidelity.

use cardbench_magic_engine::{
    CardType, Color, CombatBlock, DecisionKind, DecisionSelection, Game, GameEvent, Keyword,
    ManaCost, PlayerId, Step,
};
use cardbench_magic_rav::{RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions};

fn advance_to_declare_blockers(game: &mut Game, attacker: cardbench_magic_engine::ObjectId) {
    while game.step != Step::DeclareAttackers {
        let player = game.priority;
        game.pass_priority(player).expect("priority advances");
    }
    game.declare_attackers(PlayerId(0), &[attacker])
        .expect("Autochthon Wurm attacks");
    while game.step != Step::DeclareBlockers {
        let player = game.priority;
        game.pass_priority(player)
            .expect("priority advances to blockers");
    }
}

#[test]
fn autochthon_wurm_requires_full_multi_block_trample_fidelity() {
    let wurm = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-AUTOCHTHON-WURM")
        .expect("Autochthon Wurm exists");

    assert_eq!(
        wurm.mana_cost,
        ManaCost::with_colors(
            10,
            [
                Color::Green,
                Color::Green,
                Color::Green,
                Color::White,
                Color::White,
            ],
        )
    );
    assert_eq!(
        wurm.colors,
        [Color::Green, Color::White].into_iter().collect()
    );
    assert_eq!(wurm.card_types, [CardType::Creature].into_iter().collect());
    assert_eq!((wurm.power, wurm.toughness), (Some(9), Some(14)));
    assert_eq!(wurm.keywords, [Keyword::Convoke, Keyword::Trample]);
    assert!(wurm.supported_rules.contains(&"full-rules-fidelity"));
    assert!(
        wurm.supported_rules
            .contains(&"multi-block-trample-combat-damage")
    );
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&wurm.id));
}

#[test]
fn autochthon_wurm_assigns_lethal_damage_in_order_then_tramples_excess() {
    let mut game = Game::new(card_definitions(), 2).expect("RAV fixture builds");
    let wurm = game
        .put_on_battlefield(PlayerId(0), "RAV-AUTOCHTHON-WURM")
        .expect("Autochthon Wurm setup");
    let first_blocker = game
        .put_on_battlefield(PlayerId(1), "RAV-BIRDS-OF-PARADISE")
        .expect("first blocker setup");
    let second_blocker = game
        .put_on_battlefield(PlayerId(1), "RAV-CAREGIVER")
        .expect("second blocker setup");
    for permanent in [wurm, first_blocker, second_blocker] {
        game.set_entered_turn_for_setup(permanent, 0)
            .expect("combat permanent is long-controlled");
    }
    game.begin_game().expect("game starts");
    advance_to_declare_blockers(&mut game, wurm);
    game.declare_blockers(
        PlayerId(1),
        &[
            CombatBlock {
                attacker: wurm,
                blocker: first_blocker,
            },
            CombatBlock {
                attacker: wurm,
                blocker: second_blocker,
            },
        ],
    )
    .expect("two blockers are legal");

    let decision = game
        .view_for_player(PlayerId(0))
        .expect("attacker view")
        .pending_decision
        .expect("multi-block combat order is pending");
    assert_eq!(decision.kind, DecisionKind::CombatDamageOrder);
    game.submit_decision(
        PlayerId(0),
        decision.id,
        DecisionSelection::Objects(vec![first_blocker, second_blocker]),
    )
    .expect("attacker submits the exact blocker order");

    for _ in 0..12 {
        if game.event_log.iter().any(|event| {
            matches!(
                event,
                GameEvent::DamageDealtToPlayer {
                    source,
                    player: PlayerId(1),
                    amount: 7,
                } if *source == wurm
            )
        }) {
            break;
        }
        let player = game.priority;
        game.pass_priority(player).expect("combat advances");
    }

    assert_eq!(game.player(PlayerId(1)).expect("defender exists").life, 13);
    for blocker in [first_blocker, second_blocker] {
        assert!(game.event_log.iter().any(|event| matches!(
            event,
            GameEvent::DamageDealtToPermanent {
                source,
                permanent,
                amount: 1,
            } if *source == wurm && *permanent == blocker
        )));
    }
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::CombatDamageOrderChosen {
            player: PlayerId(0),
            attacker,
            blockers,
        } if *attacker == wurm && blockers == &vec![first_blocker, second_blocker]
    )));
    eprintln!("Autochthon Wurm trace={:?}", game.canonical_event_log());
    game.validate_invariants()
        .expect("multi-block Trample preserves engine invariants");
}
