//! Red regression for mana abilities used while a spell cost is being paid.
//!
//! This deliberately uses generic engine definitions instead of retaining any
//! card rules prose. A paid fixed bundle has enough output to cast the spell,
//! but its own one-mana activation cost must be paid first.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    ActivatedManaAbility, CardDefinition, CardType, CastRequest, Color, Game, ManaAbilityBinding,
    ManaAbilityOutput, ManaBundle, ManaCost, PlayerId, Zone,
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

#[test]
fn paid_bundle_mana_ability_can_pay_a_spell_cost_without_pre_floating() {
    let player = PlayerId(0);
    let mut game = Game::new_with_mana_abilities(
        vec![
            artifact(SIGNET, ManaCost::new(0)),
            artifact(SPELL, ManaCost::new(2)),
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
    .expect("the test mana ability is a valid definition-bound ability");
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
            },
        )
        .is_ok(),
        "a legal definition-bound mana ability must be usable while paying this spell cost; source={source:?}"
    );
}
