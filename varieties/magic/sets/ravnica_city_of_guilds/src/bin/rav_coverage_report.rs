use std::collections::BTreeSet;

use cardbench_magic_rav::{
    CardSemanticStatus, RAV_FULL_FIDELITY_DEFINITION_IDS, rav_main_set_catalog,
};

fn main() {
    let full_ids = RAV_FULL_FIDELITY_DEFINITION_IDS
        .into_iter()
        .collect::<BTreeSet<_>>();
    let catalog = rav_main_set_catalog();
    let mut full_names = BTreeSet::new();
    let mut partial_names = BTreeSet::new();
    let mut catalog_only_names = BTreeSet::new();
    let mut full_printings = 0_usize;
    let mut partial_printings = 0_usize;
    let mut catalog_only_printings = 0_usize;

    for card in catalog {
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

    let total_printings = full_printings + partial_printings + catalog_only_printings;
    let total_names = full_names.len() + partial_names.len() + catalog_only_names.len();
    println!("schema_version=cardbench.magic.rav-coverage.v1");
    println!("printings.total={total_printings}");
    println!("printings.full={full_printings}");
    println!("printings.partial={partial_printings}");
    println!("printings.catalog_only={catalog_only_printings}");
    println!("unique_names.total={total_names}");
    println!("unique_names.full={}", full_names.len());
    println!("unique_names.partial={}", partial_names.len());
    println!("unique_names.catalog_only={}", catalog_only_names.len());
    println!(
        "unique_names.percentages=full:{},partial:{},catalog_only:{}",
        percentage(full_names.len(), total_names),
        percentage(partial_names.len(), total_names),
        percentage(catalog_only_names.len(), total_names),
    );
    println!(
        "printings.percentages=full:{},partial:{},catalog_only:{}",
        percentage(full_printings, total_printings),
        percentage(partial_printings, total_printings),
        percentage(catalog_only_printings, total_printings),
    );
}

fn percentage(part: usize, total: usize) -> String {
    let tenths = (part * 1_000 + total / 2) / total;
    format!("{}.{:01}", tenths / 10, tenths % 10)
}
