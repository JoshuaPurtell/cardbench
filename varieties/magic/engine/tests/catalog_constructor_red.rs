//! Red regression: a game must not be constructed with a malformed catalog.

use std::collections::BTreeSet;

use cardbench_magic_engine::{CardDefinition, CardType, Game, ManaCost};

fn malformed_basic_land() -> CardDefinition {
    CardDefinition {
        id: "TST-MALFORMED-BASIC",
        name: "Malformed basic-land probe",
        set_code: "TST",
        mana_cost: ManaCost::new(0),
        colors: BTreeSet::new(),
        mana_colors: BTreeSet::new(),
        // The basic-land flag contradicts this type line. Construction must
        // reject it before any caller can treat the catalog as usable.
        card_types: BTreeSet::from([CardType::Artifact]),
        is_basic_land: true,
        supported_rules: &["basic-mana"],
        power: None,
        toughness: None,
        keywords: vec![],
        effects: vec![],
    }
}

#[test]
fn constructor_rejects_a_catalog_that_already_violates_the_engine_invariants() {
    let result = Game::new([malformed_basic_land()], 2);
    eprintln!("malformed catalog construction result: {result:?}");
    assert!(
        result.is_err(),
        "Game::new must not return a usable game with an invalid catalog"
    );
}
