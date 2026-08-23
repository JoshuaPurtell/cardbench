//! Red regression: a virtual spell copy can complete the target player's
//! private conditional-discard decision without a fabricated zone move.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, CastRequest, DecisionSelection, Effect, Game, GameEvent, ManaCost,
    ObjectId, PlayerId, Target, Zone,
};

const DRAW_DISCARD: &str = "TST-VIRTUAL-COPY-DRAW-DISCARD";
const COPY: &str = "TST-VIRTUAL-COPY-DRAW-DISCARD-COPY";
const LAND: &str = "TST-VIRTUAL-COPY-DRAW-DISCARD-LAND";
const FILLER: &str = "TST-VIRTUAL-COPY-DRAW-DISCARD-FILLER";

fn definition(
    id: &'static str,
    types: BTreeSet<CardType>,
    is_basic_land: bool,
    effects: Vec<Effect>,
) -> CardDefinition {
    CardDefinition {
        id,
        name: id,
        set_code: "TST",
        mana_cost: ManaCost::new(0),
        colors: BTreeSet::new(),
        mana_colors: BTreeSet::new(),
        card_types: types,
        is_basic_land,
        supported_rules: &["virtual-spell-copy-conditional-discard-red"],
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
fn virtual_copy_can_complete_conditional_private_discard() {
    let caster = PlayerId(0);
    let copy_controller = PlayerId(1);
    let mut game = Game::new(
        [
            definition(
                DRAW_DISCARD,
                BTreeSet::from([CardType::Instant]),
                false,
                vec![Effect::DrawTargetPlayerThenConditionalPrivateDiscard],
            ),
            definition(
                COPY,
                BTreeSet::from([CardType::Instant]),
                false,
                vec![Effect::CopyTargetInstantOrSorcerySpell {
                    may_choose_new_targets: false,
                }],
            ),
            definition(LAND, BTreeSet::from([CardType::Land]), true, vec![]),
            definition(FILLER, BTreeSet::from([CardType::Artifact]), false, vec![]),
        ],
        2,
    )
    .expect("fixture initializes");
    let spell = game
        .add_card(caster, DRAW_DISCARD, Zone::Hand)
        .expect("draw-discard spell enters hand");
    let copy = game
        .add_card(copy_controller, COPY, Zone::Hand)
        .expect("copy spell enters hand");
    let discard_land = game
        .add_card(copy_controller, LAND, Zone::Hand)
        .expect("target has a legal one-land discard");
    for _ in 0..3 {
        game.add_card(copy_controller, FILLER, Zone::Library)
            .expect("target has three cards to draw");
    }
    game.begin_game().expect("game begins");

    game.cast_spell(
        caster,
        request(spell, vec![Target::Player(copy_controller)]),
    )
    .expect("draw-discard spell casts");
    game.pass_priority(caster)
        .expect("caster passes to copy controller");
    game.cast_spell(copy_controller, request(copy, vec![Target::Spell(spell)]))
        .expect("copy spell casts");
    resolve_top(&mut game).expect("copy instruction creates virtual spell");
    resolve_top(&mut game).expect("virtual spell draws and opens private discard choice");

    let decision = game
        .view_for_player(copy_controller)
        .expect("target player has view")
        .pending_decision
        .expect("copied spell opens target private discard decision");
    let virtual_copy = game
        .event_log
        .iter()
        .find_map(|event| match event {
            GameEvent::SpellCopied { copy, original, .. } if *original == spell => Some(*copy),
            _ => None,
        })
        .expect("copy receipt identifies virtual spell");
    let result = game.submit_decision(
        copy_controller,
        decision.id,
        DecisionSelection::Objects(vec![discard_land]),
    );
    eprintln!(
        "virtual-copy conditional-discard red trace: result={result:?}; events={:?}",
        game.canonical_event_log()
    );
    assert!(
        result.is_ok(),
        "the completed virtual conditional discard must not attempt a physical spell-zone move"
    );
    assert_eq!(game.zone_of(discard_land), Some(Zone::Graveyard));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::SpellCopyResolved { copy, original } if *copy == virtual_copy && *original == spell
    )));
    game.validate_invariants()
        .expect("virtual conditional-discard terminal lifecycle remains auditable");
}
