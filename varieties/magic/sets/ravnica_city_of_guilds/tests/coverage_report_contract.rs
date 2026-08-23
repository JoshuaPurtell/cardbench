use std::collections::BTreeSet;

use cardbench_magic_rav::{
    CardSemanticStatus, RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions,
    rav_activated_ability_bindings, rav_activated_ability_cost_modifier_bindings,
    rav_additional_spell_cost_bindings, rav_attachment_bindings,
    rav_attachment_triggered_ability_bindings, rav_basic_land_type_bindings,
    rav_cost_reduction_bindings, rav_damage_replacement_effect_bindings,
    rav_entry_coin_flip_bindings, rav_entry_copy_bindings,
    rav_generalized_activated_ability_cost_bindings, rav_land_entry_bindings,
    rav_legendary_permanent_bindings, rav_main_set_catalog, rav_mana_ability_bindings,
    rav_mana_ability_cost_bindings, rav_replacement_effect_bindings,
    rav_static_attack_restriction_bindings, rav_static_continuous_effect_bindings,
    rav_static_creature_spell_cost_modifier_bindings, rav_static_entry_restriction_bindings,
    rav_static_library_top_reveal_bindings, rav_triggered_ability_bindings,
};

#[test]
fn coverage_categories_partition_every_printing_and_unique_name() {
    let full_ids = RAV_FULL_FIDELITY_DEFINITION_IDS
        .into_iter()
        .collect::<BTreeSet<_>>();
    let mut full_names = BTreeSet::new();
    let mut partial_names = BTreeSet::new();
    let mut catalog_only_names = BTreeSet::new();
    let mut full_printings = 0;
    let mut partial_printings = 0;
    let mut catalog_only_printings = 0;

    for card in rav_main_set_catalog() {
        match card.semantic_status {
            CardSemanticStatus::ExecutableCompatibilitySlice { definition_id }
                if full_ids.contains(definition_id) =>
            {
                full_names.insert(card.name);
                full_printings += 1;
            }
            CardSemanticStatus::ExecutableCompatibilitySlice { .. } => {
                partial_names.insert(card.name);
                partial_printings += 1;
            }
            CardSemanticStatus::CatalogOnly { .. } => {
                catalog_only_names.insert(card.name);
                catalog_only_printings += 1;
            }
        }
    }

    assert_eq!(full_names.len(), 291);
    assert_eq!(full_printings, 306);
    assert_eq!(partial_names.len(), 0);
    assert_eq!(partial_printings, 0);
    assert_eq!(catalog_only_names.len(), 0);
    assert_eq!(catalog_only_printings, 0);
    assert_eq!(
        full_names.len() + partial_names.len() + catalog_only_names.len(),
        291
    );
    assert_eq!(
        full_printings + partial_printings + catalog_only_printings,
        306
    );
}

#[test]
fn every_positive_manifest_definition_declares_full_fidelity() {
    let definitions = cardbench_magic_rav::card_definitions();
    let manifest = RAV_FULL_FIDELITY_DEFINITION_IDS
        .into_iter()
        .collect::<BTreeSet<_>>();
    assert_eq!(
        definitions.len(),
        manifest.len(),
        "definition/manifest count drift"
    );

    let mut missing_markers = Vec::new();
    let mut stale_labels = Vec::new();
    for id in manifest {
        let definition = definitions
            .iter()
            .find(|definition| definition.id == id)
            .unwrap_or_else(|| panic!("positive manifest names missing definition {id}"));
        if definition.supported_rules.first().copied() != Some("full-rules-fidelity") {
            missing_markers.push(id);
        }
        if definition
            .supported_rules
            .iter()
            .any(|rule| rule.contains("compatibility") || rule.contains("unsupported"))
        {
            stale_labels.push(id);
        }
    }
    assert!(
        missing_markers.is_empty(),
        "missing fidelity markers: {missing_markers:?}"
    );
    assert!(
        stale_labels.is_empty(),
        "stale compatibility labels: {stale_labels:?}"
    );
}

#[test]
fn every_nonvanilla_positive_definition_has_registered_behavior() {
    let definitions = card_definitions();
    let registry_debug = [
        format!("{:?}", rav_mana_ability_bindings()),
        format!("{:?}", rav_mana_ability_cost_bindings()),
        format!("{:?}", rav_land_entry_bindings()),
        format!("{:?}", rav_activated_ability_bindings()),
        format!("{:?}", rav_attachment_bindings()),
        format!("{:?}", rav_entry_copy_bindings()),
        format!("{:?}", rav_entry_coin_flip_bindings()),
        format!("{:?}", rav_legendary_permanent_bindings()),
        format!("{:?}", rav_static_continuous_effect_bindings()),
        format!("{:?}", rav_static_library_top_reveal_bindings()),
        format!("{:?}", rav_static_attack_restriction_bindings()),
        format!("{:?}", rav_static_creature_spell_cost_modifier_bindings()),
        format!("{:?}", rav_static_entry_restriction_bindings()),
        format!("{:?}", rav_triggered_ability_bindings()),
        format!("{:?}", rav_attachment_triggered_ability_bindings()),
        format!("{:?}", rav_basic_land_type_bindings()),
        format!("{:?}", rav_additional_spell_cost_bindings()),
        format!("{:?}", rav_cost_reduction_bindings()),
        format!("{:?}", rav_activated_ability_cost_modifier_bindings()),
        format!("{:?}", rav_generalized_activated_ability_cost_bindings()),
        format!("{:?}", rav_replacement_effect_bindings()),
        format!("{:?}", rav_damage_replacement_effect_bindings()),
    ]
    .join("\n");
    let mut unregistered = Vec::new();
    for definition in definitions {
        if definition.is_basic_land || !RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&definition.id) {
            continue;
        }
        let has_declared_behavior = !definition.keywords.is_empty()
            || !definition.effects.is_empty()
            || registry_debug.contains(definition.id);
        let is_verified_vanilla = matches!(definition.id, "RAV-WATCHWOLF" | "RAV-GLASS-GOLEM");
        if !has_declared_behavior && !is_verified_vanilla {
            unregistered.push(definition.id);
        }
    }
    assert!(
        unregistered.is_empty(),
        "positive definitions have no keyword, effect, or binding: {unregistered:?}"
    );
}
