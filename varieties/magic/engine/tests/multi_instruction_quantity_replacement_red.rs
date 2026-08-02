//! Red regression: every prospective quantity event needs affected-player
//! ordering, even when another instruction remains in the same stack item.
//!
//! A replacement effect is applied immediately before the event it replaces.
//! The stack item must therefore remain live while its affected player chooses
//! among concurrent token multipliers; it cannot silently resolve the whole
//! multi-instruction spell in stable object-id order.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, CastRequest, DecisionKind, Effect, Game, ManaCost, PlayerId,
    ReplacementEffect, ReplacementEffectBinding, TokenSpec, Zone,
};

const DOUBLER: &str = "TST-MULTI-INSTRUCTION-DOUBLER";
const TRIPLER: &str = "TST-MULTI-INSTRUCTION-TRIPLER";
const TOKEN_AND_LIFE: &str = "TST-MULTI-INSTRUCTION-TOKEN-AND-LIFE";

fn definition(id: &'static str, card_type: CardType, effects: Vec<Effect>) -> CardDefinition {
    CardDefinition {
        id,
        name: id,
        set_code: "TST",
        mana_cost: ManaCost::new(0),
        colors: BTreeSet::new(),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([card_type]),
        is_basic_land: false,
        supported_rules: &["multi-instruction-quantity-replacement-red"],
        power: None,
        toughness: None,
        keywords: vec![],
        effects,
    }
}

#[test]
fn concurrent_token_replacements_pause_a_multi_instruction_spell_for_the_affected_player() {
    let caster = PlayerId(0);
    let responder = PlayerId(1);
    let mut game = Game::new(
        [
            definition(DOUBLER, CardType::Enchantment, vec![]),
            definition(TRIPLER, CardType::Enchantment, vec![]),
            definition(
                TOKEN_AND_LIFE,
                CardType::Instant,
                vec![
                    Effect::CreateToken {
                        token: TokenSpec::saproling(),
                        count: 1,
                    },
                    Effect::GainLifeController { amount: 1 },
                ],
            ),
        ],
        2,
    )
    .expect("fixture initializes");
    game.register_replacement_effect_bindings([
        ReplacementEffectBinding {
            source_definition: DOUBLER,
            effect: ReplacementEffect::MultiplyTokenCreation { multiplier: 2 },
        },
        ReplacementEffectBinding {
            source_definition: TRIPLER,
            effect: ReplacementEffect::MultiplyTokenCreation { multiplier: 3 },
        },
    ])
    .expect("immutable multiplier bindings register before the game");
    game.put_on_battlefield(caster, DOUBLER)
        .expect("doubler enters before the game");
    game.put_on_battlefield(caster, TRIPLER)
        .expect("tripler enters before the game");
    let spell = game
        .add_card(caster, TOKEN_AND_LIFE, Zone::Hand)
        .expect("multi-instruction spell enters hand");
    game.begin_game().expect("game begins");

    game.cast_spell(
        caster,
        CastRequest {
            card: spell,
            targets: vec![],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("zero-cost instant casts");
    game.pass_priority(caster)
        .expect("caster passes to the responder");
    game.pass_priority(responder)
        .expect("responder reaches the first prospective event");

    let view = game
        .view_for_player(caster)
        .expect("affected player receives a view");
    eprintln!(
        "multi-instruction replacement trace: stack={:?}; life={}; battlefield={:?}; pending={:?}; events={:?}",
        game.stack,
        game.player(caster).expect("caster exists").life,
        game.player(caster).expect("caster exists").battlefield,
        view.pending_decision,
        game.canonical_event_log(),
    );
    let decision = view.pending_decision.expect(
        "concurrent token replacements must pause the live multi-instruction spell for an affected-player choice",
    );
    assert_eq!(decision.kind, DecisionKind::Replacement);
    assert_eq!(decision.replacement_candidates.len(), 2);
    assert_eq!(game.stack.len(), 1, "the spell remains on the stack");
}
