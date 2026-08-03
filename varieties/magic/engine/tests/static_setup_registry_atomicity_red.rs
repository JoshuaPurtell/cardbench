//! Red regressions: static setup registries must not retain failed-batch prefixes.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, Game, ManaCost, RulesError, StaticCreatureSpellCostModifier,
    StaticCreatureSpellCostModifierBinding, StaticEntryRestriction, StaticEntryRestrictionBinding,
    StaticLibraryTopRevealBinding, StaticLibraryTopRevealScope,
};

const SOURCE: &str = "TST-ATOMIC-STATIC-SOURCE";

fn source_definition() -> CardDefinition {
    CardDefinition {
        id: SOURCE,
        name: SOURCE,
        set_code: "TST",
        mana_cost: ManaCost::new(2),
        colors: BTreeSet::new(),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Artifact]),
        is_basic_land: false,
        supported_rules: &["test-only-static-source"],
        power: None,
        toughness: None,
        keywords: vec![],
        effects: vec![],
    }
}

#[test]
fn rejected_static_creature_spell_modifier_batch_leaves_no_prefix_binding() {
    let mut game = Game::new([source_definition()], 2).expect("fixture initializes");
    let binding = StaticCreatureSpellCostModifierBinding {
        source_definition: SOURCE,
        modifier: StaticCreatureSpellCostModifier::OptionalAnyManaForEntryCounters,
    };

    let result = game.register_static_creature_spell_cost_modifiers([binding, binding]);
    eprintln!("rejected creature-spell-modifier batch: {result:?}");
    assert!(matches!(
        result,
        Err(RulesError::IllegalAction(
            "duplicate static creature-spell cost modifier binding"
        ))
    ));
    game.register_static_creature_spell_cost_modifiers([binding])
        .expect("a rejected batch must leave no creature-spell modifier prefix");
    game.validate_invariants().expect("repaired setup is valid");
}

#[test]
fn rejected_static_entry_restriction_batch_leaves_no_prefix_binding() {
    let mut game = Game::new([source_definition()], 2).expect("fixture initializes");
    let binding = StaticEntryRestrictionBinding {
        card_definition: SOURCE,
        restriction: StaticEntryRestriction::SourceEntersTapped,
    };

    let result = game.register_static_entry_restriction_bindings([binding, binding]);
    eprintln!("rejected static-entry-restriction batch: {result:?}");
    assert!(matches!(
        result,
        Err(RulesError::IllegalAction(
            "duplicate static entry restriction binding"
        ))
    ));
    game.register_static_entry_restriction_bindings([binding])
        .expect("a rejected batch must leave no entry-restriction prefix");
    game.validate_invariants().expect("repaired setup is valid");
}

#[test]
fn rejected_static_library_reveal_batch_leaves_no_prefix_binding() {
    let mut game = Game::new([source_definition()], 2).expect("fixture initializes");
    let binding = StaticLibraryTopRevealBinding {
        card_definition: SOURCE,
        scope: StaticLibraryTopRevealScope::SourceController,
    };

    let result = game.register_static_library_top_reveal_bindings([binding, binding]);
    eprintln!("rejected static-library-reveal batch: {result:?}");
    assert!(matches!(
        result,
        Err(RulesError::IllegalAction(
            "duplicate static library-reveal binding"
        ))
    ));
    game.register_static_library_top_reveal_bindings([binding])
        .expect("a rejected batch must leave no library-reveal prefix");
    game.validate_invariants().expect("repaired setup is valid");
}
