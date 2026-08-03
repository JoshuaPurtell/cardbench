//! Red regression: a temporary damage redirection must retain its destination's
//! exact battlefield incarnation.
//!
//! The destination leaves and returns after the redirection resolves.  The
//! next damage to the protected creature must stay on that creature rather
//! than being redirected to the returned object with the same `ObjectId`.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    AbilityActivation, ActivatedAbility, ActivatedAbilityBinding, CardDefinition, CardType,
    CastRequest, Color, Effect, Game, Keyword, ManaCost, PlayerId, Target, TargetRequirement, Zone,
};

const REDIRECTOR: &str = "TST-REDIRECTION-INCARNATION-REDIRECTOR";
const PROTECTED: &str = "TST-REDIRECTION-INCARNATION-PROTECTED";
const DESTINATION: &str = "TST-REDIRECTION-INCARNATION-DESTINATION";
const BOUNCE: &str = "TST-REDIRECTION-INCARNATION-BOUNCE";
const BOLT: &str = "TST-REDIRECTION-INCARNATION-BOLT";

fn definition(
    id: &'static str,
    card_types: BTreeSet<CardType>,
    keywords: Vec<Keyword>,
    effects: Vec<Effect>,
) -> CardDefinition {
    CardDefinition {
        id,
        name: id,
        set_code: "TST",
        mana_cost: ManaCost::new(0),
        colors: BTreeSet::from([Color::Red]),
        mana_colors: BTreeSet::new(),
        card_types,
        is_basic_land: false,
        supported_rules: &["damage-redirection-destination-incarnation-red"],
        power: matches!(id, REDIRECTOR | PROTECTED | DESTINATION).then_some(4),
        toughness: matches!(id, REDIRECTOR | PROTECTED | DESTINATION).then_some(4),
        keywords,
        effects,
    }
}

fn pass_pair(game: &mut Game) {
    let first = game.priority;
    game.pass_priority(first).expect("first player passes");
    let second = game.priority;
    game.pass_priority(second).expect("second player passes");
}

#[test]
#[allow(clippy::too_many_lines)] // The transcript fixes both target identity and replacement timing.
fn temporary_redirection_does_not_follow_a_destination_through_a_zone_change() {
    let controller = PlayerId(0);
    let opponent = PlayerId(1);
    let mut game = Game::new_with_all_bindings(
        [
            definition(
                REDIRECTOR,
                BTreeSet::from([CardType::Creature]),
                vec![],
                vec![],
            ),
            definition(
                PROTECTED,
                BTreeSet::from([CardType::Creature]),
                vec![],
                vec![],
            ),
            definition(
                DESTINATION,
                BTreeSet::from([CardType::Creature]),
                vec![Keyword::Flash],
                vec![],
            ),
            definition(
                BOUNCE,
                BTreeSet::from([CardType::Instant]),
                vec![],
                vec![Effect::ReturnTargetPermanentToHandAndLoseControllerLife { amount: 1 }],
            ),
            definition(
                BOLT,
                BTreeSet::from([CardType::Instant]),
                vec![],
                vec![Effect::DealDamage {
                    amount: 2,
                    target: TargetRequirement::Creature,
                }],
            ),
        ],
        2,
        [],
        [],
        [],
        [ActivatedAbilityBinding {
            card_definition: REDIRECTOR,
            ability: ActivatedAbility {
                id: "redirect-two",
                mana_cost: ManaCost::new(0),
                tap_cost: false,
                sorcery_speed: false,
                additional_tap_creatures: 0,
                sacrifice_source: false,
                sacrifice_creatures: 0,
                sacrifice_lands: 0,
                discard_cards: 0,
                targets: vec![
                    TargetRequirement::Creature,
                    TargetRequirement::PlayerOrCreature,
                ],
                effects: vec![
                    Effect::BeginDamageRedirection { amount: 2 },
                    Effect::CompleteDamageRedirection,
                ],
            },
        }],
    )
    .expect("fixture initializes");
    let redirector = game
        .put_on_battlefield(controller, REDIRECTOR)
        .expect("redirector setup");
    let protected = game
        .put_on_battlefield(controller, PROTECTED)
        .expect("protected creature setup");
    let destination = game
        .put_on_battlefield(controller, DESTINATION)
        .expect("destination setup");
    let bounce = game
        .add_card(controller, BOUNCE, Zone::Hand)
        .expect("bounce setup");
    let bolt = game
        .add_card(opponent, BOLT, Zone::Hand)
        .expect("bolt setup");
    game.begin_game().expect("game begins");

    game.activate_ability(
        controller,
        AbilityActivation {
            source: redirector,
            ability_id: "redirect-two",
            sacrifice_sources: vec![],
            additional_tap_creatures: vec![],
            discard_cards: vec![],
            targets: vec![Target::Permanent(protected), Target::Permanent(destination)],
        },
    )
    .expect("redirection activates");
    pass_pair(&mut game);

    game.cast_spell(
        controller,
        CastRequest {
            card: bounce,
            targets: vec![Target::Permanent(destination)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("destination bounce casts");
    pass_pair(&mut game);
    assert_eq!(game.zone_of(destination), Some(Zone::Hand));

    game.cast_spell(
        controller,
        CastRequest {
            card: destination,
            targets: vec![],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("flash destination recasts");
    pass_pair(&mut game);
    assert_eq!(game.zone_of(destination), Some(Zone::Battlefield));

    game.pass_priority(controller)
        .expect("controller yields bolt priority");
    game.cast_spell(
        opponent,
        CastRequest {
            card: bolt,
            targets: vec![Target::Permanent(protected)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("bolt casts after destination returns");
    pass_pair(&mut game);

    eprintln!(
        "destination-incarnation trace: protected_damage={:?}; destination_damage={:?}; events={:?}",
        game.object(protected).map(|object| object.damage),
        game.object(destination).map(|object| object.damage),
        game.canonical_event_log(),
    );
    assert_eq!(
        game.object(protected)
            .expect("protected remains live")
            .damage,
        2,
        "the old redirection destination is no longer the object it targeted",
    );
    assert_eq!(
        game.object(destination)
            .expect("returned destination remains live")
            .damage,
        0,
        "a returned destination must not inherit an old redirection effect",
    );
    game.validate_invariants()
        .expect("destination-zone-change boundary remains invariant-valid");
}
