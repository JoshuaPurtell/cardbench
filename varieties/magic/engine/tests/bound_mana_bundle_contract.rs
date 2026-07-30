//! Public contracts for a paid fixed mana bundle, the substrate used by RAV Signets.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    ActivatedManaAbility, CardDefinition, CardType, Color, Game, GameEvent, ManaAbilityActivation,
    ManaAbilityBinding, ManaAbilityOutput, ManaBundle, ManaCost, PlayerId, PolicyAction,
    PolicyMoveKind, RulesError,
};

const SIGNET: &str = "TEST-IZZET-SIGNET";
const OVERFLOW_SIGNET: &str = "TEST-OVERFLOW-SIGNET";

fn artifact(id: &'static str) -> CardDefinition {
    CardDefinition {
        id,
        name: id,
        set_code: "TST",
        mana_cost: ManaCost::new(0),
        colors: BTreeSet::new(),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Artifact]),
        is_basic_land: false,
        supported_rules: &["base-characteristics"],
        power: None,
        toughness: None,
        keywords: vec![],
        effects: vec![],
    }
}

fn paid_bundle_binding(
    card_definition: &'static str,
    id: &'static str,
    mana_cost: ManaCost,
    bundle: ManaBundle,
) -> ManaAbilityBinding {
    ManaAbilityBinding {
        card_definition,
        ability: ActivatedManaAbility {
            id,
            tap_cost: true,
            output: ManaAbilityOutput::PaidBundle { mana_cost, bundle },
            amount: 0,
            life_payment: None,
            controller_damage: None,
        },
    }
}

#[test]
fn paid_bundle_mana_ability_pays_one_and_adds_izzet_colors_without_a_stack_object() {
    let first = PlayerId(0);
    let controller = PlayerId(1);
    let bundle = ManaBundle::new([(Color::Blue, 1), (Color::Red, 1)]);
    let mut game = Game::new_with_mana_abilities(
        vec![artifact(SIGNET)],
        2,
        [paid_bundle_binding(
            SIGNET,
            "signet-ur",
            ManaCost::new(1),
            bundle.clone(),
        )],
    )
    .expect("valid Signet-shaped binding initializes");
    let source = game
        .put_on_battlefield(controller, SIGNET)
        .expect("Signet begins on battlefield");
    game.begin_game().expect("fixture reaches priority");
    game.grant_mana(controller, Color::Green, 1)
        .expect("setup grants the activation payment");
    game.clear_event_log();

    game.pass_priority(first)
        .expect("first player creates a pending pass");
    game.submit_policy_move(
        controller,
        "test.bound-bundle.v1",
        PolicyAction::ActivateBoundManaAbility {
            activation: ManaAbilityActivation {
                source,
                ability_id: "signet-ur",
                chosen_color: None,
            },
        },
    )
    .expect("a Signet pays one and produces both fixed colors");

    let mana_pool = &game
        .player(controller)
        .expect("controller exists")
        .mana_pool;
    assert_eq!(mana_pool.amount(Color::Green), 0, "the {{1}} was paid");
    assert_eq!(mana_pool.amount(Color::Blue), 1);
    assert_eq!(mana_pool.amount(Color::Red), 1);
    assert!(game.stack.is_empty(), "mana abilities never use the stack");
    assert_eq!(game.priority, controller, "the activator retains priority");
    assert!(game.object(source).expect("source exists").tapped);
    assert_eq!(
        game.event_log,
        vec![
            GameEvent::PriorityPassed { player: first },
            GameEvent::BoundManaAbilityBundleActivated {
                player: controller,
                source,
                ability: "signet-ur",
                mana_cost: ManaCost::new(1),
                bundle,
                tapped: true,
                life_payment: None,
            },
            GameEvent::ManaAbilityManaPaid {
                player: controller,
                mana_cost: ManaCost::new(1),
            },
            GameEvent::ManaAdded {
                player: controller,
                color: Color::Blue,
                amount: 1,
            },
            GameEvent::ManaAdded {
                player: controller,
                color: Color::Red,
                amount: 1,
            },
            GameEvent::PolicyMoveSubmitted {
                player: controller,
                policy: "test.bound-bundle.v1".to_owned(),
                kind: PolicyMoveKind::ActivateBoundManaAbility,
            },
        ]
    );
    game.validate_invariants()
        .expect("Signet-shaped activation preserves invariants");
}

#[test]
fn paid_bundle_preserves_every_fixed_multi_unit_output_amount() {
    let player = PlayerId(0);
    let bundle = ManaBundle::new([(Color::Blue, 2), (Color::Red, 3)]);
    let mut game = Game::new_with_mana_abilities(
        vec![artifact(SIGNET)],
        2,
        [paid_bundle_binding(
            SIGNET,
            "multi-unit-ur",
            ManaCost::new(1),
            bundle,
        )],
    )
    .expect("valid multi-unit binding initializes");
    let source = game
        .put_on_battlefield(player, SIGNET)
        .expect("source begins on battlefield");
    game.begin_game().expect("fixture reaches priority");
    game.grant_mana(player, Color::White, 1)
        .expect("setup grants the activation payment");
    game.clear_event_log();

    game.activate_bound_mana_ability(
        player,
        ManaAbilityActivation {
            source,
            ability_id: "multi-unit-ur",
            chosen_color: None,
        },
    )
    .expect("the complete fixed bundle is produced");

    let mana_pool = &game.player(player).expect("player exists").mana_pool;
    assert_eq!(mana_pool.amount(Color::White), 0);
    assert_eq!(mana_pool.amount(Color::Blue), 2);
    assert_eq!(mana_pool.amount(Color::Red), 3);
    assert_eq!(
        game.event_log
            .iter()
            .filter_map(|event| match event {
                GameEvent::ManaAdded { color, amount, .. } => Some((*color, *amount)),
                _ => None,
            })
            .collect::<Vec<_>>(),
        vec![(Color::Blue, 2), (Color::Red, 3)],
        "the event log records one precise receipt per output color"
    );
    assert!(game.stack.is_empty());
    game.validate_invariants()
        .expect("multi-unit bundle activation preserves invariants");
}

#[test]
fn paid_bundle_rejections_are_atomic_for_missing_payment_and_output_overflow() {
    let player = PlayerId(0);
    let mut game = Game::new_with_mana_abilities(
        vec![artifact(SIGNET), artifact(OVERFLOW_SIGNET)],
        2,
        [
            paid_bundle_binding(
                SIGNET,
                "signet-ur",
                ManaCost::new(1),
                ManaBundle::new([(Color::Blue, 1), (Color::Red, 1)]),
            ),
            paid_bundle_binding(
                OVERFLOW_SIGNET,
                "overflow",
                ManaCost::with_colors(0, [Color::Green]),
                ManaBundle::new([(Color::Blue, 1), (Color::Red, 2)]),
            ),
        ],
    )
    .expect("valid paid-bundle bindings initialize");
    let unpaid_source = game
        .put_on_battlefield(player, SIGNET)
        .expect("unpaid source begins on battlefield");
    let overflow_source = game
        .put_on_battlefield(player, OVERFLOW_SIGNET)
        .expect("overflow source begins on battlefield");
    game.begin_game().expect("fixture reaches priority");
    game.clear_event_log();

    let before_events = game.event_log.clone();
    let before_pool = game
        .player(player)
        .expect("player exists")
        .mana_pool
        .clone();
    assert_eq!(
        game.activate_bound_mana_ability(
            player,
            ManaAbilityActivation {
                source: unpaid_source,
                ability_id: "signet-ur",
                chosen_color: None,
            },
        ),
        Err(RulesError::Mana("missing generic mana".to_owned()))
    );
    assert_eq!(game.event_log, before_events);
    assert_eq!(
        game.player(player).expect("player exists").mana_pool,
        before_pool
    );
    assert!(!game.object(unpaid_source).expect("source exists").tapped);

    game.grant_mana(player, Color::Blue, u8::MAX)
        .expect("setup fills blue pool");
    game.grant_mana(player, Color::Green, 1)
        .expect("setup pays the colored cost");
    let before_events = game.event_log.clone();
    let before_pool = game
        .player(player)
        .expect("player exists")
        .mana_pool
        .clone();
    assert_eq!(
        game.activate_bound_mana_ability(
            player,
            ManaAbilityActivation {
                source: overflow_source,
                ability_id: "overflow",
                chosen_color: None,
            },
        ),
        Err(RulesError::IllegalAction(
            "mana pool cannot hold the produced mana"
        ))
    );
    assert_eq!(game.event_log, before_events);
    assert_eq!(
        game.player(player).expect("player exists").mana_pool,
        before_pool
    );
    assert!(!game.object(overflow_source).expect("source exists").tapped);
    game.validate_invariants()
        .expect("rejected paid-bundle activations preserve invariants");
}

#[test]
fn paid_bundle_binding_requires_a_positive_cost_and_positive_bundle_entries() {
    let no_cost = Game::new_with_mana_abilities(
        vec![artifact(SIGNET)],
        2,
        [paid_bundle_binding(
            SIGNET,
            "invalid-no-cost",
            ManaCost::new(0),
            ManaBundle::new([(Color::Blue, 1)]),
        )],
    );
    assert_eq!(
        no_cost.expect_err("paid bundle must name a positive cost"),
        RulesError::IllegalAction("paid mana bundle ability requires a positive mana cost")
    );

    let empty_bundle = Game::new_with_mana_abilities(
        vec![artifact(SIGNET)],
        2,
        [paid_bundle_binding(
            SIGNET,
            "invalid-empty-bundle",
            ManaCost::new(1),
            ManaBundle::new([]),
        )],
    );
    assert_eq!(
        empty_bundle.expect_err("paid bundle must add mana"),
        RulesError::IllegalAction("mana ability bundle must contain positive mana amounts")
    );

    let zero_entry = Game::new_with_mana_abilities(
        vec![artifact(SIGNET)],
        2,
        [paid_bundle_binding(
            SIGNET,
            "invalid-zero-entry",
            ManaCost::new(1),
            ManaBundle::new([(Color::Blue, 0)]),
        )],
    );
    assert_eq!(
        zero_entry.expect_err("a bundle entry cannot add zero mana"),
        RulesError::IllegalAction("mana ability bundle must contain positive mana amounts")
    );

    let redundant_amount = Game::new_with_mana_abilities(
        vec![artifact(SIGNET)],
        2,
        [ManaAbilityBinding {
            card_definition: SIGNET,
            ability: ActivatedManaAbility {
                id: "invalid-redundant-amount",
                tap_cost: false,
                output: ManaAbilityOutput::PaidBundle {
                    mana_cost: ManaCost::new(1),
                    bundle: ManaBundle::new([(Color::Blue, 1)]),
                },
                amount: 1,
                life_payment: None,
                controller_damage: None,
            },
        }],
    );
    assert_eq!(
        redundant_amount.expect_err("paid bundles own their output quantities"),
        RulesError::IllegalAction(
            "paid mana bundle ability must use bundle quantities instead of amount"
        )
    );
}
