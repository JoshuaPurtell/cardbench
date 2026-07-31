//! Public contract for ordered mixed cast-payment receipts.
//!
//! The fixture deliberately uses expansion-neutral semantic identifiers. It
//! exercises the same typed-basic-land plus paid-bundle path used by public
//! RAV traces without duplicating card prose, artwork, or set data.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    ActivatedManaAbility, BasicLandManaAbilityActivation, BasicLandType, BasicLandTypeBinding,
    CardDefinition, CardType, CastPaymentManaAbility, CastRequest, Color, Game, GameEvent,
    ManaAbilityActivation, ManaAbilityBinding, ManaAbilityOutput, ManaBundle, ManaCost, PlayerId,
    Zone,
};

const FOREST: &str = "TEST-RECEIPT-FOREST";
const SIGNET: &str = "TEST-RECEIPT-SIGNET";
const SPELL: &str = "TEST-RECEIPT-SPELL";

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

fn forest() -> CardDefinition {
    CardDefinition {
        id: FOREST,
        name: FOREST,
        set_code: "TST",
        mana_cost: ManaCost::new(0),
        colors: BTreeSet::new(),
        mana_colors: BTreeSet::from([Color::Green]),
        card_types: BTreeSet::from([CardType::Land]),
        is_basic_land: true,
        supported_rules: &[
            "basic-land-type-line",
            "intrinsic-single-color-mana-ability",
        ],
        power: None,
        toughness: None,
        keywords: vec![],
        effects: vec![],
    }
}

fn game() -> Game {
    Game::new_with_mana_abilities_and_basic_land_types(
        [
            forest(),
            artifact(SIGNET, ManaCost::new(2)),
            artifact(SPELL, ManaCost::with_colors(0, [Color::Blue, Color::Red])),
        ],
        2,
        [ManaAbilityBinding {
            card_definition: SIGNET,
            ability: ActivatedManaAbility {
                id: "paid-blue-red-bundle",
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
        [BasicLandTypeBinding {
            card_definition: FOREST,
            land_type: BasicLandType::Forest,
        }],
    )
    .expect("the mixed-payment fixture is valid")
}

#[test]
#[allow(clippy::too_many_lines)] // The full public receipt is the contract under test.
fn mixed_basic_and_bound_cast_payment_has_one_ordered_receipt_lifecycle() {
    let player = PlayerId(0);
    let opponent = PlayerId(1);
    let mut game = game();
    let forest = game
        .put_on_battlefield(player, FOREST)
        .expect("typed basic land starts on the battlefield");
    let signet = game
        .put_on_battlefield(player, SIGNET)
        .expect("bound mana source starts on the battlefield");
    let spell = game
        .add_card(player, SPELL, Zone::Hand)
        .expect("the cast spell starts in hand");
    game.clear_event_log();

    game.cast_spell(
        player,
        CastRequest {
            card: spell,
            targets: vec![],
            convoke: vec![],
            payment_mana_abilities: vec![
                CastPaymentManaAbility::BasicLand(BasicLandManaAbilityActivation {
                    land: forest,
                    color: Color::Green,
                }),
                CastPaymentManaAbility::Bound(ManaAbilityActivation {
                    source: signet,
                    ability_id: "paid-blue-red-bundle",
                    chosen_color: None,
                }),
            ],
        },
    )
    .expect("the Forest output pays the Signet activation before its bundle pays the spell");

    game.pass_priority(player)
        .expect("caster passes after the completed cast");
    game.pass_priority(opponent)
        .expect("opponent pass resolves the spell");

    assert_eq!(
        game.event_log,
        vec![
            GameEvent::CastPaymentBasicLandManaAbilityActivated {
                player,
                card: spell,
                land: forest,
                color: Color::Green,
            },
            GameEvent::ManaAbilityActivated {
                player,
                land: forest,
                color: Color::Green,
            },
            GameEvent::ManaAdded {
                player,
                color: Color::Green,
                amount: 1,
            },
            GameEvent::CastPaymentManaAbilityActivated {
                player,
                card: spell,
                source: signet,
                ability: "paid-blue-red-bundle",
            },
            GameEvent::BoundManaAbilityBundleActivated {
                player,
                source: signet,
                ability: "paid-blue-red-bundle",
                mana_cost: ManaCost::new(1),
                bundle: ManaBundle::new([(Color::Blue, 1), (Color::Red, 1)]),
                tapped: true,
                life_payment: None,
            },
            GameEvent::ManaAbilityManaPaid {
                player,
                mana_cost: ManaCost::new(1),
            },
            GameEvent::ManaAdded {
                player,
                color: Color::Blue,
                amount: 1,
            },
            GameEvent::ManaAdded {
                player,
                color: Color::Red,
                amount: 1,
            },
            GameEvent::SpellCast {
                player,
                card: spell
            },
            GameEvent::PriorityPassed { player },
            GameEvent::PriorityPassed { player: opponent },
            GameEvent::SpellResolved { card: spell },
            GameEvent::CardMoved {
                card: spell,
                to: Zone::Battlefield,
            },
        ],
        "each payment receipt is complete before the next payment and the spell has one terminal stack lifecycle"
    );
    assert!(game.object(forest).expect("Forest remains").tapped);
    assert!(game.object(signet).expect("Signet remains").tapped);
    assert_eq!(game.zone_of(spell), Some(Zone::Battlefield));
    assert!(game.stack.is_empty());
    assert_eq!(
        game.player(player)
            .expect("player remains")
            .mana_pool
            .total_exact(),
        0,
        "the Forest is spent on the Signet and the bundle is spent on the spell"
    );
    game.validate_invariants()
        .expect("the complete mixed receipt trace preserves all engine invariants");
}
