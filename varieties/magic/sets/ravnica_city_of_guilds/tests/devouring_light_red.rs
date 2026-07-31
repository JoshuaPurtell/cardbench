//! Red contract for Devouring Light's attacking/blocking target and exile.

use cardbench_magic_engine::{Effect, TargetRequirement};
use cardbench_magic_rav::card_definitions;

#[test]
fn devouring_light_executable_contract_exists() {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.name == "Devouring Light")
        .expect("Devouring Light must have an executable definition");
    assert!(
        definition
            .keywords
            .iter()
            .any(|keyword| matches!(keyword, cardbench_magic_engine::Keyword::Convoke))
    );
    assert_eq!(definition.effects, vec![Effect::ExileTargetPermanent]);
    assert_eq!(
        definition.effects[0].target_requirement(),
        Some(TargetRequirement::AttackingOrBlockingCreature)
    );
}
