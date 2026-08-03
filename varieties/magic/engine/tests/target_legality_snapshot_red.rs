//! Red regression: target legality is established once when resolution begins.
//!
//! A target that was legal as a spell began resolving remains the same target
//! through later instructions.  In particular, an earlier instruction cannot
//! make the source's later instruction fail merely by granting that permanent
//! protection from the source's color.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, CastRequest, Color, Effect, Game, GameEvent, Keyword, ManaCost,
    PlayerId, Target, Zone,
};

const TARGET: &str = "TST-TARGET-LEGALITY-SNAPSHOT-CREATURE";
const SPELL: &str = "TST-TARGET-LEGALITY-SNAPSHOT-SPELL";

fn creature() -> CardDefinition {
    CardDefinition {
        id: TARGET,
        name: TARGET,
        set_code: "TST",
        mana_cost: ManaCost::new(0),
        colors: BTreeSet::from([Color::Blue]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Creature]),
        is_basic_land: false,
        supported_rules: &["target-legality-snapshot-red"],
        power: Some(2),
        toughness: Some(2),
        keywords: vec![],
        effects: vec![],
    }
}

fn spell() -> CardDefinition {
    CardDefinition {
        id: SPELL,
        name: SPELL,
        set_code: "TST",
        mana_cost: ManaCost::new(0),
        colors: BTreeSet::from([Color::Red]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Instant]),
        is_basic_land: false,
        supported_rules: &["target-legality-snapshot-red"],
        power: None,
        toughness: None,
        keywords: vec![],
        effects: vec![
            Effect::ModifyTargetKeywordUntilEndOfTurn {
                keyword: Keyword::Protection(Color::Red),
            },
            Effect::DealDamage {
                amount: 1,
                target: cardbench_magic_engine::TargetRequirement::Creature,
            },
        ],
    }
}

#[test]
fn earlier_protection_does_not_retroactively_make_a_later_target_instruction_illegal() {
    let mut game = Game::new([creature(), spell()], 2).expect("fixture initializes");
    let target = game
        .put_on_battlefield(PlayerId(1), TARGET)
        .expect("target setup");
    let card = game
        .add_card(PlayerId(0), SPELL, Zone::Hand)
        .expect("spell setup");
    game.begin_game().expect("game begins");

    game.cast_spell(
        PlayerId(0),
        CastRequest {
            card,
            targets: vec![Target::Permanent(target), Target::Permanent(target)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("both target occurrences are legal before resolution");
    game.pass_priority(PlayerId(0))
        .expect("caster passes to resolution");
    game.pass_priority(PlayerId(1))
        .expect("opponent resolves the spell");

    eprintln!(
        "damage={}; characteristics={:?}; events={:?}",
        game.object(target).expect("target remains").damage,
        game.characteristics(target)
            .expect("target characteristics"),
        game.canonical_event_log()
    );
    assert_eq!(
        game.object(target).expect("target remains").damage,
        1,
        "protection granted by the first instruction cannot retroactively skip the second"
    );
    assert!(game.event_log.iter().all(|event| !matches!(
        event,
        GameEvent::TargetInstructionSkipped { card: skipped, effect_index: 1, .. }
            if *skipped == card
    )));
    game.validate_invariants()
        .expect("the complete resolution retains valid state provenance");
}
