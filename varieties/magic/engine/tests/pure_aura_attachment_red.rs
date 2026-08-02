//! Red regression for an Aura whose printed attachment has no continuous change.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    AttachmentBinding, AttachmentKind, CardDefinition, CardType, CastRequest, Effect, Game,
    ManaCost, PlayerId, Target, TargetRequirement, Zone,
};

const CREATURE: &str = "TST-PURE-AURA-CREATURE";
const AURA: &str = "TST-PURE-AURA";

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
        supported_rules: &["pure-aura-attachment-contract"],
        power: creature.then_some(1),
        toughness: creature.then_some(1),
        keywords: vec![],
        effects,
    }
}

fn advance_to_main(game: &mut Game) {
    for _ in 0..2 {
        let first = game.priority;
        game.pass_priority(first).expect("first player passes");
        let second = game.priority;
        game.pass_priority(second).expect("second player passes");
    }
}

#[test]
fn zero_change_aura_attaches_and_preserves_auditable_lifecycle() {
    let mut game = Game::new(
        vec![
            definition(CREATURE, CardType::Creature, vec![]),
            definition(
                AURA,
                CardType::Enchantment,
                vec![Effect::AttachSourceToTarget {
                    target: TargetRequirement::Creature,
                    changes: vec![],
                }],
            ),
        ],
        2,
    )
    .expect("synthetic pure-Aura game builds");
    let creature = game
        .put_on_battlefield(PlayerId(0), CREATURE)
        .expect("creature enters during setup");
    let aura = game
        .add_card(PlayerId(0), AURA, Zone::Hand)
        .expect("Aura enters hand during setup");
    game.begin_game().expect("game starts");
    advance_to_main(&mut game);
    game.cast_spell(
        PlayerId(0),
        CastRequest {
            card: aura,
            targets: vec![Target::Permanent(creature)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("zero-change Aura casts");
    game.pass_priority(PlayerId(0))
        .expect("Aura controller passes");
    game.pass_priority(PlayerId(1))
        .expect("zero-change Aura resolves");
    assert_eq!(game.zone_of(aura), Some(Zone::Battlefield));
    assert_eq!(
        game.object(aura).expect("Aura remains defined").attached_to,
        Some(creature)
    );
    game.validate_invariants()
        .expect("pure Aura attachment has a valid receipt lifecycle");
}

#[test]
fn explicit_zero_change_aura_binding_is_a_valid_typed_attachment_contract() {
    let mut game = Game::new(
        vec![definition(
            AURA,
            CardType::Enchantment,
            vec![Effect::AttachSourceToTarget {
                target: TargetRequirement::Creature,
                changes: vec![],
            }],
        )],
        2,
    )
    .expect("synthetic pure-Aura game builds");
    game.register_attachment_bindings([AttachmentBinding {
        card_definition: AURA,
        kind: AttachmentKind::Aura,
        target: TargetRequirement::Creature,
        changes: vec![],
    }])
    .expect("an explicit pure-Aura binding is accepted");
    game.validate_invariants()
        .expect("pure Aura binding preserves invariant validity");
}
