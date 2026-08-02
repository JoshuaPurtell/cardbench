//! Red regression: losing for an empty-library draw is an SBA after the whole
//! spell, not an immediate interruption of its remaining instructions.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, CastRequest, Effect, Game, GameEvent, ManaCost, PlayerId, Zone,
};

const DRAW_THEN_GAIN: &str = "STACK-DRAW-LOSS-SBA-TIMING";

fn draw_then_gain() -> CardDefinition {
    CardDefinition {
        id: DRAW_THEN_GAIN,
        name: DRAW_THEN_GAIN,
        set_code: "TST",
        mana_cost: ManaCost::new(0),
        colors: BTreeSet::new(),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Instant]),
        is_basic_land: false,
        supported_rules: &["empty-library-draw-sba-timing-probe"],
        power: None,
        toughness: None,
        keywords: vec![],
        effects: vec![
            Effect::DrawController,
            Effect::GainLifeController { amount: 1 },
        ],
    }
}

#[test]
fn empty_library_draw_waits_for_the_full_spell_before_player_loss() {
    let caster = PlayerId(0);
    let opponent = PlayerId(1);
    let mut game = Game::new([draw_then_gain()], 2).expect("fixture initializes");
    let spell = game
        .add_card(caster, DRAW_THEN_GAIN, Zone::Hand)
        .expect("spell begins in the empty-library caster's hand");
    game.cast_spell(
        caster,
        CastRequest {
            card: spell,
            targets: vec![],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("spell casts");
    game.pass_priority(caster).expect("caster passes");
    game.pass_priority(opponent)
        .expect("spell resolves through both instructions");

    let resolved = game
        .event_log
        .iter()
        .position(|event| matches!(event, GameEvent::SpellResolved { card } if *card == spell))
        .expect("the spell resolves before the draw-loss SBA removes its controller");
    let life_gain = game
        .event_log
        .iter()
        .position(|event| matches!(event, GameEvent::LifeGained { player, amount: 1 } if *player == caster))
        .expect("the later gain-life instruction resolves after the failed draw");
    let lost = game
        .event_log
        .iter()
        .position(|event| matches!(event, GameEvent::PlayerLost { player, reason } if *player == caster && *reason == "attempted to draw from an empty library"))
        .expect("the failed draw eventually produces its SBA loss");
    assert!(
        life_gain < resolved && resolved < lost,
        "the complete spell must finish before the draw-loss SBA; events={:?}",
        game.canonical_event_log(),
    );
    assert!(game.players[caster.0].lost);
    assert!(game.is_game_over());
    game.validate_invariants()
        .expect("deferred draw-loss provenance is auditable after resolution");
}
