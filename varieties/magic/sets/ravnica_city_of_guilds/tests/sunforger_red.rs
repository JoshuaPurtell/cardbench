//! Red discovery contract for Sunforger's Equipment and library-cast ability.
//!
//! This intentionally names the observable card contract before the engine
//! grows the search-and-cast continuation.  It must fail while Sunforger is
//! catalog-only, rather than silently treating the printed ability as a
//! generic Equipment chassis.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    AttachmentKind, CardType, Color, ManaCost, TargetRequirement,
};
use cardbench_magic_rav::{
    RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, rav_activated_ability_bindings,
    rav_attachment_bindings,
};

#[test]
fn sunforger_requires_equipment_and_red_white_library_cast_contracts() {
    let sunforger = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-SUNFORGER")
        .expect("Sunforger definition exists");

    assert_eq!(sunforger.name, "Sunforger");
    assert_eq!(sunforger.mana_cost, ManaCost::new(3));
    assert_eq!(sunforger.colors, BTreeSet::<Color>::new());
    assert_eq!(sunforger.card_types, BTreeSet::from([CardType::Artifact]));
    assert!(
        RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&sunforger.id),
        "Sunforger must not be labeled full fidelity until both its equip and library cast are live"
    );
    assert!(sunforger
        .supported_rules
        .contains(&"equip-three-and-detach-search-red-or-white-instant-mana-value-at-most-four-cast-without-mana"));

    let attachment = rav_attachment_bindings()
        .into_iter()
        .find(|binding| binding.card_definition == sunforger.id)
        .expect("Sunforger has an Equipment attachment binding");
    assert_eq!(attachment.kind, AttachmentKind::Equipment);
    assert_eq!(attachment.target, TargetRequirement::ControlledCreature);

    let equip = rav_activated_ability_bindings()
        .into_iter()
        .find(|binding| {
            binding.card_definition == sunforger.id && binding.ability.id == "equip-plus-four-plus-zero"
        })
        .expect("Sunforger has its sorcery-speed equip ability");
    assert_eq!(equip.ability.mana_cost, ManaCost::new(3));
    assert!(equip.ability.sorcery_speed);
    assert_eq!(equip.ability.targets, vec![TargetRequirement::ControlledCreature]);

    let cast = rav_activated_ability_bindings()
        .into_iter()
        .find(|binding| {
            binding.card_definition == sunforger.id
                && binding.ability.id == "red-white-detach-search-and-cast-instant"
        })
        .expect("Sunforger has its red-white detach search-and-cast ability");
    assert_eq!(
        cast.ability.mana_cost,
        ManaCost::with_colors(0, [Color::Red, Color::White])
    );
    assert!(!cast.ability.sorcery_speed);
    assert!(cast.ability.targets.is_empty());
}
