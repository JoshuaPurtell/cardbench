//! Red regression: a target-player color choice must be able to suspend one
//! instruction in an otherwise ordinary activated-ability stack suffix.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    AbilityActivation, ActivatedAbility, ActivatedAbilityBinding, CardDefinition, CardType, Color,
    DecisionKind, DecisionSelection, Effect, Game, GameEvent, ManaCost, PlayerId, Target,
    TargetRequirement,
};

const SOURCE: &str = "TST-MULTI-INSTRUCTION-TARGET-PLAYER-MANA";

fn source_definition() -> CardDefinition {
    CardDefinition {
        id: SOURCE,
        name: SOURCE,
        set_code: "TST",
        mana_cost: ManaCost::new(0),
        colors: BTreeSet::from([Color::Blue]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Artifact]),
        is_basic_land: false,
        supported_rules: &["multi-instruction-target-player-mana-red"],
        power: None,
        toughness: None,
        keywords: vec![],
        effects: vec![],
    }
}

#[test]
fn target_player_mana_choice_can_appear_in_a_middle_spell_instruction() {
    let result = Game::new(
        [CardDefinition {
            id: "TST-MULTI-INSTRUCTION-TARGET-PLAYER-MANA-SPELL",
            name: "TST-MULTI-INSTRUCTION-TARGET-PLAYER-MANA-SPELL",
            set_code: "TST",
            mana_cost: ManaCost::new(0),
            colors: BTreeSet::from([Color::Blue]),
            mana_colors: BTreeSet::new(),
            card_types: BTreeSet::from([CardType::Instant]),
            is_basic_land: false,
            supported_rules: &["multi-instruction-target-player-mana-red"],
            power: None,
            toughness: None,
            keywords: vec![],
            effects: vec![
                Effect::GainLifeController { amount: 1 },
                Effect::AddOneManaOfTargetPlayersChosenColor,
                Effect::GainLifeController { amount: 2 },
            ],
        }],
        2,
    );
    assert!(
        result.is_ok(),
        "the identical resolution instruction is also valid in a spell suffix"
    );
}

#[test]
#[allow(clippy::too_many_lines)] // Covers the full prefix/choice/suffix stack lifecycle.
fn target_player_mana_choice_can_appear_in_a_middle_activated_ability_instruction() {
    let result = Game::new_with_all_bindings(
        [source_definition()],
        2,
        [],
        [],
        [],
        [ActivatedAbilityBinding {
            card_definition: SOURCE,
            ability: ActivatedAbility {
                id: "gain-choose-gain",
                mana_cost: ManaCost::new(0),
                tap_cost: false,
                sorcery_speed: false,
                additional_tap_creatures: 0,
                sacrifice_source: false,
                sacrifice_creatures: 0,
                sacrifice_lands: 0,
                discard_cards: 0,
                targets: vec![TargetRequirement::Player],
                effects: vec![
                    Effect::GainLifeController { amount: 1 },
                    Effect::AddOneManaOfTargetPlayersChosenColor,
                    Effect::GainLifeController { amount: 2 },
                ],
            },
        }],
    );

    if let Err(error) = &result {
        eprintln!("multi-instruction target-player-mana red construction: {error:?}");
    }
    assert!(
        result.is_ok(),
        "a target-player mana choice is a normal resolution instruction, not a whole-ability restriction"
    );

    let mut game = result.expect("the multi-instruction ability is constructible");
    let source = game
        .put_on_battlefield(PlayerId(0), SOURCE)
        .expect("source begins on the battlefield");
    game.begin_game().expect("game begins");
    game.activate_ability(
        PlayerId(0),
        AbilityActivation {
            source,
            ability_id: "gain-choose-gain",
            sacrifice_sources: vec![],
            additional_tap_creatures: vec![],
            discard_cards: vec![],
            targets: vec![Target::Player(PlayerId(1))],
        },
    )
    .expect("ability activates");
    game.pass_priority(PlayerId(0)).expect("controller passes");
    game.pass_priority(PlayerId(1))
        .expect("recipient passes into the middle choice");

    let decision = game
        .view_for_player(PlayerId(1))
        .expect("recipient policy view")
        .pending_decision
        .expect("recipient chooses the middle mana output");
    assert_eq!(decision.kind, DecisionKind::TargetPlayerManaColor);
    assert_eq!(
        game.player(PlayerId(0)).expect("controller exists").life,
        21
    );
    assert_eq!(game.stack.len(), 1, "the ability remains live while paused");
    assert!(
        game.submit_decision(
            PlayerId(0),
            decision.id,
            DecisionSelection::Color(Color::Red),
        )
        .is_err(),
        "the resolving controller cannot choose the target player's mana"
    );
    game.submit_decision(
        PlayerId(1),
        decision.id,
        DecisionSelection::Color(Color::Green),
    )
    .expect("recipient chooses green");

    assert_eq!(
        game.player(PlayerId(0)).expect("controller exists").life,
        23
    );
    assert_eq!(game.players[1].mana_pool.amount(Color::Green), 1);
    assert_eq!(game.stack.len(), 0, "ability resolves to completion");
    let events = &game.event_log;
    let prefix = events
        .iter()
        .position(|event| {
            matches!(event, GameEvent::LifeGained { player, amount } if *player == PlayerId(0) && *amount == 1)
        })
        .expect("prefix life receipt");
    let added = events
        .iter()
        .position(|event| {
            matches!(event, GameEvent::ManaAdded { player, color: Color::Green, amount: 1 } if *player == PlayerId(1))
        })
        .expect("selected mana receipt");
    let suffix = events
        .iter()
        .position(|event| {
            matches!(event, GameEvent::LifeGained { player, amount } if *player == PlayerId(0) && *amount == 2)
        })
        .expect("suffix life receipt");
    assert!(prefix < added && added < suffix);
    eprintln!(
        "multi-instruction target-player-mana green trace: {:?}",
        game.canonical_event_log()
    );
    game.validate_invariants()
        .expect("middle mana materialization preserves the state machine");
}
