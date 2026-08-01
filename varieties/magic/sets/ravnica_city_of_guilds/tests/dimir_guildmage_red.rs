use cardbench_magic_engine::{CardType, Color, HybridManaSymbol, ManaCost};
use cardbench_magic_rav::{card_definitions, rav_activated_ability_bindings};

#[test]
fn dimir_guildmage_requires_its_hybrid_body_and_targeted_activated_pair() {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-DIMIR-GUILDMAGE")
        .expect("Dimir Guildmage definition exists");
    assert_eq!(
        definition.mana_cost,
        ManaCost::with_hybrid(
            0,
            [],
            [
                HybridManaSymbol {
                    first: Color::Blue,
                    second: Color::Black,
                },
                HybridManaSymbol {
                    first: Color::Blue,
                    second: Color::Black,
                },
            ],
        )
    );
    assert_eq!(definition.card_types, [CardType::Creature].into());
    assert_eq!((definition.power, definition.toughness), (Some(2), Some(2)));
    assert!(
        rav_activated_ability_bindings().iter().any(|binding| {
            binding.card_definition == definition.id && binding.ability.id == "target-player-draw"
        }),
        "sorcery-speed target draw must be bound"
    );
    assert!(
        rav_activated_ability_bindings().iter().any(|binding| {
            binding.card_definition == definition.id
                && binding.ability.id == "target-player-discard"
        }),
        "targeted discard must be bound"
    );
}
