//! Red regression: a virtual copied token spell can finish an affected-player
//! quantity-replacement decision without a fabricated source-zone move.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, CastRequest, DecisionKind, DecisionSelection, Effect, Game,
    GameEvent, ManaCost, ObjectId, PlayerId, ReplacementChoice, ReplacementEffect,
    ReplacementEffectBinding, Target, TokenSpec, Zone,
};

const DOUBLER: &str = "TST-VIRTUAL-COPY-QUANTITY-DOUBLER";
const TRIPLER: &str = "TST-VIRTUAL-COPY-QUANTITY-TRIPLER";
const TOKEN: &str = "TST-VIRTUAL-COPY-QUANTITY-TOKEN";
const COPY: &str = "TST-VIRTUAL-COPY-QUANTITY-COPY";

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
        supported_rules: &["virtual-spell-copy-quantity-replacement-terminal-red"],
        power: None,
        toughness: None,
        keywords: vec![],
        effects,
    }
}

fn request(card: ObjectId, targets: Vec<Target>) -> CastRequest {
    CastRequest {
        card,
        targets,
        convoke: vec![],
        payment_mana_abilities: vec![],
    }
}

fn resolve_top(game: &mut Game) -> Result<(), cardbench_magic_engine::RulesError> {
    let first = game.priority;
    game.pass_priority(first)?;
    let second = game.priority;
    game.pass_priority(second)
}

#[test]
fn virtual_copy_can_complete_quantity_replacement_decision() {
    let caster = PlayerId(0);
    let copy_controller = PlayerId(1);
    let mut game = Game::new(
        [
            definition(DOUBLER, CardType::Enchantment, vec![]),
            definition(TRIPLER, CardType::Enchantment, vec![]),
            definition(
                TOKEN,
                CardType::Instant,
                vec![Effect::CreateToken {
                    token: TokenSpec::saproling(),
                    count: 1,
                }],
            ),
            definition(
                COPY,
                CardType::Instant,
                vec![Effect::CopyTargetInstantOrSorcerySpell {
                    may_choose_new_targets: false,
                }],
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
    .expect("replacement bindings register before the game");
    game.put_on_battlefield(copy_controller, DOUBLER)
        .expect("doubler enters before game");
    game.put_on_battlefield(copy_controller, TRIPLER)
        .expect("tripler enters before game");
    let token = game
        .add_card(caster, TOKEN, Zone::Hand)
        .expect("token spell enters hand");
    let copy = game
        .add_card(copy_controller, COPY, Zone::Hand)
        .expect("copy spell enters hand");
    game.begin_game().expect("game begins");

    game.cast_spell(caster, request(token, vec![]))
        .expect("token spell casts");
    game.pass_priority(caster)
        .expect("caster passes to copy controller");
    game.cast_spell(copy_controller, request(copy, vec![Target::Spell(token)]))
        .expect("copy spell casts");
    resolve_top(&mut game).expect("copy instruction creates virtual token spell");
    resolve_top(&mut game).expect("virtual token spell opens replacement decision");

    let virtual_copy = game
        .event_log
        .iter()
        .find_map(|event| match event {
            GameEvent::SpellCopied { copy, original, .. } if *original == token => Some(*copy),
            _ => None,
        })
        .expect("copy receipt identifies virtual token spell");
    let decision = game
        .view_for_player(copy_controller)
        .expect("copy controller view")
        .pending_decision
        .expect("concurrent replacements open an affected-player decision");
    assert_eq!(decision.kind, DecisionKind::Replacement);
    let tripler = decision
        .replacement_candidates
        .iter()
        .copied()
        .find(|choice| matches!(
            choice,
            ReplacementChoice::Quantity {
                effect: ReplacementEffect::MultiplyTokenCreation { multiplier: 3 },
                ..
            }
        ))
        .expect("tripler is a legal replacement choice");
    let result = game.submit_decision(
        copy_controller,
        decision.id,
        DecisionSelection::Replacements(vec![tripler]),
    );
    eprintln!(
        "virtual-copy quantity-replacement red trace: result={result:?}; events={:?}",
        game.canonical_event_log()
    );
    assert!(
        result.is_ok(),
        "a virtual token spell must not attempt a physical terminal-zone move after replacement"
    );
    assert_eq!(
        game.player(copy_controller)
            .expect("copy controller exists")
            .battlefield
            .len(),
        8,
        "one token becomes six after selected tripler then forced doubler"
    );
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::SpellCopyResolved { copy, original } if *copy == virtual_copy && *original == token
    )));
    game.validate_invariants()
        .expect("virtual quantity-replacement terminal lifecycle remains auditable");
}
