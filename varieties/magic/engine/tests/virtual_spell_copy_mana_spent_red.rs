//! Red regression: a spell copy has no copied mana-payment receipt.
//!
//! CR 707.10 copies choices such as modes and X, but not mana: a copied spell
//! with a “if {U} was spent” rider must not see the original's blue payment.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    BasicLandManaAbilityActivation, BasicLandType, BasicLandTypeBinding, CardDefinition, CardType,
    CastPaymentManaAbility, CastRequest, Color, Effect, Game, ManaCost, ManaPaymentSelection,
    PlayerId, Target, Zone,
};

const CONDITIONAL: &str = "TST-COPY-MANA-SPENT-CONDITIONAL";
const COPY: &str = "TST-COPY-MANA-SPENT-COPY";
const LIBRARY_CARD: &str = "TST-COPY-MANA-SPENT-LIBRARY";
const ISLAND: &str = "TST-COPY-MANA-SPENT-ISLAND";

fn definition(id: &'static str, mana_cost: ManaCost, effects: Vec<Effect>) -> CardDefinition {
    CardDefinition {
        id,
        name: id,
        set_code: "TST",
        mana_cost,
        colors: BTreeSet::new(),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Instant]),
        is_basic_land: false,
        supported_rules: &["virtual-spell-copy-mana-spent-red"],
        power: None,
        toughness: None,
        keywords: vec![],
        effects,
    }
}

fn library_card() -> CardDefinition {
    CardDefinition {
        id: LIBRARY_CARD,
        name: LIBRARY_CARD,
        set_code: "TST",
        mana_cost: ManaCost::new(0),
        colors: BTreeSet::new(),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Artifact]),
        is_basic_land: false,
        supported_rules: &["virtual-spell-copy-mana-spent-red"],
        power: None,
        toughness: None,
        keywords: vec![],
        effects: vec![],
    }
}

fn island() -> CardDefinition {
    CardDefinition {
        id: ISLAND,
        name: ISLAND,
        set_code: "TST",
        mana_cost: ManaCost::new(0),
        colors: BTreeSet::new(),
        mana_colors: BTreeSet::from([Color::Blue]),
        card_types: BTreeSet::from([CardType::Land]),
        is_basic_land: true,
        supported_rules: &["virtual-spell-copy-mana-spent-red"],
        power: None,
        toughness: None,
        keywords: vec![],
        effects: vec![],
    }
}

fn request(card: cardbench_magic_engine::ObjectId, targets: Vec<Target>) -> CastRequest {
    CastRequest {
        card,
        targets,
        convoke: vec![],
        payment_mana_abilities: vec![],
    }
}

fn resolve_top(game: &mut Game) {
    let first = game.priority;
    game.pass_priority(first).expect("first pass succeeds");
    let second = game.priority;
    game.pass_priority(second).expect("second pass succeeds");
}

#[test]
fn copied_spell_does_not_inherit_originals_spent_mana() {
    let caster = PlayerId(0);
    let copy_controller = PlayerId(1);
    let mut game = Game::new_with_basic_land_types(
        [
            definition(
                CONDITIONAL,
                ManaCost::new(1),
                vec![Effect::DrawControllerIfManaColorSpent { color: Color::Blue }],
            ),
            definition(
                COPY,
                ManaCost::new(0),
                vec![Effect::CopyTargetInstantOrSorcerySpell {
                    may_choose_new_targets: false,
                }],
            ),
            library_card(),
            island(),
        ],
        2,
        [BasicLandTypeBinding {
            card_definition: ISLAND,
            land_type: BasicLandType::Island,
        }],
    )
    .expect("fixture initializes");
    let conditional = game
        .add_card(caster, CONDITIONAL, Zone::Hand)
        .expect("conditional enters caster hand");
    let copy_effect = game
        .add_card(copy_controller, COPY, Zone::Hand)
        .expect("copy effect enters opponent hand");
    let original_draw = game
        .add_card(caster, LIBRARY_CARD, Zone::Library)
        .expect("original controller library card enters");
    let copied_draw = game
        .add_card(copy_controller, LIBRARY_CARD, Zone::Library)
        .expect("copy controller library card enters");
    let island = game
        .put_on_battlefield(caster, ISLAND)
        .expect("caster has an Island mana source");
    game.begin_game().expect("game begins");

    game.cast_spell_with_mana_spend(
        caster,
        CastRequest {
            card: conditional,
            targets: vec![],
            convoke: vec![],
            payment_mana_abilities: vec![CastPaymentManaAbility::BasicLand(
                BasicLandManaAbilityActivation {
                    land: island,
                    color: Color::Blue,
                },
            )],
        },
        ManaPaymentSelection {
            generic: vec![Color::Blue],
            hybrid: vec![],
        },
    )
    .expect("original casts with an explicit blue payment");
    game.pass_priority(caster).expect("caster passes priority");
    game.cast_spell(
        copy_controller,
        request(copy_effect, vec![Target::Spell(conditional)]),
    )
    .expect("opponent copies the paid conditional spell");
    resolve_top(&mut game);
    resolve_top(&mut game);

    eprintln!(
        "virtual-copy spent-mana red trace: copied_draw_zone={:?}; events={:?}",
        game.zone_of(copied_draw),
        game.canonical_event_log(),
    );
    assert_eq!(
        game.zone_of(copied_draw),
        Some(Zone::Library),
        "a spell copy was not cast and must not inherit blue mana spent on the original"
    );
    assert_eq!(
        game.zone_of(original_draw),
        Some(Zone::Library),
        "the physical original remains below the resolved copy"
    );
}
