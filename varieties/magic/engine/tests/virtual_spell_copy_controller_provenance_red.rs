//! Red regression: copied damage uses the copy controller, not the physical
//! original's controller, for source-controller prevention.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, CastRequest, Effect, Game, Keyword, ManaCost, ObjectId, PlayerId,
    Target, TargetRequirement, Zone,
};

const BOLT: &str = "TST-VIRTUAL-COPY-CONTROLLER-BOLT";
const COPY: &str = "TST-VIRTUAL-COPY-CONTROLLER-COPY";
const WARD: &str = "TST-VIRTUAL-COPY-CONTROLLER-WARD";

fn definition(
    id: &'static str,
    types: BTreeSet<CardType>,
    keywords: Vec<Keyword>,
    effects: Vec<Effect>,
) -> CardDefinition {
    let creature = types.contains(&CardType::Creature);
    CardDefinition {
        id,
        name: id,
        set_code: "TST",
        mana_cost: ManaCost::new(0),
        colors: BTreeSet::new(),
        mana_colors: BTreeSet::new(),
        card_types: types,
        is_basic_land: false,
        supported_rules: &["virtual-spell-copy-controller-provenance-red"],
        power: creature.then_some(1),
        toughness: creature.then_some(4),
        keywords,
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
fn virtual_copy_uses_its_controller_for_friendly_source_damage_prevention() {
    let original_controller = PlayerId(0);
    let copy_controller = PlayerId(1);
    let mut game = Game::new(
        [
            definition(
                BOLT,
                BTreeSet::from([CardType::Instant]),
                vec![],
                vec![Effect::DealDamage {
                    amount: 2,
                    target: TargetRequirement::Creature,
                }],
            ),
            definition(
                COPY,
                BTreeSet::from([CardType::Instant]),
                vec![],
                vec![Effect::CopyTargetInstantOrSorcerySpell {
                    may_choose_new_targets: false,
                }],
            ),
            definition(
                WARD,
                BTreeSet::from([CardType::Creature]),
                vec![Keyword::PreventDamageFromControlledSources],
                vec![],
            ),
        ],
        2,
    )
    .expect("fixture initializes");
    let ward = game
        .put_on_battlefield(copy_controller, WARD)
        .expect("ward enters before game start");
    let bolt = game
        .add_card(original_controller, BOLT, Zone::Hand)
        .expect("bolt enters hand");
    let copy = game
        .add_card(copy_controller, COPY, Zone::Hand)
        .expect("copy enters hand");
    game.begin_game().expect("game begins");

    game.cast_spell(
        original_controller,
        request(bolt, vec![Target::Permanent(ward)]),
    )
    .expect("original bolt casts");
    game.pass_priority(original_controller)
        .expect("original controller passes");
    game.cast_spell(copy_controller, request(copy, vec![Target::Spell(bolt)]))
        .expect("opponent copies the bolt");
    resolve_top(&mut game).expect("copy effect resolves");

    let result = resolve_top(&mut game);
    eprintln!(
        "virtual-copy controller provenance red trace: result={result:?}; damage={}; events={:?}",
        game.object(ward).expect("ward remains").damage,
        game.canonical_event_log()
    );
    assert!(result.is_ok(), "virtual copied damage resolves");
    assert_eq!(
        game.object(ward).expect("ward remains").damage,
        0,
        "the copy controller's ward prevents damage from its controlled virtual copy"
    );
    game.validate_invariants()
        .expect("copy-controller prevention is replay-auditable");
}
