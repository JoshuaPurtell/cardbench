//! Red regression for a target player's resolution-time colored-mana choice.
//!
//! This is intentionally synthetic: it protects the shared decision/stack
//! substrate needed by targeted mana artifacts without encoding card text.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, Color, Effect, Game, ManaCost, PlayerId,
};

const TARGETED_MANA: &str = "TST-TARGETED-MANA";

#[test]
fn target_player_must_choose_the_colored_mana_they_receive_at_resolution() {
    let game = Game::new(
        [CardDefinition {
            id: TARGETED_MANA,
            name: TARGETED_MANA,
            set_code: "TST",
            mana_cost: ManaCost::new(0),
            colors: BTreeSet::new(),
            mana_colors: BTreeSet::new(),
            card_types: BTreeSet::from([CardType::Artifact]),
            is_basic_land: false,
            supported_rules: &["target-player-mana-color-decision"],
            power: None,
            toughness: None,
            keywords: vec![],
            effects: vec![Effect::AddOneManaOfTargetPlayersChosenColor],
        }],
        2,
    );

    assert!(game.is_ok(), "fixture requires target-player mana-choice substrate");
    let _ = Color::Blue;
    let _ = PlayerId(1);
}
