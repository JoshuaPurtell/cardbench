//! Public contract coverage for definition-bound activated mana abilities.
//!
//! These fixtures use synthetic identifiers only. They exercise the shared
//! substrate without making any claim about an expansion's card catalog.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    ActivatedManaAbility, CardDefinition, CardType, Color, Game, GameEvent, ManaAbilityActivation,
    ManaAbilityBinding, ManaAbilityOutput, ManaCost, PlayerId, PolicyAction, PolicyMoveKind,
    RulesError, Zone,
};

const RELIC: &str = "TEST-MANA-RELIC";
const ADEPT: &str = "TEST-MANA-ADEPT";
const VANILLA: &str = "TEST-VANILLA";

fn colors(colors: impl IntoIterator<Item = Color>) -> BTreeSet<Color> {
    colors.into_iter().collect()
}

fn types(types: impl IntoIterator<Item = CardType>) -> BTreeSet<CardType> {
    types.into_iter().collect()
}

fn artifact(id: &'static str) -> CardDefinition {
    CardDefinition {
        id,
        name: id,
        set_code: "TST",
        mana_cost: ManaCost::new(0),
        colors: BTreeSet::new(),
        mana_colors: BTreeSet::new(),
        card_types: types([CardType::Artifact]),
        is_basic_land: false,
        supported_rules: &["base-characteristics"],
        power: None,
        toughness: None,
        keywords: vec![],
        effects: vec![],
    }
}

fn creature(id: &'static str) -> CardDefinition {
    CardDefinition {
        id,
        name: id,
        set_code: "TST",
        mana_cost: ManaCost::new(0),
        colors: BTreeSet::new(),
        mana_colors: BTreeSet::new(),
        card_types: types([CardType::Creature]),
        is_basic_land: false,
        supported_rules: &["base-characteristics"],
        power: Some(1),
        toughness: Some(1),
        keywords: vec![],
        effects: vec![],
    }
}

fn definitions() -> Vec<CardDefinition> {
    vec![artifact(RELIC), creature(ADEPT), artifact(VANILLA)]
}

fn binding(
    card_definition: &'static str,
    id: &'static str,
    tap_cost: bool,
    output: ManaAbilityOutput,
    amount: u8,
    life_payment: Option<u8>,
) -> ManaAbilityBinding {
    ManaAbilityBinding {
        card_definition,
        ability: ActivatedManaAbility {
            id,
            tap_cost,
            output,
            amount,
            life_payment,
        },
    }
}

#[test]
fn bound_mana_ability_resets_passes_retains_priority_and_never_uses_the_stack() {
    let first = PlayerId(0);
    let controller = PlayerId(1);
    let mut game = Game::new_with_mana_abilities(
        definitions(),
        2,
        [binding(
            RELIC,
            "fixed-blue",
            true,
            ManaAbilityOutput::Fixed(Color::Blue),
            2,
            None,
        )],
    )
    .expect("valid generic ability catalog initializes");
    let source = game
        .put_on_battlefield(controller, RELIC)
        .expect("source begins on the battlefield");
    game.begin_game().expect("fixture reaches upkeep priority");
    game.clear_event_log();

    game.pass_priority(first)
        .expect("first player creates one pending pass");
    game.submit_policy_move(
        controller,
        "test.bound-mana.v1",
        PolicyAction::ActivateBoundManaAbility {
            activation: ManaAbilityActivation {
                source,
                ability_id: "fixed-blue",
                chosen_color: None,
            },
        },
    )
    .expect("the controller activates its definition-bound mana ability");

    assert!(
        game.stack.is_empty(),
        "a mana ability cannot enter the stack"
    );
    assert_eq!(game.priority, controller, "activator retains priority");
    assert!(game.object(source).expect("source exists").tapped);
    assert_eq!(
        game.player(controller)
            .expect("controller exists")
            .mana_pool
            .amount(Color::Blue),
        2
    );
    assert_eq!(
        game.event_log,
        vec![
            GameEvent::PriorityPassed { player: first },
            GameEvent::BoundManaAbilityActivated {
                player: controller,
                source,
                ability: "fixed-blue",
                color: Color::Blue,
                amount: 2,
                tapped: true,
                life_payment: None,
            },
            GameEvent::ManaAdded {
                player: controller,
                color: Color::Blue,
                amount: 2,
            },
            GameEvent::PolicyMoveSubmitted {
                player: controller,
                policy: "test.bound-mana.v1".to_owned(),
                kind: PolicyMoveKind::ActivateBoundManaAbility,
            },
        ]
    );

    game.pass_priority(controller)
        .expect("the activator may pass after the mana ability");
    assert_eq!(game.step, cardbench_magic_engine::Step::Upkeep);
    assert_eq!(
        game.priority, first,
        "the prior pass was reset rather than advancing the step"
    );
    game.validate_invariants()
        .expect("successful bound-mana transition preserves invariants");
}

#[test]
fn choice_and_life_payment_are_explicit_and_rejections_are_atomic() {
    let player = PlayerId(0);
    let mut game = Game::new_with_mana_abilities(
        definitions(),
        2,
        [binding(
            RELIC,
            "life-choice",
            false,
            ManaAbilityOutput::Choice(colors([Color::Black, Color::Green])),
            1,
            Some(2),
        )],
    )
    .expect("valid choice ability catalog initializes");
    let source = game
        .put_on_battlefield(player, RELIC)
        .expect("source begins on the battlefield");
    game.begin_game().expect("fixture reaches upkeep priority");
    game.clear_event_log();

    let before_events = game.event_log.clone();
    let before_life = game.player(player).expect("player exists").life;
    let rejected = game.activate_bound_mana_ability(
        player,
        ManaAbilityActivation {
            source,
            ability_id: "life-choice",
            chosen_color: None,
        },
    );
    assert_eq!(
        rejected,
        Err(RulesError::IllegalAction(
            "mana ability requires one supported color choice"
        ))
    );
    assert_eq!(game.event_log, before_events);
    assert_eq!(
        game.player(player).expect("player exists").life,
        before_life
    );
    assert!(!game.object(source).expect("source exists").tapped);

    game.activate_bound_mana_ability(
        player,
        ManaAbilityActivation {
            source,
            ability_id: "life-choice",
            chosen_color: Some(Color::Green),
        },
    )
    .expect("one allowed color can be selected explicitly");
    assert_eq!(game.player(player).expect("player exists").life, 18);
    assert_eq!(
        game.player(player)
            .expect("player exists")
            .mana_pool
            .amount(Color::Green),
        1
    );
    assert_eq!(
        game.event_log,
        vec![
            GameEvent::BoundManaAbilityActivated {
                player,
                source,
                ability: "life-choice",
                color: Color::Green,
                amount: 1,
                tapped: false,
                life_payment: Some(2),
            },
            GameEvent::ManaAbilityLifePaid { player, amount: 2 },
            GameEvent::ManaAdded {
                player,
                color: Color::Green,
                amount: 1,
            },
        ]
    );
    game.validate_invariants()
        .expect("choice-and-payment activation preserves invariants");
}

#[test]
fn creature_tap_cost_observes_summoning_sickness_without_state_changes() {
    let player = PlayerId(0);
    let mut game = Game::new_with_mana_abilities(
        definitions(),
        2,
        [binding(
            ADEPT,
            "tap-green",
            true,
            ManaAbilityOutput::Fixed(Color::Green),
            1,
            None,
        )],
    )
    .expect("valid creature ability catalog initializes");
    let source = game
        .put_on_battlefield(player, ADEPT)
        .expect("creature begins on the battlefield");
    game.begin_game().expect("fixture reaches upkeep priority");
    game.clear_event_log();

    let result = game.activate_bound_mana_ability(
        player,
        ManaAbilityActivation {
            source,
            ability_id: "tap-green",
            chosen_color: None,
        },
    );
    assert_eq!(
        result,
        Err(RulesError::IllegalAction(
            "a summoning-sick creature cannot pay a tap mana-ability cost"
        ))
    );
    assert!(game.event_log.is_empty());
    assert!(!game.object(source).expect("source exists").tapped);
    assert_eq!(
        game.player(player)
            .expect("player exists")
            .mana_pool
            .amount(Color::Green),
        0
    );
    game.validate_invariants()
        .expect("rejected summoning-sick activation is atomic");
}

#[test]
fn life_payment_can_end_the_game_and_terminal_rejection_is_atomic() {
    let player = PlayerId(0);
    let opponent = PlayerId(1);
    let mut game = Game::new_with_mana_abilities(
        definitions(),
        2,
        [binding(
            RELIC,
            "dangerous-red",
            false,
            ManaAbilityOutput::Fixed(Color::Red),
            1,
            Some(20),
        )],
    )
    .expect("valid life-payment catalog initializes");
    let source = game
        .put_on_battlefield(player, RELIC)
        .expect("source begins on the battlefield");
    game.begin_game().expect("fixture reaches upkeep priority");
    game.clear_event_log();

    game.activate_bound_mana_ability(
        player,
        ManaAbilityActivation {
            source,
            ability_id: "dangerous-red",
            chosen_color: None,
        },
    )
    .expect("a player may pay exactly their current life");
    assert!(game.is_game_over());
    assert_eq!(game.winner(), Some(opponent));
    assert!(game.event_log.iter().any(|event| {
        matches!(
            event,
            GameEvent::ManaAbilityLifePaid {
                player: paid_player,
                amount: 20,
            } if *paid_player == player
        )
    }));
    assert!(matches!(
        game.event_log.last(),
        Some(GameEvent::GameEnded { .. })
    ));

    let events_before = game.event_log.clone();
    let rejected = game.activate_bound_mana_ability(
        player,
        ManaAbilityActivation {
            source,
            ability_id: "dangerous-red",
            chosen_color: None,
        },
    );
    assert_eq!(
        rejected,
        Err(RulesError::IllegalAction("the game has already ended"))
    );
    assert_eq!(game.event_log, events_before);
    game.validate_invariants()
        .expect("terminal activation rejection preserves invariants");
}

#[test]
fn invalid_or_duplicate_bindings_are_rejected_before_any_game_state_exists() {
    let invalid = Game::new_with_mana_abilities(
        definitions(),
        2,
        [binding(
            RELIC,
            "",
            false,
            ManaAbilityOutput::Fixed(Color::White),
            0,
            Some(0),
        )],
    );
    assert_eq!(
        invalid.expect_err("invalid binding must be rejected"),
        RulesError::IllegalAction("mana ability lacks an identity")
    );

    let duplicate = Game::new_with_mana_abilities(
        definitions(),
        2,
        [
            binding(
                RELIC,
                "same-id",
                false,
                ManaAbilityOutput::Fixed(Color::White),
                1,
                None,
            ),
            binding(
                RELIC,
                "same-id",
                false,
                ManaAbilityOutput::Fixed(Color::Blue),
                1,
                None,
            ),
        ],
    );
    assert_eq!(
        duplicate.expect_err("duplicate binding must be rejected"),
        RulesError::IllegalAction("duplicate mana ability id for card definition")
    );

    let nonpermanent = Game::new_with_mana_abilities(
        vec![CardDefinition {
            id: "TEST-NONPERMANENT",
            name: "TEST-NONPERMANENT",
            set_code: "TST",
            mana_cost: ManaCost::new(0),
            colors: BTreeSet::new(),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Instant]),
            is_basic_land: false,
            supported_rules: &["test-effect"],
            power: None,
            toughness: None,
            keywords: vec![],
            effects: vec![],
        }],
        2,
        [binding(
            "TEST-NONPERMANENT",
            "bad-source",
            false,
            ManaAbilityOutput::Fixed(Color::White),
            1,
            None,
        )],
    );
    assert_eq!(
        nonpermanent.expect_err("nonpermanent binding must be rejected"),
        RulesError::IllegalAction("a definition-bound mana ability requires a permanent source")
    );
}

#[test]
fn a_bound_ability_source_must_be_on_the_battlefield() {
    let player = PlayerId(0);
    let mut game = Game::new_with_mana_abilities(
        definitions(),
        2,
        [binding(
            RELIC,
            "hand-blue",
            false,
            ManaAbilityOutput::Fixed(Color::Blue),
            1,
            None,
        )],
    )
    .expect("valid catalog initializes");
    let source = game
        .add_card(player, RELIC, Zone::Hand)
        .expect("source card begins in hand");
    game.begin_game().expect("fixture reaches upkeep priority");
    game.clear_event_log();

    assert_eq!(
        game.activate_bound_mana_ability(
            player,
            ManaAbilityActivation {
                source,
                ability_id: "hand-blue",
                chosen_color: None,
            },
        ),
        Err(RulesError::WrongZone {
            card: source,
            expected: Zone::Battlefield,
        })
    );
    assert!(game.event_log.is_empty());
    game.validate_invariants()
        .expect("wrong-zone rejection preserves invariants");
}
