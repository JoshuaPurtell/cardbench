//! Red regression: target legality must read the current type layer rather
//! than the printed type line.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, CastRequest, ContinuousChange, Duration, Effect, Game, ManaCost,
    PlayerId, Target, Zone,
};

const TYPE_SOURCE: &str = "DERIVED-TYPE-TARGET-SOURCE";
const CREATURE: &str = "DERIVED-TYPE-TARGET-CREATURE";
const DESTROY_LAND: &str = "DERIVED-TYPE-TARGET-DESTROY-LAND";
const DESTROY_ARTIFACT: &str = "DERIVED-TYPE-TARGET-DESTROY-ARTIFACT";

fn definition(
    id: &'static str,
    card_types: BTreeSet<CardType>,
    effects: Vec<Effect>,
) -> CardDefinition {
    let is_creature = card_types.contains(&CardType::Creature);
    CardDefinition {
        id,
        name: id,
        set_code: "TST",
        mana_cost: ManaCost::new(0),
        colors: BTreeSet::new(),
        mana_colors: BTreeSet::new(),
        card_types,
        is_basic_land: false,
        supported_rules: &["derived-type-target-legality-probe"],
        power: is_creature.then_some(2),
        toughness: is_creature.then_some(2),
        keywords: vec![],
        effects,
    }
}

fn pass_pair(game: &mut Game) {
    let first = game.priority;
    game.pass_priority(first).expect("first player passes");
    let second = game.priority;
    game.pass_priority(second).expect("second player passes");
}

#[allow(clippy::too_many_lines)] // Cast and resolution both exercise the same derived-type target contract.
fn assert_derived_type_is_a_legal_target(
    derived_type: &CardType,
    spell: &'static str,
    effect: Effect,
) {
    let player = PlayerId(0);
    let mut game = Game::new(
        [
            definition(TYPE_SOURCE, BTreeSet::from([CardType::Enchantment]), vec![]),
            definition(CREATURE, BTreeSet::from([CardType::Creature]), vec![]),
            definition(spell, BTreeSet::from([CardType::Instant]), vec![effect]),
        ],
        2,
    )
    .expect("fixture initializes");
    let source = game
        .put_on_battlefield(player, TYPE_SOURCE)
        .expect("type-changing source begins on the battlefield");
    let target = game
        .put_on_battlefield(player, CREATURE)
        .expect("creature begins on the battlefield");
    let destroy = game
        .add_card(player, spell, Zone::Hand)
        .expect("typed destruction spell begins in hand");
    game.begin_game().expect("game begins");
    game.add_continuous_effect(
        source,
        target,
        ContinuousChange::AddCardType(derived_type.clone()),
        Duration::Permanent,
    )
    .expect("a legal type-layer effect changes the creature's current type");
    assert!(
        game.characteristics(target)
            .expect("target has derived characteristics")
            .card_types
            .contains(derived_type),
        "the live type layer, not the catalog definition, determines the target type",
    );

    game.cast_spell(
        player,
        CastRequest {
            card: destroy,
            targets: vec![Target::Permanent(target)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("a live derived type is a legal target for the matching spell");
    pass_pair(&mut game);
    assert_eq!(game.zone_of(target), Some(Zone::Graveyard));
    eprintln!(
        "derived {:?} target trace={:?}",
        derived_type,
        game.canonical_event_log()
    );
    game.validate_invariants()
        .expect("derived-type target legality remains valid through resolution");
}

#[test]
fn a_creature_made_into_a_land_is_a_legal_land_target() {
    assert_derived_type_is_a_legal_target(&CardType::Land, DESTROY_LAND, Effect::DestroyTargetLand);
}

#[test]
fn a_creature_made_into_an_artifact_is_a_legal_artifact_target() {
    assert_derived_type_is_a_legal_target(
        &CardType::Artifact,
        DESTROY_ARTIFACT,
        Effect::DestroyTargetArtifact,
    );
}
