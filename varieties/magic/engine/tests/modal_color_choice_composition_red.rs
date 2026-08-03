//! Red regression for one cast action that needs both a mode and a color.
//!
//! `Game::cast_spell_impl` already carries independent `chosen_modal_mode` and
//! `chosen_color` slots.  The public action surface must expose the same
//! atomic choice boundary: Magic does not permit casting a modal spell, then
//! later supplying a color after the card has left the hand.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, CastRequest, Color, Effect, Game, ManaCost, PlayerId, Target,
    Zone,
};

const CREATURE: &str = "TST-MODAL-COLOR-CREATURE";
const SPELL: &str = "TST-MODAL-COLOR-SPELL";

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
        colors: BTreeSet::from([Color::Blue]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([card_type]),
        is_basic_land: false,
        supported_rules: &["modal-color-choice-composition-red"],
        power,
        toughness,
        keywords: vec![],
        effects,
    }
}

#[test]
fn selected_modal_branch_can_submit_its_required_color_in_the_same_cast() {
    let mut game = Game::new(
        [
            definition(CREATURE, CardType::Creature, vec![], Some(2), Some(2)),
            definition(
                SPELL,
                CardType::Instant,
                vec![Effect::ChooseOneOf(vec![
                    vec![Effect::ReplaceTargetCreatureColorsWithChosenColorUntilEndOfTurn],
                    vec![Effect::GainLifeController { amount: 1 }],
                ])],
                None,
                None,
            ),
        ],
        2,
    )
    .expect("fixture initializes");
    let creature = game
        .put_on_battlefield(PlayerId(1), CREATURE)
        .expect("creature setup");
    let spell = game
        .add_card(PlayerId(0), SPELL, Zone::Hand)
        .expect("spell setup");
    game.begin_game().expect("game begins");

    // Before the green repair this is the closest public action, but it has
    // no way to provide the required color.  It rejects with the color-boundary
    // error and leaves the spell in hand, proving the policy surface cannot
    // represent this one legal cast action.
    let cast = game.cast_spell_with_mode(
        PlayerId(0),
        CastRequest {
            card: spell,
            targets: vec![Target::Permanent(creature)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
        0,
    );
    eprintln!(
        "modal color choice cast={cast:?}; zone={:?}; stack={:?}; events={:?}",
        game.zone_of(spell),
        game.stack,
        game.canonical_event_log()
    );
    assert!(
        cast.is_ok(),
        "one cast action must accept both its selected mode and required color"
    );
}
