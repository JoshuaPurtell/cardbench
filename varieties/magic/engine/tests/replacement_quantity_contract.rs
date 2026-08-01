//! Contracts for expansion-neutral token and counter quantity replacements.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, Game, ManaCost, ReplacementEffect, ReplacementEffectBinding,
    RulesError,
};

const REPLACER: &str = "TEST-REPLACER";
const NONPERMANENT: &str = "TEST-NONPERMANENT";

fn definition(id: &'static str, card_type: CardType) -> CardDefinition {
    CardDefinition {
        id,
        name: id,
        set_code: "TST",
        mana_cost: ManaCost::new(0),
        colors: BTreeSet::new(),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([card_type]),
        is_basic_land: false,
        supported_rules: &["fixture"],
        power: None,
        toughness: None,
        keywords: vec![],
        effects: vec![],
    }
}

#[test]
fn replacement_registration_requires_live_permanent_sources_positive_multipliers_and_setup_time() {
    let mut game = Game::new(
        [
            definition(REPLACER, CardType::Enchantment),
            definition(NONPERMANENT, CardType::Instant),
        ],
        2,
    )
    .expect("fixture game constructs");

    let invalid_multiplier =
        game.register_replacement_effect_bindings([ReplacementEffectBinding {
            source_definition: REPLACER,
            effect: ReplacementEffect::MultiplyTokenCreation { multiplier: 1 },
        }]);
    assert!(matches!(
        invalid_multiplier,
        Err(RulesError::IllegalAction(_))
    ));

    let nonpermanent_source =
        game.register_replacement_effect_bindings([ReplacementEffectBinding {
            source_definition: NONPERMANENT,
            effect: ReplacementEffect::MultiplyCounterPlacement { multiplier: 2 },
        }]);
    assert!(matches!(
        nonpermanent_source,
        Err(RulesError::IllegalAction(_))
    ));

    game.register_replacement_effect_bindings([
        ReplacementEffectBinding {
            source_definition: REPLACER,
            effect: ReplacementEffect::MultiplyTokenCreation { multiplier: 2 },
        },
        ReplacementEffectBinding {
            source_definition: REPLACER,
            effect: ReplacementEffect::MultiplyCounterPlacement { multiplier: 2 },
        },
    ])
    .expect("two distinct replacements register");
    game.validate_invariants()
        .expect("registered replacement definitions are invariant-safe");

    game.begin_game().expect("fixture game begins");
    let late_registration = game.register_replacement_effect_bindings([ReplacementEffectBinding {
        source_definition: REPLACER,
        effect: ReplacementEffect::MultiplyTokenCreation { multiplier: 3 },
    }]);
    assert!(matches!(
        late_registration,
        Err(RulesError::IllegalAction(_))
    ));
    game.validate_invariants()
        .expect("rejected late registration is atomic");
}
