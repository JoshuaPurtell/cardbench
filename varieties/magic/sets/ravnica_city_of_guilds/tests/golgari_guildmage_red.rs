//! Red discovery probe for the source-only Golgari Guildmage lane.

use cardbench_magic_engine::{Color, Effect, HybridManaSymbol, ManaCost, TargetRequirement};
use cardbench_magic_rav::{
    RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, rav_activated_ability_bindings,
};

#[test]
fn golgari_guildmage_requires_its_full_two_ability_slice() {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-GOLGARI-GUILDMAGE")
        .expect("Golgari Guildmage definition exists");
    assert!(
        RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&definition.id),
        "both printed activated abilities must be bound before the card is full"
    );
    assert_eq!(
        definition.mana_cost,
        ManaCost::with_hybrid(
            0,
            [],
            [
                HybridManaSymbol {
                    first: Color::Black,
                    second: Color::Green,
                },
                HybridManaSymbol {
                    first: Color::Black,
                    second: Color::Green,
                },
            ],
        )
    );
    let bindings = rav_activated_ability_bindings();
    let trample = bindings
        .iter()
        .find(|binding| {
            binding.card_definition == definition.id
                && binding.ability.id == "target-pump-and-trample"
        })
        .expect("Golgari Guildmage pump-and-trample binding exists");
    assert_eq!(
        trample.ability.mana_cost,
        ManaCost::with_colors(0, [Color::Black, Color::Green])
    );
    assert_eq!(trample.ability.targets, [TargetRequirement::Creature]);
    let regenerate = bindings
        .iter()
        .find(|binding| {
            binding.card_definition == definition.id
                && binding.ability.id == "regenerate-target-creature"
        })
        .expect("Golgari Guildmage regeneration binding exists");
    assert_eq!(
        regenerate.ability.mana_cost,
        ManaCost::with_colors(2, [Color::Black, Color::Green])
    );
    assert_eq!(regenerate.ability.targets, [TargetRequirement::Creature]);
    assert_eq!(
        regenerate.ability.effects,
        [Effect::RegenerateTargetCreature]
    );
}
