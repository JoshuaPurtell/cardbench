//! Red regression for mana abilities used while a spell cost is being paid.
//!
//! This deliberately uses generic engine definitions instead of retaining any
//! card rules prose. A paid fixed bundle has enough output to cast the spell,
//! but its own one-mana activation cost must be paid first.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    ActivatedManaAbility, CardDefinition, CardType, CastRequest, Color, Game, GameEvent,
    ManaAbilityActivation, ManaAbilityBinding, ManaAbilityOutput, ManaBundle, ManaCost, PlayerId,
    RulesError, Zone,
};

const SIGNET: &str = "TEST-CAST-PAYMENT-SIGNET";
const SPELL: &str = "TEST-CAST-PAYMENT-SPELL";

fn artifact(id: &'static str, mana_cost: ManaCost) -> CardDefinition {
    CardDefinition {
        id,
        name: id,
        set_code: "TST",
        mana_cost,
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

fn game_with_spell_cost(spell_cost: ManaCost) -> Game {
    Game::new_with_mana_abilities(
        vec![
            artifact(SIGNET, ManaCost::new(0)),
            artifact(SPELL, spell_cost),
        ],
        2,
        [ManaAbilityBinding {
            card_definition: SIGNET,
            ability: ActivatedManaAbility {
                id: "two-color-bundle",
                tap_cost: true,
                output: ManaAbilityOutput::PaidBundle {
                    mana_cost: ManaCost::new(1),
                    bundle: ManaBundle::new([(Color::Blue, 1), (Color::Red, 1)]),
                },
                amount: 0,
                life_payment: None,
                controller_damage: None,
            },
        }],
    )
    .expect("the test mana ability is a valid definition-bound ability")
}

#[test]
fn paid_bundle_mana_ability_can_pay_a_spell_cost_without_pre_floating() {
    let player = PlayerId(0);
    let mut game = game_with_spell_cost(ManaCost::new(2));
    let source = game
        .put_on_battlefield(player, SIGNET)
        .expect("fixture puts the mana source onto the battlefield");
    let spell = game
        .add_card(player, SPELL, Zone::Hand)
        .expect("fixture puts the spell into its owner's hand");
    game.grant_mana(player, Color::White, 1)
        .expect("fixture grants only the Signet activation payment");
    game.clear_event_log();

    assert!(
        game.cast_spell(
            player,
            CastRequest {
                card: spell,
                targets: vec![],
                convoke: vec![],
                payment_mana_abilities: vec![ManaAbilityActivation {
                    source,
                    ability_id: "two-color-bundle",
                    chosen_color: None,
                }],
            },
        )
        .is_ok(),
        "a legal definition-bound mana ability must be usable while paying this spell cost"
    );

    assert!(game.object(source).expect("source remains").tapped);
    assert_eq!(game.zone_of(spell), None, "the spell is now on the stack");
    assert_eq!(game.stack.len(), 1, "only the spell enters the stack");
    let mana_pool = &game.player(player).expect("player exists").mana_pool;
    assert_eq!(mana_pool.total_exact(), 0, "the spell consumes the bundle");
    assert_eq!(
        game.event_log,
        vec![
            cardbench_magic_engine::GameEvent::CastPaymentManaAbilityActivated {
                player,
                card: spell,
                source,
                ability: "two-color-bundle",
            },
            cardbench_magic_engine::GameEvent::BoundManaAbilityBundleActivated {
                player,
                source,
                ability: "two-color-bundle",
                mana_cost: ManaCost::new(1),
                bundle: ManaBundle::new([(Color::Blue, 1), (Color::Red, 1)]),
                tapped: true,
                life_payment: None,
            },
            cardbench_magic_engine::GameEvent::ManaAbilityManaPaid {
                player,
                mana_cost: ManaCost::new(1),
            },
            cardbench_magic_engine::GameEvent::ManaAdded {
                player,
                color: Color::Blue,
                amount: 1,
            },
            cardbench_magic_engine::GameEvent::ManaAdded {
                player,
                color: Color::Red,
                amount: 1,
            },
            cardbench_magic_engine::GameEvent::SpellCast {
                player,
                card: spell,
            },
        ],
        "payment-context, activation-cost, output, and spell receipts retain causal order"
    );
    game.validate_invariants()
        .expect("a payment-context cast preserves game-state invariants");
}

#[test]
fn failed_final_spell_payment_restores_all_payment_context_state() {
    let player = PlayerId(0);
    let mut game = game_with_spell_cost(ManaCost::new(3));
    let source = game
        .put_on_battlefield(player, SIGNET)
        .expect("fixture puts the mana source onto the battlefield");
    let spell = game
        .add_card(player, SPELL, Zone::Hand)
        .expect("fixture puts the spell into its owner's hand");
    game.grant_mana(player, Color::White, 1)
        .expect("fixture grants only the activation payment");
    game.clear_event_log();
    let before_pool = game
        .player(player)
        .expect("player exists")
        .mana_pool
        .clone();

    assert_eq!(
        game.cast_spell(
            player,
            CastRequest {
                card: spell,
                targets: vec![],
                convoke: vec![],
                payment_mana_abilities: vec![ManaAbilityActivation {
                    source,
                    ability_id: "two-color-bundle",
                    chosen_color: None,
                }],
            },
        ),
        Err(RulesError::Mana("missing generic mana".to_owned()))
    );
    assert_eq!(game.zone_of(spell), Some(Zone::Hand));
    assert!(!game.object(source).expect("source remains").tapped);
    assert_eq!(
        game.player(player).expect("player exists").mana_pool,
        before_pool
    );
    assert!(
        game.event_log.is_empty(),
        "no partial activation receipt survives"
    );
    assert!(
        game.stack.is_empty(),
        "a failed cast creates no spell stack object"
    );
    game.validate_invariants()
        .expect("failed payment-context casts preserve game-state invariants");
}

#[test]
fn ordered_payment_activations_can_spend_earlier_mana_output() {
    let player = PlayerId(0);
    let mut game = game_with_spell_cost(ManaCost::new(3));
    let first_source = game
        .put_on_battlefield(player, SIGNET)
        .expect("first source begins on the battlefield");
    let second_source = game
        .put_on_battlefield(player, SIGNET)
        .expect("second source begins on the battlefield");
    let spell = game
        .add_card(player, SPELL, Zone::Hand)
        .expect("fixture puts the spell into its owner's hand");
    game.grant_mana(player, Color::White, 1)
        .expect("fixture grants only the first activation payment");
    game.clear_event_log();

    game.cast_spell(
        player,
        CastRequest {
            card: spell,
            targets: vec![],
            convoke: vec![],
            payment_mana_abilities: vec![
                ManaAbilityActivation {
                    source: first_source,
                    ability_id: "two-color-bundle",
                    chosen_color: None,
                },
                ManaAbilityActivation {
                    source: second_source,
                    ability_id: "two-color-bundle",
                    chosen_color: None,
                },
            ],
        },
    )
    .expect("the second activation may spend mana produced by the first");

    let contextual_sources = game
        .event_log
        .iter()
        .filter_map(|event| match event {
            GameEvent::CastPaymentManaAbilityActivated { source, .. } => Some(*source),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(contextual_sources, [first_source, second_source]);
    assert!(
        game.object(first_source)
            .expect("first source exists")
            .tapped
    );
    assert!(
        game.object(second_source)
            .expect("second source exists")
            .tapped
    );
    assert_eq!(
        game.player(player)
            .expect("player exists")
            .mana_pool
            .total_exact(),
        0
    );
    assert_eq!(game.stack.len(), 1, "only the spell enters the stack");
    game.validate_invariants()
        .expect("ordered payment-context activations preserve game-state invariants");
}

#[test]
fn failed_later_payment_activation_restores_earlier_payment_context_state() {
    let player = PlayerId(0);
    let mut game = game_with_spell_cost(ManaCost::new(2));
    let source = game
        .put_on_battlefield(player, SIGNET)
        .expect("fixture puts the mana source onto the battlefield");
    let spell = game
        .add_card(player, SPELL, Zone::Hand)
        .expect("fixture puts the spell into its owner's hand");
    game.grant_mana(player, Color::White, 1)
        .expect("fixture grants the first activation payment");
    game.clear_event_log();
    let before_pool = game
        .player(player)
        .expect("player exists")
        .mana_pool
        .clone();

    assert_eq!(
        game.cast_spell(
            player,
            CastRequest {
                card: spell,
                targets: vec![],
                convoke: vec![],
                payment_mana_abilities: vec![
                    ManaAbilityActivation {
                        source,
                        ability_id: "two-color-bundle",
                        chosen_color: None,
                    },
                    ManaAbilityActivation {
                        source,
                        ability_id: "two-color-bundle",
                        chosen_color: None,
                    },
                ],
            },
        ),
        Err(RulesError::IllegalAction(
            "mana ability requires an untapped source"
        ))
    );
    assert_eq!(game.zone_of(spell), Some(Zone::Hand));
    assert!(!game.object(source).expect("source remains").tapped);
    assert_eq!(
        game.player(player).expect("player exists").mana_pool,
        before_pool
    );
    assert!(
        game.event_log.is_empty(),
        "no earlier activation receipt survives"
    );
    assert!(
        game.stack.is_empty(),
        "a failed cast creates no spell stack object"
    );
    game.validate_invariants()
        .expect("failed later activations preserve game-state invariants");
}
