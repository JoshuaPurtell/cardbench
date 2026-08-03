//! Red regression: modal stack targets must be audited against the selected mode.
//!
//! A modal spell's immutable stack effects are the selected branch, not the
//! card's unmaterialized `ChooseOneOf` wrapper. The invariant audit must
//! therefore reject a target whose enum shape no longer matches the selected
//! branch, even when its cardinality remains unchanged.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, CastRequest, Effect, Game, ManaCost, PlayerId, Target,
    TargetRequirement, Zone,
};

const CREATURE: &str = "TST-MODAL-TARGET-SHAPE-CREATURE";
const MODAL: &str = "TST-MODAL-TARGET-SHAPE-SPELL";

fn definition(
    id: &'static str,
    card_type: CardType,
    effects: Vec<Effect>,
    power: Option<i16>,
    toughness: Option<i16>,
) -> CardDefinition {
    CardDefinition {
        id,
        name: id,
        set_code: "TST",
        mana_cost: ManaCost::new(0),
        colors: BTreeSet::new(),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([card_type]),
        is_basic_land: false,
        supported_rules: &["modal-stack-target-shape-invariant-red"],
        power,
        toughness,
        keywords: vec![],
        effects,
    }
}

#[test]
fn invariant_rejects_a_modal_player_target_rewritten_as_a_permanent() {
    let mut game = Game::new(
        [
            definition(CREATURE, CardType::Creature, vec![], Some(1), Some(1)),
            definition(
                MODAL,
                CardType::Instant,
                vec![Effect::ChooseOneOf(vec![
                    vec![Effect::DealDamage {
                        amount: 1,
                        target: TargetRequirement::Player,
                    }],
                    vec![Effect::GainLifeController { amount: 1 }],
                ])],
                None,
                None,
            ),
        ],
        2,
    )
    .expect("fixture initializes");
    let permanent = game
        .put_on_battlefield(PlayerId(1), CREATURE)
        .expect("permanent enters battlefield");
    let spell = game
        .add_card(PlayerId(0), MODAL, Zone::Hand)
        .expect("modal spell enters hand");
    game.begin_game().expect("game begins");
    game.cast_spell_with_mode(
        PlayerId(0),
        CastRequest {
            card: spell,
            targets: vec![Target::Player(PlayerId(1))],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
        0,
    )
    .expect("player-targeting modal branch casts");

    // This simulates a malformed external state. Its target count is still
    // one, but its enum shape violates the materialized player-target effect.
    game.stack[0].targets = vec![Target::Permanent(permanent)];
    let audit = game.validate_invariants();
    eprintln!(
        "modal target-shape audit: {audit:?}; stack={:?}; events={:?}",
        game.stack,
        game.canonical_event_log()
    );
    assert!(
        audit.is_err(),
        "a selected modal player target cannot be rewritten as a permanent"
    );
}
