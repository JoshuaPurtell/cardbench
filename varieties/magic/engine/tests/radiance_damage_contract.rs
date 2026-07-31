//! Expansion-neutral contracts for a radiance damage effect.
//!
//! The test cards deliberately encode only synthetic characteristics. They
//! prove target legality, one-pass shared-color selection, receipt order, and
//! rules-based countering independently of any expansion fixture.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, CastRequest, Color, Effect, Game, GameEvent, ManaCost, ObjectId,
    PlayerId, RulesError, Target, TargetRequirement, Zone,
};

const RADIANCE_BEAM: &str = "TST-RADIANCE-DAMAGE";
const REMOVAL: &str = "TST-REMOVAL";
const TARGET: &str = "TST-TARGET";
const RED_ALLY: &str = "TST-RED-ALLY";
const WHITE_ALLY: &str = "TST-WHITE-ALLY";
const OFF_COLOR: &str = "TST-OFF-COLOR";
const NONCREATURE: &str = "TST-NONCREATURE";

fn colors(colors: impl IntoIterator<Item = Color>) -> BTreeSet<Color> {
    colors.into_iter().collect()
}

fn types(types: impl IntoIterator<Item = CardType>) -> BTreeSet<CardType> {
    types.into_iter().collect()
}

fn creature(id: &'static str, creature_colors: impl IntoIterator<Item = Color>) -> CardDefinition {
    CardDefinition {
        id,
        name: id,
        set_code: "TST",
        mana_cost: ManaCost::new(0),
        colors: colors(creature_colors),
        mana_colors: BTreeSet::new(),
        card_types: types([CardType::Creature]),
        is_basic_land: false,
        supported_rules: &["base-characteristics"],
        power: Some(2),
        toughness: Some(2),
        keywords: vec![],
        effects: vec![],
    }
}

fn definitions() -> Vec<CardDefinition> {
    vec![
        CardDefinition {
            id: RADIANCE_BEAM,
            name: RADIANCE_BEAM,
            set_code: "TST",
            mana_cost: ManaCost::new(0),
            colors: colors([Color::Red]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Instant]),
            is_basic_land: false,
            supported_rules: &["radiance", "damage"],
            power: None,
            toughness: None,
            keywords: vec![],
            effects: vec![Effect::RadianceDealDamageToCreatures { amount: 2 }],
        },
        CardDefinition {
            id: REMOVAL,
            name: REMOVAL,
            set_code: "TST",
            mana_cost: ManaCost::new(0),
            colors: colors([Color::Red]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Instant]),
            is_basic_land: false,
            supported_rules: &["damage"],
            power: None,
            toughness: None,
            keywords: vec![],
            effects: vec![Effect::DealDamage {
                amount: 2,
                target: TargetRequirement::Creature,
            }],
        },
        creature(TARGET, [Color::Red, Color::White]),
        creature(RED_ALLY, [Color::Red]),
        creature(WHITE_ALLY, [Color::White]),
        creature(OFF_COLOR, [Color::Green]),
        CardDefinition {
            id: NONCREATURE,
            name: NONCREATURE,
            set_code: "TST",
            mana_cost: ManaCost::new(0),
            colors: colors([Color::Red]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Artifact]),
            is_basic_land: false,
            supported_rules: &["artifact-characteristics"],
            power: None,
            toughness: None,
            keywords: vec![],
            effects: vec![],
        },
    ]
}

fn game() -> Game {
    Game::new(definitions(), 2).expect("radiance damage contract game initializes")
}

fn cast(game: &mut Game, player: PlayerId, card: ObjectId, target: ObjectId) {
    game.cast_spell(
        player,
        CastRequest {
            card,
            targets: vec![Target::Permanent(target)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("prepared cast is legal");
}

fn resolve_top(game: &mut Game) {
    let first = game.priority;
    game.pass_priority(first).expect("first player passes");
    let second = game.priority;
    game.pass_priority(second)
        .expect("second player passes and resolves top object");
}

#[test]
fn radiance_damage_includes_target_and_each_shared_color_creature_once_before_sbas() {
    let mut game = game();
    let beam = game
        .add_card(PlayerId(0), RADIANCE_BEAM, Zone::Hand)
        .expect("beam enters hand");
    let target = game
        .put_on_battlefield(PlayerId(0), TARGET)
        .expect("target enters battlefield");
    let red_ally = game
        .put_on_battlefield(PlayerId(1), RED_ALLY)
        .expect("red ally enters battlefield");
    let white_ally = game
        .put_on_battlefield(PlayerId(1), WHITE_ALLY)
        .expect("white ally enters battlefield");
    let off_color = game
        .put_on_battlefield(PlayerId(1), OFF_COLOR)
        .expect("off-color creature enters battlefield");
    let noncreature = game
        .put_on_battlefield(PlayerId(1), NONCREATURE)
        .expect("same-color artifact enters battlefield");
    game.clear_event_log();

    cast(&mut game, PlayerId(0), beam, target);
    resolve_top(&mut game);

    for selected in [target, red_ally, white_ally] {
        assert_eq!(game.zone_of(selected), Some(Zone::Graveyard));
    }
    for unselected in [off_color, noncreature] {
        assert_eq!(game.zone_of(unselected), Some(Zone::Battlefield));
    }

    let damaged = game
        .event_log
        .iter()
        .filter_map(|event| match event {
            GameEvent::DamageDealtToPermanent {
                source,
                permanent,
                amount,
            } if *source == beam && *amount == 2 => Some(*permanent),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(damaged, vec![target, red_ally, white_ally]);
    assert_eq!(
        damaged.iter().copied().collect::<BTreeSet<_>>().len(),
        damaged.len(),
        "a multicolored target must not select the same peer twice"
    );
    let last_damage = game
        .event_log
        .iter()
        .rposition(|event| matches!(event, GameEvent::DamageDealtToPermanent { source, .. } if *source == beam))
        .expect("radiance damage receipts exist");
    let first_sba = game
        .event_log
        .iter()
        .position(|event| matches!(event, GameEvent::StateBasedAction { .. }))
        .expect("lethal creatures trigger SBAs");
    assert!(
        last_damage < first_sba,
        "the effect must apply every selected creature's damage before SBAs"
    );
    game.validate_invariants()
        .expect("radiance damage resolution preserves invariants");
}

#[test]
fn radiance_damage_rejects_noncreature_targets_at_cast_time() {
    let mut game = game();
    let beam = game
        .add_card(PlayerId(0), RADIANCE_BEAM, Zone::Hand)
        .expect("beam enters hand");
    let artifact = game
        .put_on_battlefield(PlayerId(1), NONCREATURE)
        .expect("artifact enters battlefield");

    assert_eq!(
        game.cast_spell(
            PlayerId(0),
            CastRequest {
                card: beam,
                targets: vec![Target::Permanent(artifact)],
                convoke: vec![],
                payment_mana_abilities: vec![],
            },
        ),
        Err(RulesError::IllegalTarget(Target::Permanent(artifact)))
    );
    assert_eq!(game.zone_of(beam), Some(Zone::Hand));
    assert!(game.stack.is_empty());
    game.validate_invariants()
        .expect("rejected target leaves game state valid");
}

#[test]
fn radiance_damage_revalidates_its_target_on_resolution() {
    let mut game = game();
    let beam = game
        .add_card(PlayerId(0), RADIANCE_BEAM, Zone::Hand)
        .expect("beam enters hand");
    let removal = game
        .add_card(PlayerId(1), REMOVAL, Zone::Hand)
        .expect("removal enters hand");
    let target = game
        .put_on_battlefield(PlayerId(1), TARGET)
        .expect("target enters battlefield");
    game.clear_event_log();

    cast(&mut game, PlayerId(0), beam, target);
    game.pass_priority(PlayerId(0))
        .expect("caster passes for response");
    cast(&mut game, PlayerId(1), removal, target);
    resolve_top(&mut game);
    assert_eq!(game.zone_of(target), Some(Zone::Graveyard));
    resolve_top(&mut game);

    assert!(
        game.event_log.iter().any(
            |event| matches!(event, GameEvent::SpellCounteredByRules { card } if *card == beam)
        )
    );
    assert!(
        !game.event_log.iter().any(|event| matches!(
            event,
            GameEvent::DamageDealtToPermanent { source, .. } if *source == beam
        )),
        "an illegal target prevents all radiance damage at resolution"
    );
    game.validate_invariants()
        .expect("rules-counter trace preserves invariants");
}
