//! Red regression for an Aura modifier based on the enchanted controller's team.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, CastRequest, ContinuousChange, Effect, Game, ManaCost, PlayerId,
    Target, TargetRequirement, Zone,
};

const CREATURE: &str = "TST-DYNAMIC-ATTACHED-COUNT-CREATURE";
const AURA: &str = "TST-DYNAMIC-ATTACHED-COUNT-AURA";

fn definition(id: &'static str, card_type: CardType, effects: Vec<Effect>) -> CardDefinition {
    let creature = card_type == CardType::Creature;
    CardDefinition {
        id,
        name: id,
        set_code: "TST",
        mana_cost: ManaCost::new(0),
        colors: BTreeSet::new(),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([card_type]),
        is_basic_land: false,
        supported_rules: &["dynamic-attached-creature-count-contract"],
        power: creature.then_some(1),
        toughness: creature.then_some(1),
        keywords: vec![],
        effects,
    }
}

#[test]
fn attached_modifier_counts_other_creatures_of_its_targets_controller() {
    let mut game = Game::new(
        vec![
            definition(CREATURE, CardType::Creature, vec![]),
            definition(
                AURA,
                CardType::Enchantment,
                vec![Effect::AttachSourceToTarget {
                    target: TargetRequirement::Creature,
                    changes: vec![
                        ContinuousChange::ModifyPowerToughnessForEachOtherCreatureControlledByTarget {
                            power_per_creature: 0,
                            toughness_per_creature: 2,
                        },
                    ],
                }],
            ),
        ],
        2,
    )
    .expect("synthetic fixture builds");
    let target = game
        .put_on_battlefield(PlayerId(0), CREATURE)
        .expect("enchanted creature setup");
    game.put_on_battlefield(PlayerId(0), CREATURE)
        .expect("first other creature setup");
    game.put_on_battlefield(PlayerId(0), CREATURE)
        .expect("second other creature setup");
    let aura = game
        .add_card(PlayerId(0), AURA, Zone::Hand)
        .expect("Aura setup");
    game.begin_game().expect("game starts");
    for _ in 0..2 {
        let first = game.priority;
        game.pass_priority(first).expect("first player passes");
        let second = game.priority;
        game.pass_priority(second).expect("second player passes");
    }
    game.cast_spell(
        PlayerId(0),
        CastRequest {
            card: aura,
            targets: vec![Target::Permanent(target)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("Aura casts");
    game.pass_priority(PlayerId(0))
        .expect("Aura controller passes");
    game.pass_priority(PlayerId(1)).expect("Aura resolves");
    assert_eq!(
        game.characteristics(target)
            .expect("enchanted creature characteristics")
            .toughness,
        Some(5),
    );
    game.validate_invariants()
        .expect("dynamic attached modifier is a valid continuous effect");
}
