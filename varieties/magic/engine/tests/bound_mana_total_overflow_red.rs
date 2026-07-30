//! Red regression: mana accounting must not panic when valid per-color pools are large.

use std::collections::BTreeSet;
use std::panic::{AssertUnwindSafe, catch_unwind};

use cardbench_magic_engine::{
    ActivatedManaAbility, CardDefinition, CardType, Color, Game, ManaAbilityActivation,
    ManaAbilityBinding, ManaAbilityOutput, ManaBundle, ManaCost, PlayerId,
};

const SIGNET: &str = "TEST-HIGH-POOL-SIGNET";

fn artifact() -> CardDefinition {
    CardDefinition {
        id: SIGNET,
        name: "High Pool Signet",
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

#[test]
fn paid_mana_bundle_does_not_panic_when_generic_payment_sums_large_color_pools() {
    let player = PlayerId(0);
    let mut game = Game::new_with_mana_abilities(
        vec![artifact()],
        2,
        [ManaAbilityBinding {
            card_definition: SIGNET,
            ability: ActivatedManaAbility {
                id: "pay-one-produce-ur",
                tap_cost: true,
                output: ManaAbilityOutput::PaidBundle {
                    mana_cost: ManaCost::new(1),
                    bundle: ManaBundle::new([(Color::White, 1), (Color::Green, 1)]),
                },
                amount: 0,
                life_payment: None,
            },
        }],
    )
    .expect("valid binding initializes");
    let source = game
        .put_on_battlefield(player, SIGNET)
        .expect("source begins on the battlefield");
    game.begin_game().expect("fixture reaches upkeep priority");

    // Both per-color quantities are valid u8 values. The public mana-action
    // seam accepts them, and the combined pool is sufficient to pay {1}.
    game.add_mana_from_action(player, Color::Blue, u8::MAX)
        .expect("blue mana action succeeds");
    game.add_mana_from_action(player, Color::Red, u8::MAX)
        .expect("red mana action succeeds");
    game.clear_event_log();

    let result = catch_unwind(AssertUnwindSafe(|| {
        game.activate_bound_mana_ability(
            player,
            ManaAbilityActivation {
                source,
                ability_id: "pay-one-produce-ur",
                chosen_color: None,
            },
        )
    }));

    assert!(
        result.is_ok(),
        "a valid generic payment panicked instead of producing mana; events: {:?}",
        game.event_log
    );
    result
        .expect("the activation must not panic")
        .expect("the large valid pool can pay the ability cost");
    game.validate_invariants()
        .expect("a completed mana ability preserves the state machine");
}
