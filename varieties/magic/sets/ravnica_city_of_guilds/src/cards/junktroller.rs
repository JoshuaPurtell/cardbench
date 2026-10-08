//! Candidate-owned printed RAV Junktroller definition and graveyard-recycling ability.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    ActivatedAbility, ActivatedAbilityBinding, CardDefinition, CardType, Effect, Keyword, ManaCost,
};

#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-JUNKTROLLER",
        name: "Junktroller",
        set_code: "RAV",
        mana_cost: ManaCost::new(4),
        colors: BTreeSet::new(),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Artifact, CardType::Creature]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "colorless-artifact-creature-casting",
            "base-characteristics",
            "defender",
            "printed-tap-graveyard-card-to-owner-library-bottom",
        ],
        power: Some(0),
        toughness: Some(6),
        keywords: vec![Keyword::Defender],
        effects: vec![],
    }
}

#[must_use]
pub(crate) fn activated_ability() -> ActivatedAbilityBinding {
    ActivatedAbilityBinding {
        card_definition: "RAV-JUNKTROLLER",
        ability: ActivatedAbility {
            id: "tap-target-graveyard-card-to-owners-library-bottom",
            mana_cost: ManaCost::new(0),
            tap_cost: true,
            sorcery_speed: false,
            additional_tap_creatures: 0,
            sacrifice_source: false,
            sacrifice_creatures: 0,
            sacrifice_lands: 0,
            discard_cards: 0,
            targets: vec![cardbench_magic_engine::TargetRequirement::GraveyardCard],
            effects: vec![Effect::PutTargetGraveyardCardOnOwnersLibraryBottom],
        },
    }
}
