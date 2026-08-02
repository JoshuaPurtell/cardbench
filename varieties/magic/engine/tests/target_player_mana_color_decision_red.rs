//! Red regression for a target player's resolution-time colored-mana choice.
//!
//! This is intentionally synthetic: it protects the shared decision/stack
//! substrate needed by targeted mana artifacts without encoding card text.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, CastRequest, Color, DecisionKind, DecisionSelection, Effect, Game,
    GameEvent, ManaCost, PlayerId, Target, Zone,
};

const TARGETED_MANA: &str = "TST-TARGETED-MANA";

#[test]
fn target_player_must_choose_the_colored_mana_they_receive_at_resolution() {
    let mut game = Game::new(
        [CardDefinition {
            id: TARGETED_MANA,
            name: TARGETED_MANA,
            set_code: "TST",
            mana_cost: ManaCost::new(0),
            colors: BTreeSet::new(),
            mana_colors: BTreeSet::new(),
            card_types: BTreeSet::from([CardType::Instant]),
            is_basic_land: false,
            supported_rules: &["target-player-mana-color-decision"],
            power: None,
            toughness: None,
            keywords: vec![],
            effects: vec![Effect::AddOneManaOfTargetPlayersChosenColor],
        }],
        2,
    )
    .expect("fixture requires target-player mana-choice substrate");
    let spell = game
        .add_card(PlayerId(0), TARGETED_MANA, Zone::Hand)
        .expect("spell setup");
    game.begin_game().expect("game begins");
    game.cast_spell(
        PlayerId(0),
        CastRequest {
            card: spell,
            targets: vec![Target::Player(PlayerId(1))],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("targeted mana spell casts");
    game.pass_priority(PlayerId(0)).expect("caster passes");
    game.pass_priority(PlayerId(1))
        .expect("recipient passes to resolution");

    let decision = game
        .view_for_player(PlayerId(1))
        .expect("recipient view")
        .pending_decision
        .expect("recipient must choose colored mana");
    assert_eq!(decision.kind, DecisionKind::TargetPlayerManaColor);
    assert_eq!(decision.min_selections, 1);
    assert_eq!(decision.max_selections, 1);
    assert_eq!(decision.color_candidates, Color::ALL);
    assert!(
        game.submit_decision(
            PlayerId(0),
            decision.id,
            DecisionSelection::Color(Color::Red)
        )
        .is_err(),
        "only the target player may choose their mana color"
    );

    game.submit_decision(
        PlayerId(1),
        decision.id,
        DecisionSelection::Color(Color::Blue),
    )
    .expect("recipient chooses blue mana");
    assert_eq!(game.players[1].mana_pool.amount(Color::Blue), 1);
    assert_eq!(game.players[1].mana_pool.amount(Color::Red), 0);
    let opened = game
        .event_log
        .iter()
        .position(|event| matches!(event, GameEvent::DecisionOpened { kind: DecisionKind::TargetPlayerManaColor, player, .. } if *player == PlayerId(1)))
        .expect("decision-opened receipt");
    let added = game
        .event_log
        .iter()
        .position(|event| matches!(event, GameEvent::ManaAdded { player, color: Color::Blue, amount: 1 } if *player == PlayerId(1)))
        .expect("target-player mana receipt");
    assert!(
        opened < added,
        "choice receipt precedes selected mana output"
    );
    eprintln!(
        "target_player_mana_color_trace={:?}",
        game.canonical_event_log()
    );
    game.validate_invariants()
        .expect("target-player color choice preserves the stack decision boundary");
}
