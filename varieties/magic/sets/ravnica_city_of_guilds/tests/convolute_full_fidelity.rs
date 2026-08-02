//! Full-fidelity public contracts for Convolute's stack payment decision.

use cardbench_magic_engine::{
    BasicLandManaAbilityActivation, CastPaymentManaAbility, CastRequest, Color, DecisionKind,
    DecisionSelection, Game, GameEvent, ManaPaymentSelection, PlayerId,
    ResolutionPaymentManaAbility, RulesError, Zone,
};
use cardbench_magic_rav::{card_definitions, rav_basic_land_type_bindings};

fn game() -> Game {
    Game::new_with_basic_land_types(card_definitions(), 2, rav_basic_land_type_bindings())
        .expect("RAV typed basic-land bindings initialize")
}

fn cast_watchwolf_and_convolute(
    game: &mut Game,
) -> (
    cardbench_magic_engine::ObjectId,
    cardbench_magic_engine::ObjectId,
    Vec<cardbench_magic_engine::ObjectId>,
) {
    let p0 = PlayerId(0);
    let p1 = PlayerId(1);
    let forest = game.put_on_battlefield(p0, "RAV-FOREST").expect("forest");
    let plains = game.put_on_battlefield(p0, "RAV-PLAINS").expect("plains");
    let islands = (0..4)
        .map(|_| game.put_on_battlefield(p0, "RAV-ISLAND").expect("island"))
        .collect::<Vec<_>>();
    let counter_islands = (0..3)
        .map(|_| {
            game.put_on_battlefield(p1, "RAV-ISLAND")
                .expect("counter island")
        })
        .collect::<Vec<_>>();
    let watchwolf = game
        .add_card(p0, "RAV-WATCHWOLF", Zone::Hand)
        .expect("watchwolf");
    let convolute = game
        .add_card(p1, "RAV-CONVOLUTE", Zone::Hand)
        .expect("Convolute");

    game.cast_spell(
        p0,
        CastRequest {
            card: watchwolf,
            targets: vec![],
            convoke: vec![],
            payment_mana_abilities: vec![
                CastPaymentManaAbility::BasicLand(BasicLandManaAbilityActivation {
                    land: forest,
                    color: Color::Green,
                }),
                CastPaymentManaAbility::BasicLand(BasicLandManaAbilityActivation {
                    land: plains,
                    color: Color::White,
                }),
            ],
        },
    )
    .expect("Watchwolf is cast");
    game.pass_priority(p0).expect("caster passes");
    game.cast_spell(
        p1,
        CastRequest {
            card: convolute,
            targets: vec![cardbench_magic_engine::Target::Spell(watchwolf)],
            convoke: vec![],
            payment_mana_abilities: counter_islands
                .iter()
                .map(|land| {
                    CastPaymentManaAbility::BasicLand(BasicLandManaAbilityActivation {
                        land: *land,
                        color: Color::Blue,
                    })
                })
                .collect(),
        },
    )
    .expect("Convolute is cast");
    game.pass_priority(p1)
        .expect("counterspell controller passes");
    game.pass_priority(p0)
        .expect("resolution opens target controller payment decision");
    (watchwolf, convolute, islands)
}

#[test]
fn target_spell_controller_can_pay_four_with_explicit_mana_abilities() {
    let mut game = game();
    let p0 = PlayerId(0);
    let (watchwolf, convolute, islands) = cast_watchwolf_and_convolute(&mut game);
    let decision = game
        .view_for_player(p0)
        .expect("payment controller view")
        .pending_decision
        .expect("payment decision")
        .id;

    assert_eq!(
        game.submit_decision(
            p0,
            decision,
            DecisionSelection::CounterUnlessPaysMana {
                pay: true,
                mana_abilities: islands
                    .iter()
                    .map(|land| {
                        ResolutionPaymentManaAbility::IntrinsicLand(
                            BasicLandManaAbilityActivation {
                                land: *land,
                                color: Color::Blue,
                            },
                        )
                    })
                    .collect(),
                mana_selection: ManaPaymentSelection {
                    generic: vec![Color::Blue; 4],
                    hybrid: vec![],
                },
            },
        ),
        Ok(())
    );
    assert_eq!(game.stack.len(), 1, "the paid-for spell stays on stack");
    assert_eq!(game.stack[0].card, watchwolf);
    assert_eq!(game.zone_of(convolute), Some(Zone::Graveyard));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::CounterUnlessPaysManaPaid {
            player,
            source,
            target_spell,
            mana_spent,
            ..
        } if *player == p0 && *source == convolute && *target_spell == watchwolf && mana_spent == &vec![Color::Blue; 4]
    )));
    game.validate_invariants().expect("paid branch invariants");
}

#[test]
fn unaffordable_payment_is_atomic_and_explicit_decline_counters_the_spell() {
    let mut game = game();
    let p0 = PlayerId(0);
    let (watchwolf, convolute, _islands) = cast_watchwolf_and_convolute(&mut game);
    let decision = game
        .view_for_player(p0)
        .expect("payment controller view")
        .pending_decision
        .expect("payment decision")
        .id;
    let events_before = game.event_log.clone();
    assert_eq!(
        game.submit_decision(
            p0,
            decision,
            DecisionSelection::CounterUnlessPaysMana {
                pay: true,
                mana_abilities: vec![],
                mana_selection: ManaPaymentSelection {
                    generic: vec![Color::Blue; 4],
                    hybrid: vec![],
                },
            },
        ),
        Err(RulesError::Mana("missing Blue mana".to_owned()))
    );
    assert_eq!(
        game.event_log, events_before,
        "failed payment rolls back receipts"
    );
    assert_eq!(game.stack.len(), 2, "failed payment preserves both spells");

    game.submit_decision(
        p0,
        decision,
        DecisionSelection::CounterUnlessPaysMana {
            pay: false,
            mana_abilities: vec![],
            mana_selection: ManaPaymentSelection::default(),
        },
    )
    .expect("controller explicitly declines");
    assert_eq!(game.zone_of(watchwolf), Some(Zone::Graveyard));
    assert_eq!(game.zone_of(convolute), Some(Zone::Graveyard));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::DecisionOpened {
            kind: DecisionKind::CounterUnlessPaysMana,
            ..
        }
    )));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::SpellCountered { card, source } if *card == watchwolf && *source == convolute
    )));
    game.validate_invariants()
        .expect("declined branch invariants");
}
