//! Red regression: a fixture attachment primitive cannot inject a live Aura.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    AttachmentBinding, AttachmentKind, CardDefinition, CardType, Effect, Game, GameEvent, ManaCost,
    PlayerId, TargetRequirement, Zone,
};

const CREATURE: &str = "TST-LIVE-ATTACHMENT-CREATURE";
const AURA: &str = "TST-LIVE-ATTACHMENT-AURA";

fn definition(id: &'static str, card_type: &CardType) -> CardDefinition {
    CardDefinition {
        id,
        name: id,
        set_code: "TST",
        mana_cost: ManaCost::new(0),
        colors: BTreeSet::new(),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([card_type.clone()]),
        is_basic_land: false,
        supported_rules: &["live-attachment-primitive-injection-red"],
        power: (card_type == &CardType::Creature).then_some(2),
        toughness: (card_type == &CardType::Creature).then_some(2),
        keywords: vec![],
        effects: (id == AURA)
            .then(|| Effect::AttachSourceToTarget {
                target: TargetRequirement::Creature,
                changes: vec![],
            })
            .into_iter()
            .collect(),
    }
}

#[test]
fn entering_an_aura_without_cast_rejects_a_live_call_without_priority_or_policy() {
    let mut game = Game::new(
        [
            definition(CREATURE, &CardType::Creature),
            definition(AURA, &CardType::Enchantment),
        ],
        2,
    )
    .expect("fixture initializes");
    game.register_attachment_bindings([AttachmentBinding {
        card_definition: AURA,
        kind: AttachmentKind::Aura,
        target: TargetRequirement::Creature,
        changes: vec![],
        granted_activated_abilities: vec![],
    }])
    .expect("Aura binding setup");
    let creature = game
        .put_on_battlefield(PlayerId(0), CREATURE)
        .expect("creature setup");
    let aura = game
        .add_card(PlayerId(0), AURA, Zone::Hand)
        .expect("Aura setup");
    game.begin_game().expect("game begins");
    game.pass_priority(PlayerId(0))
        .expect("player zero passes priority");
    assert_eq!(game.priority, PlayerId(1));
    let before_events = game.canonical_event_log();

    let result = game.enter_attachment_without_cast(aura, creature);
    eprintln!(
        "attachment result={result:?}; priority={:?}; zone={:?}; attached_to={:?}; events={:?}",
        game.priority,
        game.zone_of(aura),
        game.object(aura).expect("aura remains").attached_to,
        game.canonical_event_log()
    );

    assert!(
        result.is_err(),
        "a live Aura entry must not bypass priority and a policy move"
    );
    assert_eq!(game.canonical_event_log(), before_events);
    assert_eq!(game.zone_of(aura), Some(Zone::Hand));
    assert!(game.event_log.iter().all(|event| !matches!(
        event,
        GameEvent::AttachmentEstablishedWithoutContinuousEffect {
            attachment,
            kind: AttachmentKind::Aura,
            ..
        } if *attachment == aura
    )));
}
