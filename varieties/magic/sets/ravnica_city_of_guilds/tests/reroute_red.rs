//! Red regression for the public Reroute semantic slice.
//!
//! The green milestone must model an activated-ability stack item as a typed
//! retargetable object. A source permanent is not enough: two activations of
//! one source can coexist on the stack.

use cardbench_magic_engine::{CardType, Color, HybridManaSymbol, ManaCost};
use cardbench_magic_rav::{RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions};

fn definition(id: &str) -> cardbench_magic_engine::CardDefinition {
    card_definitions()
        .into_iter()
        .find(|definition| definition.id == id)
        .unwrap_or_else(|| panic!("{id} definition exists"))
}

#[test]
fn reroute_is_a_full_fidelity_hybrid_instant() {
    let reroute = definition("RAV-REROUTE");
    assert_eq!(
        reroute.mana_cost,
        ManaCost::with_hybrid(
            0,
            [],
            [HybridManaSymbol {
                first: Color::Blue,
                second: Color::Red,
            }],
        )
    );
    assert_eq!(
        reroute.card_types,
        [CardType::Instant].into_iter().collect()
    );
    assert!(
        RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&reroute.id),
        "Reroute requires a resolution-time single-target activated-ability retarget choice"
    );
}
