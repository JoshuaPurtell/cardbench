//! Red regression: a virtual copied public graveyard-creature return can
//! complete every affected player's decision without a fabricated source move.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, CastRequest, DecisionSelection, Effect, Game, GameEvent, ManaCost,
    ObjectId, PlayerId, Target, Zone,
};

const RETURN_CREATURES: &str = "TST-VIRTUAL-COPY-GRAVEYARD-CREATURES";
const COPY: &str = "TST-VIRTUAL-COPY-GRAVEYARD-CREATURES-COPY";
const CREATURE: &str = "TST-VIRTUAL-COPY-GRAVEYARD-CREATURE";

fn definition(id: &'static str, types: BTreeSet<CardType>, effects: Vec<Effect>) -> CardDefinition {
    let is_creature = types.contains(&CardType::Creature);
    CardDefinition {
        id,
        name: id,
        set_code: "TST",
        mana_cost: ManaCost::new(0),
        colors: BTreeSet::new(),
        mana_colors: BTreeSet::new(),
        card_types: types,
        is_basic_land: false,
        supported_rules: &["virtual-spell-copy-graveyard-creature-return-red"],
        power: is_creature.then_some(1),
        toughness: is_creature.then_some(1),
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
fn virtual_copy_can_complete_every_public_graveyard_creature_return_choice() {
    let caster = PlayerId(0);
    let copy_controller = PlayerId(1);
    let mut game = Game::new(
        [
            definition(
                RETURN_CREATURES,
                BTreeSet::from([CardType::Instant]),
                vec![Effect::ReturnOneCreatureCardFromEachGraveyardToHand],
            ),
            definition(
                COPY,
                BTreeSet::from([CardType::Instant]),
                vec![Effect::CopyTargetInstantOrSorcerySpell {
                    may_choose_new_targets: false,
                }],
            ),
            definition(CREATURE, BTreeSet::from([CardType::Creature]), vec![]),
        ],
        2,
    )
    .expect("fixture initializes");
    let return_creatures = game
        .add_card(caster, RETURN_CREATURES, Zone::Hand)
        .expect("return spell enters hand");
    let copy = game
        .add_card(copy_controller, COPY, Zone::Hand)
        .expect("copy spell enters hand");
    let caster_creature = game
        .add_card(caster, CREATURE, Zone::Graveyard)
        .expect("caster has public creature candidate");
    let copy_controller_creature = game
        .add_card(copy_controller, CREATURE, Zone::Graveyard)
        .expect("copy controller has public creature candidate");
    game.begin_game().expect("game begins");

    game.cast_spell(caster, request(return_creatures, vec![]))
        .expect("return spell casts");
    game.pass_priority(caster)
        .expect("caster passes to copy controller");
    game.cast_spell(
        copy_controller,
        request(copy, vec![Target::Spell(return_creatures)]),
    )
    .expect("copy spell casts");
    resolve_top(&mut game).expect("copy instruction creates virtual return spell");
    resolve_top(&mut game).expect("virtual return opens first public decision");

    let virtual_copy = game
        .event_log
        .iter()
        .find_map(|event| match event {
            GameEvent::SpellCopied { copy, original, .. } if *original == return_creatures => {
                Some(*copy)
            }
            _ => None,
        })
        .expect("copy receipt identifies virtual stack spell");
    let caster_decision = game
        .view_for_player(caster)
        .expect("caster has view")
        .pending_decision
        .expect("first public creature choice opens");
    game.submit_decision(
        caster,
        caster_decision.id,
        DecisionSelection::Objects(vec![caster_creature]),
    )
    .expect("first public creature choice advances to next player");
    let copy_controller_decision = game
        .view_for_player(copy_controller)
        .expect("copy controller has view")
        .pending_decision
        .expect("second public creature choice opens");
    let result = game.submit_decision(
        copy_controller,
        copy_controller_decision.id,
        DecisionSelection::Objects(vec![copy_controller_creature]),
    );
    eprintln!(
        "virtual-copy graveyard-creature-return red trace: result={result:?}; events={:?}",
        game.canonical_event_log()
    );
    assert!(
        result.is_ok(),
        "the completed virtual creature return must not attempt a physical spell-zone move"
    );
    assert_eq!(game.zone_of(caster_creature), Some(Zone::Hand));
    assert_eq!(game.zone_of(copy_controller_creature), Some(Zone::Hand));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::SpellCopyResolved { copy, original } if *copy == virtual_copy && *original == return_creatures
    )));
    game.validate_invariants()
        .expect("virtual graveyard-creature terminal lifecycle remains auditable");
}
