//! Red regression: a virtual copied public graveyard-land return can complete
//! its policy decision without a fabricated physical source zone move.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, CastRequest, DecisionSelection, Effect, Game, GameEvent, ManaCost,
    ObjectId, PlayerId, Target, Zone,
};

const RETURN_LANDS: &str = "TST-VIRTUAL-COPY-GRAVEYARD-LANDS";
const COPY: &str = "TST-VIRTUAL-COPY-GRAVEYARD-LANDS-COPY";
const LAND: &str = "TST-VIRTUAL-COPY-GRAVEYARD-LANDS-LAND";

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
        supported_rules: &["virtual-spell-copy-graveyard-land-return-red"],
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
fn virtual_copy_can_complete_public_graveyard_land_return() {
    let caster = PlayerId(0);
    let copy_controller = PlayerId(1);
    let mut game = Game::new(
        [
            definition(
                RETURN_LANDS,
                BTreeSet::from([CardType::Instant]),
                false,
                vec![Effect::ReturnUpToThreeControllerGraveyardLandCardsToHand],
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
        ],
        2,
    )
    .expect("fixture initializes");
    let return_lands = game
        .add_card(caster, RETURN_LANDS, Zone::Hand)
        .expect("return spell enters hand");
    let copy = game
        .add_card(copy_controller, COPY, Zone::Hand)
        .expect("copy spell enters hand");
    let land = game
        .add_card(copy_controller, LAND, Zone::Graveyard)
        .expect("copy controller has public land candidate");
    game.begin_game().expect("game begins");

    game.cast_spell(caster, request(return_lands, vec![]))
        .expect("return spell casts");
    game.pass_priority(caster)
        .expect("caster passes to copy controller");
    game.cast_spell(
        copy_controller,
        request(copy, vec![Target::Spell(return_lands)]),
    )
    .expect("copy spell casts");
    resolve_top(&mut game).expect("copy instruction creates virtual return spell");
    resolve_top(&mut game).expect("virtual return opens public decision");

    let decision = game
        .view_for_player(copy_controller)
        .expect("copy controller has view")
        .pending_decision
        .expect("virtual return opens public land decision");
    let virtual_copy = game
        .event_log
        .iter()
        .find_map(|event| match event {
            GameEvent::SpellCopied { copy, original, .. } if *original == return_lands => {
                Some(*copy)
            }
            _ => None,
        })
        .expect("copy receipt identifies virtual stack spell");
    let result = game.submit_decision(
        copy_controller,
        decision.id,
        DecisionSelection::Objects(vec![land]),
    );
    eprintln!(
        "virtual-copy graveyard-land-return red trace: result={result:?}; events={:?}",
        game.canonical_event_log()
    );
    assert!(
        result.is_ok(),
        "the completed virtual land return must not attempt a physical spell-zone move"
    );
    assert_eq!(game.zone_of(land), Some(Zone::Hand));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::SpellCopyResolved { copy, original } if *copy == virtual_copy && *original == return_lands
    )));
    game.validate_invariants()
        .expect("virtual graveyard-land terminal lifecycle remains auditable");
}
