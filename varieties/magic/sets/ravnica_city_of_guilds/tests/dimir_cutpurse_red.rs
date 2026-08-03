//! Red discovery contract for Dimir Cutpurse's combat-player trigger.
//!
//! The test retains only CardBench-authored semantic identities. It does not
//! store upstream card text, images, or hidden benchmark data.

use std::collections::BTreeSet;

use cardbench_magic_engine::{CardType, Color, ManaCost};
use cardbench_magic_rav::{
    RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, executable_definition_id_for_collector,
    rav_triggered_ability_bindings,
};

#[test]
fn dimir_cutpurse_requires_combat_player_discard_then_draw_trigger() {
    let cutpurse = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-DIMIR-CUTPURSE")
        .expect("Dimir Cutpurse definition exists");

    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&cutpurse.id));
    assert_eq!(cutpurse.name, "Dimir Cutpurse");
    assert_eq!(
        cutpurse.mana_cost,
        ManaCost::with_colors(1, [Color::Blue, Color::Black])
    );
    assert_eq!(cutpurse.colors, BTreeSet::from([Color::Blue, Color::Black]));
    assert_eq!(cutpurse.card_types, BTreeSet::from([CardType::Creature]));
    assert_eq!((cutpurse.power, cutpurse.toughness), (Some(2), Some(2)));
    assert_eq!(
        executable_definition_id_for_collector(201),
        Ok("RAV-DIMIR-CUTPURSE")
    );
    assert!(
        cutpurse
            .supported_rules
            .contains(&"combat-player-trigger-private-discard-then-draw")
    );
    assert!(rav_triggered_ability_bindings().iter().any(|binding| {
        binding.card_definition == cutpurse.id
            && binding.ability.id == "combat-player-discard-then-controller-draw"
    }));
}
