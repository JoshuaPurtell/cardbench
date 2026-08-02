//! Red regression for promoting Siege Wurm once its complete printed rules
//! are covered by the shared Convoke and Trample substrates.

use cardbench_magic_engine::{
    CastRequest, Color, CombatBlock, ConvokeContribution, ConvokePayment, Game, GameEvent,
    PlayerId, Step, Zone,
};
use cardbench_magic_rav::{RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions};

const SIEGE_WURM: &str = "RAV-SIEGE-WURM";
const GREEN_CREATURE: &str = "RAV-GOLGARI-BROWNSCALE";
const BLOCKER: &str = "RAV-WATCHWOLF";

fn pass_pair(game: &mut Game) {
    let first = game.priority;
    game.pass_priority(first).expect("first pass");
    let second = game.priority;
    game.pass_priority(second).expect("second pass");
}

fn advance_to(game: &mut Game, turn: u32, step: Step) {
    while game.turn != turn || game.step != step {
        if game
            .view_for_player(game.next_policy_player())
            .expect("public view")
            .draw_replacement_pending
        {
            let player = game.next_policy_player();
            game.resolve_pending_draw(player, None)
                .expect("take ordinary draw");
        }
        match game.step {
            Step::DeclareAttackers
                if !game
                    .view_for_player(game.next_policy_player())
                    .expect("combat view")
                    .attackers_declared =>
            {
                let player = game.next_policy_player();
                game.declare_attackers(player, &[])
                    .expect("empty declaration is legal");
            }
            Step::DeclareBlockers
                if !game
                    .view_for_player(game.next_policy_player())
                    .expect("combat view")
                    .blockers_declared =>
            {
                let player = game.next_policy_player();
                game.declare_blockers(player, &[])
                    .expect("empty declaration is legal");
            }
            _ => {
                let player = game.priority;
                game.pass_priority(player).expect("priority passes");
            }
        }
    }
}

#[test]
fn siege_wurm_has_complete_convoke_and_trample_fidelity() {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == SIEGE_WURM)
        .expect("Siege Wurm exists");
    assert!(
        definition.supported_rules.contains(&"full-rules-fidelity"),
        "Siege Wurm requires its complete Convoke and Trample behavior"
    );
    assert!(
        RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&definition.id),
        "full-fidelity manifest must include Siege Wurm"
    );

    let caster = PlayerId(0);
    let mut game = Game::new(card_definitions(), 2).expect("catalog validates");
    let wurm = game
        .add_card(caster, SIEGE_WURM, Zone::Hand)
        .expect("wurm starts in hand");
    let convokers = (0..7)
        .map(|_| {
            game.put_on_battlefield(caster, GREEN_CREATURE)
                .expect("green creature enters")
        })
        .collect::<Vec<_>>();
    game.begin_game().expect("game begins");

    game.cast_spell(
        caster,
        CastRequest {
            card: wurm,
            targets: vec![],
            convoke: convokers
                .iter()
                .take(5)
                .copied()
                .map(|creature| ConvokePayment {
                    creature,
                    contribution: ConvokeContribution::Generic,
                })
                .chain(convokers.iter().skip(5).copied().map(|creature| ConvokePayment {
                    creature,
                    contribution: ConvokeContribution::Color(Color::Green),
                }))
                .collect(),
            payment_mana_abilities: vec![],
        },
    )
    .expect("seven legal Convoke payments cast Siege Wurm");
    pass_pair(&mut game);

    assert_eq!(game.zone_of(wurm), Some(Zone::Battlefield));
    assert!(convokers.iter().all(|creature| {
        game.object(*creature)
            .expect("convoke creature remains")
            .tapped
    }));
    assert_eq!(
        game.event_log
            .iter()
            .filter(|event| matches!(event, GameEvent::ConvokeUsed { player, .. } if *player == caster))
            .count(),
        7
    );
    game.validate_invariants()
        .expect("Siege Wurm Convoke trace is invariant-safe");
}

#[test]
fn siege_wurm_tramples_over_a_real_rav_blocker() {
    let attacker = PlayerId(0);
    let defender = PlayerId(1);
    let mut game = Game::new(card_definitions(), 2).expect("catalog validates");
    let wurm = game
        .put_on_battlefield(attacker, SIEGE_WURM)
        .expect("Siege Wurm enters");
    let blocker = game
        .put_on_battlefield(defender, BLOCKER)
        .expect("Watchwolf enters");
    for player in [attacker, defender] {
        for _ in 0..8 {
            game.add_card(player, BLOCKER, Zone::Library)
                .expect("library card enters");
        }
    }
    game.begin_game().expect("game begins");
    advance_to(&mut game, 3, Step::DeclareAttackers);
    assert_eq!(game.step, Step::DeclareAttackers);
    game.declare_attackers(attacker, &[wurm])
        .expect("Siege Wurm attacks");
    pass_pair(&mut game);
    assert_eq!(game.step, Step::DeclareBlockers);
    game.declare_blockers(defender, &[CombatBlock { attacker: wurm, blocker }])
        .expect("Watchwolf blocks");
    pass_pair(&mut game);

    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::DamageDealtToPermanent { source, permanent, amount: 3 }
            if *source == wurm && *permanent == blocker
    )));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::DamageDealtToPlayer { source, player, amount: 2 }
            if *source == wurm && *player == defender
    )));
    assert_eq!(game.player(defender).expect("defender").life, 18);
    game.validate_invariants()
        .expect("Siege Wurm Trample trace is invariant-safe");
}
