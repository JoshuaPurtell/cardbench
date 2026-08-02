//! RED: an Aura-relative exile effect must retain its exact attachment group
//! and schedule an incarnation-aware delayed return.
//!
//! This uses only synthetic `TST-*` definitions. It intentionally starts
//! from the pre-substrate `ExileTargetCreature` behavior: that effect exiles
//! the creature but loses its attached Aura to state-based actions, so there
//! is no group available for a later deterministic return.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    AbilityActivation, ActivatedAbility, ActivatedAbilityBinding, CardDefinition, CardType,
    CastRequest, ContinuousChange, Effect, Game, ManaCost, PlayerId, Target,
    TargetRequirement, Zone,
};

const CREATURE: &str = "TST-LINKED-CREATURE";
const AURA: &str = "TST-LINKED-AURA";

fn definition(
    id: &'static str,
    card_types: BTreeSet<CardType>,
    power: Option<i16>,
    toughness: Option<i16>,
    effects: Vec<Effect>,
) -> CardDefinition {
    CardDefinition {
        id,
        name: id,
        set_code: "TST",
        mana_cost: ManaCost::new(0),
        colors: BTreeSet::new(),
        mana_colors: BTreeSet::new(),
        card_types,
        is_basic_land: false,
        supported_rules: &["linked-exile-delayed-return-probe"],
        power,
        toughness,
        keywords: vec![],
        effects,
    }
}

fn game() -> Game {
    Game::new_with_all_bindings(
        vec![
            definition(
                CREATURE,
                BTreeSet::from([CardType::Creature]),
                Some(2),
                Some(2),
                vec![],
            ),
            definition(
                AURA,
                BTreeSet::from([CardType::Enchantment]),
                None,
                None,
                vec![Effect::AttachSourceToTarget {
                    target: TargetRequirement::Creature,
                    changes: vec![ContinuousChange::ModifyPowerToughness {
                        power: 1,
                        toughness: 1,
                    }],
                }],
            ),
        ],
        2,
        [],
        [],
        [],
        [ActivatedAbilityBinding {
            card_definition: AURA,
            ability: ActivatedAbility {
                id: "exile-linked-group",
                mana_cost: ManaCost::new(0),
                tap_cost: false,
                sorcery_speed: false,
                additional_tap_creatures: 0,
                sacrifice_source: false,
                sacrifice_creatures: 0,
                sacrifice_lands: 0,
                discard_cards: 0,
                targets: vec![TargetRequirement::Creature],
                effects: vec![Effect::ExileTargetCreature],
            },
        }],
    )
    .expect("synthetic fixture initializes")
}

fn pass_pair(game: &mut Game) {
    let first = game.priority;
    game.pass_priority(first).expect("first pass succeeds");
    let second = game.priority;
    game.pass_priority(second).expect("second pass succeeds");
}

#[test]
fn attached_group_exiles_together_instead_of_orphaning_the_aura() {
    let controller = PlayerId(0);
    let mut game = game();
    let creature = game
        .put_on_battlefield(controller, CREATURE)
        .expect("creature enters before the game begins");
    let aura = game
        .add_card(controller, AURA, Zone::Hand)
        .expect("Aura enters hand before the game begins");
    game.begin_game().expect("game begins");
    pass_pair(&mut game); // Upkeep → Draw
    pass_pair(&mut game); // Draw → precombat main

    game.cast_spell(
        controller,
        CastRequest {
            card: aura,
            targets: vec![Target::Permanent(creature)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("Aura spell casts");
    pass_pair(&mut game);
    assert_eq!(game.zone_of(aura), Some(Zone::Battlefield));

    game.activate_ability(
        controller,
        AbilityActivation {
            source: aura,
            ability_id: "exile-linked-group",
            sacrifice_sources: vec![],
            additional_tap_creatures: vec![],
            discard_cards: vec![],
            targets: vec![Target::Permanent(creature)],
        },
    )
    .expect("synthetic group ability enters the stack");
    pass_pair(&mut game);

    assert_eq!(game.zone_of(creature), Some(Zone::Exile));
    assert_eq!(
        game.zone_of(aura),
        Some(Zone::Exile),
        "an attached Aura must remain in the exact linked exile group rather than die as an orphan",
    );
}
