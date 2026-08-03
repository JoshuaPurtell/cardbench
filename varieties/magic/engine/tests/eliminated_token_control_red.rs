//! Regression: CR 800.4a ends temporary control before it decides whether a
//! departing player still controls an opponent's token.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, CastRequest, ContinuousChange, Duration, Effect, Game, GameEvent,
    ManaCost, PlayerId, Zone,
};

const TOKEN_MAKER: &str = "TST-DEPARTING-CONTROLLER-TOKEN-MAKER";
const CONTROL_SOURCE: &str = "TST-DEPARTING-CONTROLLER-SOURCE";

fn definition(
    id: &'static str,
    card_types: impl IntoIterator<Item = CardType>,
    effects: Vec<Effect>,
) -> CardDefinition {
    CardDefinition {
        id,
        name: id,
        set_code: "TST",
        mana_cost: ManaCost::new(0),
        colors: BTreeSet::new(),
        mana_colors: BTreeSet::new(),
        card_types: card_types.into_iter().collect(),
        is_basic_land: false,
        supported_rules: &["synthetic-token-departure-contract"],
        power: None,
        toughness: None,
        keywords: vec![],
        effects,
    }
}

#[test]
fn token_controlled_by_departing_player_reverts_to_owner_before_cleanup() {
    let owner = PlayerId(0);
    let departing_controller = PlayerId(1);
    let surviving_effect_controller = PlayerId(2);
    let mut game = Game::new(
        [
            definition(
                TOKEN_MAKER,
                [CardType::Instant],
                vec![Effect::CreateToken {
                    token: cardbench_magic_engine::TokenSpec::saproling(),
                    count: 1,
                }],
            ),
            definition(CONTROL_SOURCE, [CardType::Artifact], vec![]),
        ],
        3,
    )
    .expect("three-player fixture initializes");
    let token_maker = game
        .add_card(owner, TOKEN_MAKER, Zone::Hand)
        .expect("owner receives token spell");
    let control_source = game
        .put_on_battlefield(surviving_effect_controller, CONTROL_SOURCE)
        .expect("surviving source enters");

    game.begin_game().expect("game starts");
    game.cast_spell(
        owner,
        CastRequest {
            card: token_maker,
            targets: vec![],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("token spell casts");
    for player in [owner, departing_controller, surviving_effect_controller] {
        game.pass_priority(player)
            .expect("every player passes the token spell");
    }
    let token = game
        .event_log
        .iter()
        .find_map(|event| match event {
            GameEvent::TokenCreated { token, .. } => Some(*token),
            _ => None,
        })
        .expect("spell creates one token");
    game.add_continuous_effect(
        control_source,
        token,
        ContinuousChange::ChangeController(departing_controller),
        Duration::Permanent,
    )
    .expect("surviving source gives the departing player control of the token");

    game.players[departing_controller.0].life = 0;
    game.check_state_based_actions()
        .expect("player-loss SBA itself should complete");

    assert_eq!(game.zone_of(token), Some(Zone::Battlefield));
    assert_eq!(game.controller_of(token), Ok(owner));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::ContinuousEffectExpired { source, target, layer }
            if *source == control_source
                && *target == token
                && *layer == cardbench_magic_engine::Layer::Control
    )));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::ControllerChanged { target, from, to, .. }
            if *target == token && *from == departing_controller && *to == owner
    )));
    assert!(
        !game.event_log.iter().any(|event| {
            matches!(event, GameEvent::CardMoved { card, to: Zone::Exile } if *card == token)
        }),
        "reverted temporary control cannot fabricate an owner-zone exile; events: {:?}",
        game.canonical_event_log()
    );
    assert!(!game.event_log.iter().any(|event| {
        matches!(event, GameEvent::TokenCeasedToExist { token: ceased } if *ceased == token)
    }));
    game.validate_invariants()
        .expect("player-loss cleanup must not retain temporary control for a departed seat");
}
