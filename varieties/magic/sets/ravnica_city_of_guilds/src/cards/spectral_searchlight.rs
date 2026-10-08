//! CardBench-authored RAV Spectral Searchlight identity and activation.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    ActivatedAbility, CardDefinition, CardType, Effect, ManaCost, TargetRequirement,
};

/// Printed characteristics for the colorless mana-producing artifact.
#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-SPECTRAL-SEARCHLIGHT",
        name: "Spectral Searchlight",
        set_code: "RAV",
        mana_cost: ManaCost::new(3),
        colors: BTreeSet::new(),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Artifact]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "colorless-artifact-casting",
            "tap-target-player-chooses-one-color-mana",
            "target-player-chooses-one-color-mana",
        ],
        power: None,
        toughness: None,
        keywords: vec![],
        effects: vec![],
    }
}

/// Taps to target a player; that player chooses one colored mana to add.
#[must_use]
pub(crate) fn activated_ability() -> ActivatedAbility {
    ActivatedAbility {
        id: "tap-target-player-chosen-color-mana",
        mana_cost: ManaCost::new(0),
        tap_cost: true,
        sorcery_speed: false,
        additional_tap_creatures: 0,
        sacrifice_source: false,
        sacrifice_creatures: 0,
        sacrifice_lands: 0,
        discard_cards: 0,
        targets: vec![TargetRequirement::Player],
        effects: vec![Effect::AddOneManaOfTargetPlayersChosenColor],
    }
}
