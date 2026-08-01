//! Red regression for two source-audited Dimir card facts.
//!
//! The cards remain bounded Transmute compatibility slices; this probe checks
//! their printed front-face mana costs independently of that capability bound.

use cardbench_magic_engine::{Color, ManaCost};
use cardbench_magic_rav::card_definitions;

fn definition(id: &str) -> cardbench_magic_engine::CardDefinition {
    card_definitions()
        .into_iter()
        .find(|definition| definition.id == id)
        .unwrap_or_else(|| panic!("missing RAV definition {id}"))
}

#[test]
fn dimir_machinations_and_shred_memory_keep_their_front_face_costs() {
    let machinations = definition("RAV-DIMIR-MACHINATIONS");
    assert_eq!(
        machinations.mana_cost,
        ManaCost::with_colors(2, [Color::Black]),
        "Dimir Machinations is a black 2B sorcery, even while its hand-zone Transmute cost is blue"
    );
    assert_eq!(machinations.colors, [Color::Black].into());

    let shred = definition("RAV-SHRED-MEMORY");
    assert_eq!(
        shred.mana_cost,
        ManaCost::with_colors(1, [Color::Black]),
        "Shred Memory's front face costs 1B; its Transmute cost remains separate"
    );
    assert_eq!(shred.colors, [Color::Black].into());
}
