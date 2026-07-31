use std::collections::BTreeSet;

use cardbench_magic_engine::{
    BasicLandManaAbilityActivation, CardType, CastPaymentManaAbility, CastRequest, Color,
    CombatBlock, Game, GameEvent, Keyword, ManaAbilityActivation, ManaAbilityOutput, ManaCost,
    PlayerId, RulesError, Step, Zone,
};
use cardbench_magic_rav::{
    RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, rav_basic_land_type_bindings,
    rav_mana_ability_bindings, run_all_scenarios,
};

fn rav_game() -> Game {
    Game::new_with_mana_abilities_and_basic_land_types(
        card_definitions(),
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
    )
    .expect("RAV bindings construct a game")
}

fn advance_to_precombat_main(game: &mut Game) {
    game.begin_game().expect("fixture starts game");
    for _ in 0..2 {
        game.pass_priority(PlayerId(0))
            .expect("active player passes through early step");
        game.pass_priority(PlayerId(1))
            .expect("opponent passes through early step");
    }
    assert_eq!(game.step, Step::PrecombatMain);
    assert_eq!(game.priority, PlayerId(0));
}

#[test]
fn birds_of_paradise_has_complete_flying_choice_mana_definition_and_binding() {
    let definitions = card_definitions();
    let birds = definitions
        .iter()
        .find(|definition| definition.id == "RAV-BIRDS-OF-PARADISE")
        .expect("RAV Birds of Paradise definition");
    assert_eq!(birds.name, "Birds of Paradise");
    assert_eq!(birds.mana_cost, ManaCost::with_colors(0, [Color::Green]));
    assert_eq!(birds.colors, BTreeSet::from([Color::Green]));
    assert_eq!(birds.card_types, BTreeSet::from([CardType::Creature]));
    assert_eq!(birds.power, Some(0));
    assert_eq!(birds.toughness, Some(1));
    assert_eq!(birds.keywords, [Keyword::Flying]);
    assert!(birds.effects.is_empty());
    assert_eq!(
        birds.supported_rules,
        [
            "full-rules-fidelity",
            "colored-cost-casting",
            "base-characteristics",
            "flying",
            "tap-choice-single-color-mana-ability",
            "cast-payment-mana-activation",
        ]
    );
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&birds.id));

    let bindings = rav_mana_ability_bindings();
    let birds_bindings = bindings
        .iter()
        .filter(|binding| binding.card_definition == birds.id)
        .collect::<Vec<_>>();
    assert_eq!(birds_bindings.len(), 1);
    let binding = birds_bindings[0];
    assert_eq!(binding.card_definition, birds.id);
    assert_eq!(binding.ability.id, "produce-one-color");
    assert!(binding.ability.tap_cost);
    assert_eq!(binding.ability.amount, 1);
    assert_eq!(binding.ability.life_payment, None);
    assert_eq!(
        binding.ability.output,
        ManaAbilityOutput::Choice(BTreeSet::from(Color::ALL))
    );
}

#[test]
fn birds_direct_mana_ability_accepts_each_explicit_color_and_rejects_summoning_sickness() {
    for color in Color::ALL {
        let mut game = rav_game();
        let birds = game
            .add_card(PlayerId(0), "RAV-BIRDS-OF-PARADISE", Zone::Battlefield)
            .expect("Birds begins on the battlefield");
        game.set_entered_turn_for_setup(birds, 0)
            .expect("Birds is established before the measured turn");
        advance_to_precombat_main(&mut game);
        game.clear_event_log();

        game.activate_bound_mana_ability(
            PlayerId(0),
            ManaAbilityActivation {
                source: birds,
                ability_id: "produce-one-color",
                chosen_color: Some(color),
            },
        )
        .expect("the controller selects one legal color");

        assert!(game.stack.is_empty(), "mana abilities never use the stack");
        assert_eq!(game.priority, PlayerId(0));
        assert!(game.object(birds).expect("Birds object exists").tapped);
        assert_eq!(
            game.player(PlayerId(0))
                .expect("controller exists")
                .mana_pool
                .amount(color),
            1
        );
        assert_eq!(
            game.event_log,
            vec![
                GameEvent::BoundManaAbilityActivated {
                    player: PlayerId(0),
                    source: birds,
                    ability: "produce-one-color",
                    color,
                    amount: 1,
                    tapped: true,
                    life_payment: None,
                },
                GameEvent::ManaAdded {
                    player: PlayerId(0),
                    color,
                    amount: 1,
                },
            ]
        );
        game.validate_invariants()
            .expect("each selected output preserves engine invariants");
    }

    let mut game = rav_game();
    let birds = game
        .add_card(PlayerId(0), "RAV-BIRDS-OF-PARADISE", Zone::Battlefield)
        .expect("new Birds begins on the battlefield");
    advance_to_precombat_main(&mut game);
    game.clear_event_log();
    assert_eq!(
        game.activate_bound_mana_ability(
            PlayerId(0),
            ManaAbilityActivation {
                source: birds,
                ability_id: "produce-one-color",
                chosen_color: Some(Color::Blue),
            },
        ),
        Err(RulesError::IllegalAction(
            "a summoning-sick creature cannot pay a tap mana-ability cost"
        ))
    );
    assert!(!game.object(birds).expect("Birds object exists").tapped);
    assert!(game.event_log.is_empty(), "rejection is atomic");
    game.validate_invariants()
        .expect("summoning-sickness rejection preserves engine invariants");
}

#[test]
fn birds_cast_payment_keeps_receipts_ordered_and_resolves_only_the_spell_on_the_stack() {
    let mut game = rav_game();
    let birds = game
        .add_card(PlayerId(0), "RAV-BIRDS-OF-PARADISE", Zone::Battlefield)
        .expect("Birds begins on the battlefield");
    game.set_entered_turn_for_setup(birds, 0)
        .expect("Birds predates the measured turn");
    let forest = game
        .add_card(PlayerId(0), "RAV-FOREST", Zone::Battlefield)
        .expect("Forest begins on the battlefield");
    let watchwolf = game
        .add_card(PlayerId(0), "RAV-WATCHWOLF", Zone::Hand)
        .expect("spell begins in the controller hand");
    advance_to_precombat_main(&mut game);
    game.clear_event_log();

    game.cast_spell(
        PlayerId(0),
        CastRequest {
            card: watchwolf,
            targets: vec![],
            convoke: vec![],
            payment_mana_abilities: vec![
                CastPaymentManaAbility::Bound(ManaAbilityActivation {
                    source: birds,
                    ability_id: "produce-one-color",
                    chosen_color: Some(Color::White),
                }),
                CastPaymentManaAbility::BasicLand(BasicLandManaAbilityActivation {
                    land: forest,
                    color: Color::Green,
                }),
            ],
        },
    )
    .expect("Birds plus Forest pay the two colored symbols atomically");

    let position = |needle: &GameEvent| {
        game.event_log
            .iter()
            .position(|event| event == needle)
            .unwrap_or_else(|| panic!("missing cast-payment receipt {needle:?}"))
    };
    let contextual_birds = position(&GameEvent::CastPaymentManaAbilityActivated {
        player: PlayerId(0),
        card: watchwolf,
        source: birds,
        ability: "produce-one-color",
    });
    let birds_output = position(&GameEvent::BoundManaAbilityActivated {
        player: PlayerId(0),
        source: birds,
        ability: "produce-one-color",
        color: Color::White,
        amount: 1,
        tapped: true,
        life_payment: None,
    });
    let birds_mana = position(&GameEvent::ManaAdded {
        player: PlayerId(0),
        color: Color::White,
        amount: 1,
    });
    let contextual_forest = position(&GameEvent::CastPaymentBasicLandManaAbilityActivated {
        player: PlayerId(0),
        card: watchwolf,
        land: forest,
        color: Color::Green,
    });
    let forest_activation = position(&GameEvent::ManaAbilityActivated {
        player: PlayerId(0),
        land: forest,
        color: Color::Green,
    });
    let forest_mana = position(&GameEvent::ManaAdded {
        player: PlayerId(0),
        color: Color::Green,
        amount: 1,
    });
    let spell_cast = position(&GameEvent::SpellCast {
        player: PlayerId(0),
        card: watchwolf,
    });
    assert!(contextual_birds < birds_output && birds_output < birds_mana);
    assert!(birds_mana < contextual_forest && contextual_forest < forest_activation);
    assert!(forest_activation < forest_mana && forest_mana < spell_cast);
    assert_eq!(game.stack.len(), 1, "only the spell becomes a stack object");
    assert_eq!(game.priority, PlayerId(0));

    game.pass_priority(PlayerId(0))
        .expect("caster passes with the spell on the stack");
    game.pass_priority(PlayerId(1))
        .expect("opponent pass resolves the spell");
    assert_eq!(game.zone_of(watchwolf), Some(Zone::Battlefield));
    assert!(
        game.event_log.iter().any(
            |event| matches!(event, GameEvent::SpellResolved { card, .. } if *card == watchwolf)
        )
    );
    game.validate_invariants()
        .expect("Birds cast-payment transaction preserves invariants");
}

#[test]
fn birds_flying_rejects_a_ground_blocker_without_writing_a_block_declaration() {
    let mut game = rav_game();
    let birds = game
        .add_card(PlayerId(0), "RAV-BIRDS-OF-PARADISE", Zone::Battlefield)
        .expect("Birds begins on the battlefield");
    let ground_blocker = game
        .add_card(PlayerId(1), "RAV-FRENZIED-GOBLIN", Zone::Battlefield)
        .expect("ground blocker begins on the battlefield");
    game.set_entered_turn_for_setup(birds, 0)
        .expect("Birds predates the measured turn");
    game.set_entered_turn_for_setup(ground_blocker, 0)
        .expect("blocker predates the measured turn");
    advance_to_precombat_main(&mut game);
    for _ in 0..2 {
        game.pass_priority(PlayerId(0))
            .expect("active player passes toward combat");
        game.pass_priority(PlayerId(1))
            .expect("opponent passes toward combat");
    }
    assert_eq!(game.step, Step::DeclareAttackers);
    game.declare_attackers(PlayerId(0), &[birds])
        .expect("long-controlled Birds can attack");
    game.pass_priority(PlayerId(0))
        .expect("attacker controller passes after declaration");
    game.pass_priority(PlayerId(1))
        .expect("defender passes into blocker declaration");
    assert_eq!(game.step, Step::DeclareBlockers);
    let events_before_rejection = game.event_log.clone();

    assert_eq!(
        game.declare_blockers(
            PlayerId(1),
            &[CombatBlock {
                attacker: birds,
                blocker: ground_blocker,
            }],
        ),
        Err(RulesError::IllegalAction(
            "flying attacker can be blocked only by flying or reach"
        ))
    );
    assert_eq!(game.event_log, events_before_rejection);
    assert!(game.object(birds).expect("Birds exists").tapped);
    assert!(!game.object(ground_blocker).expect("blocker exists").tapped);
    game.validate_invariants()
        .expect("rejected Flying block preserves engine invariants");
}

#[test]
fn public_birds_scenarios_cover_turn_progression_and_flying_cast_payment() {
    let scenario = run_all_scenarios()
        .expect("public RAV scenarios execute")
        .into_iter()
        .find(|result| result.id == "rav_birds_of_paradise_bound_mana_ability")
        .expect("Birds of Paradise public scenario");

    assert_eq!(scenario.digest, "fnv1a64:7115881e15dd0d84");
    assert!(scenario.event_log.iter().any(|event| {
        event.contains("StepBegan { turn: 2, active_player: PlayerId(1), step: Upkeep }")
    }));
    assert!(scenario.event_log.iter().any(|event| {
        event.contains("BoundManaAbilityActivated")
            && event.contains("player: PlayerId(0)")
            && event.contains("ability: \"produce-one-color\"")
            && event.contains("color: Blue")
            && event.contains("amount: 1")
            && event.contains("tapped: true")
    }));
    assert!(scenario.event_log.iter().any(|event| {
        event.contains("ManaAdded { player: PlayerId(0), color: Blue, amount: 1 }")
    }));

    let cast_payment = run_all_scenarios()
        .expect("public RAV scenarios execute")
        .into_iter()
        .find(|result| result.id == "rav_birds_of_paradise_flying_cast_payment")
        .expect("Birds flying cast-payment scenario");
    println!(
        "Birds flying cast-payment trace: {:?}",
        cast_payment.event_log
    );
    for marker in [
        "CastPaymentManaAbilityActivated",
        "BoundManaAbilityActivated",
        "CastPaymentBasicLandManaAbilityActivated",
        "SpellCast",
        "SpellResolved",
    ] {
        assert!(
            cast_payment
                .event_log
                .iter()
                .any(|event| event.contains(marker)),
            "Birds cast-payment trace lacks {marker}: {:?}",
            cast_payment.event_log
        );
    }
    assert!(
        cast_payment.event_log.iter().all(
            |event| !event.contains("AttackersDeclared") && !event.contains("BlockersDeclared")
        ),
        "only the spell, never its mana sources, is represented on the stack"
    );
}
