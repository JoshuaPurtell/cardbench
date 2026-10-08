//! Full-game diagnostic authority for RAV Watery Grave.

use cardbench_magic_engine::{
    CardType, Color, DeckEntry, DeckList, Game, GameEvent, GameView, Keyword, LandEntryBinding,
    ManaAbilityActivation, ManaAbilityBinding, ManaAbilityOutput, ManaCost, ManaPaymentSelection,
    ObjectId, PlayerId, PolicyAction, Step, Zone,
};
use cardbench_magic_policies::CodePolicy;
use cardbench_magic_protocol::ViewerScope;
use cardbench_magic_rav::{
    CardSemanticStatus, CatalogCard, RAV_MAIN_SET_EXPECTED_PRINTING_COUNT,
    RAV_MAIN_SET_EXPECTED_UNIQUE_NAME_COUNT, card_definitions, rav_activated_ability_bindings,
    rav_activated_ability_cost_modifier_bindings, rav_additional_spell_cost_bindings,
    rav_attachment_bindings, rav_attachment_triggered_ability_bindings as attachment_triggers,
    rav_basic_land_type_bindings, rav_cost_reduction_bindings,
    rav_damage_replacement_effect_bindings, rav_entry_coin_flip_bindings, rav_entry_copy_bindings,
    rav_generalized_activated_ability_cost_bindings, rav_land_entry_bindings,
    rav_legendary_permanent_bindings, rav_main_set_catalog, rav_mana_ability_bindings,
    rav_mana_ability_cost_bindings, rav_replacement_effect_bindings,
    rav_static_attack_restriction_bindings, rav_static_continuous_effect_bindings,
    rav_static_creature_spell_cost_modifier_bindings, rav_static_entry_restriction_bindings,
    rav_static_library_top_reveal_bindings, rav_triggered_ability_bindings,
};
use cardbench_magic_session::{ProjectedEvent, project_events};
use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::process::ExitCode;

const TASK_ID: &str = "cardbench/magic/card";
const CARD_ID: &str = "RAV-WATERY-GRAVE";
const CARD_NAME: &str = "Watery Grave";
const MANA_ABILITY_ID: &str = "produce-blue-or-black";
const SPRITE_ID: &str = "RAV-SURVEILLING-SPRITE";
const WIGHT_ID: &str = "RAV-ROOFSTALKER-WIGHT";
const SOURCE_PATH: &str = "varieties/magic/sets/ravnica_city_of_guilds/src/cards/watery_grave.rs";
const MAX_SEED: u64 = 128;

#[path = "../../../sets/ravnica_city_of_guilds/src/cards/watery_grave.rs"]
mod candidate_watery_grave;

fn main() -> ExitCode {
    match run_authority() {
        Ok(report) => {
            if let Some(root) = output_root()
                && let Err(error) = write_outputs(&root, &report)
            {
                eprintln!("could not write Watery Grave PvP result: {error}");
                return ExitCode::FAILURE;
            }
            println!(
                "{}",
                serde_json::to_string(&report).expect("report serializes")
            );
            if report["passed"].as_bool() == Some(true) {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            }
        }
        Err(error) => {
            eprintln!("Watery Grave PvP authority failed: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run_authority() -> Result<Value, String> {
    validate_candidate_source()?;
    validate_candidate_semantics()?;
    let catalog = unique_catalog()?;
    let (seed, reference) = find_reference()?;
    let trace = validate_trace(&reference, seed)?;
    let replay = run_match(seed, Mutation::None)?;
    if reference != replay {
        return Err("Watery Grave full-game replay differs".to_owned());
    }
    let mutations = json!({
        "entry_life_amount_mutation_detected": mutation_detected(seed, Mutation::WrongLifeAmount)?,
        "forced_tapped_entry_mutation_detected": mutation_detected(seed, Mutation::ForcedTapped)?,
        "mana_color_choice_mutation_detected": mutation_detected(seed, Mutation::WrongManaColors)?,
        "entry_choice_removed_mutation_detected": mutation_detected(seed, Mutation::NoEntryChoice)?,
        "mana_tap_cost_mutation_detected": mutation_detected(seed, Mutation::NoTapCost)?,
        "sealed_mutation_authority": false
    });
    for key in [
        "entry_life_amount_mutation_detected",
        "forced_tapped_entry_mutation_detected",
        "mana_color_choice_mutation_detected",
        "entry_choice_removed_mutation_detected",
        "mana_tap_cost_mutation_detected",
    ] {
        if mutations[key] != true {
            return Err(format!("Watery Grave semantic mutation escaped: {key}"));
        }
    }
    let breakdown: Vec<Value> = catalog.iter().map(card_status).collect();
    let cards_passed = breakdown
        .iter()
        .filter(|card| card["trace_status"] == "passed")
        .count();
    Ok(json!({
        "schema_version":"cardbench.magic.card-pvp.check.v1", "task_id":TASK_ID,
        "candidate":{"kind":candidate_kind(),"card_definition":CARD_ID}, "candidate_kind":candidate_kind(),
        "card":{"collector_number":286,"name":CARD_NAME,"definition_id":CARD_ID},
        "passed":true,"pilot_card_passed":true,"cards_passed":cards_passed,"cards_credited":0,
        "cards_total":catalog.len(),"printings_total":RAV_MAIN_SET_EXPECTED_PRINTING_COUNT,
        "coverage_complete":false,"full_reward":false,"harbor_reward":0.0,"pilot_reward":0.0,
        "viewer_scope":format!("{:?}", ViewerScope::Judge),"qualification_status":"reference_pilot_pass; independent review and candidate trial remain",
        "replay_deterministic":true,"mutation_sensitivity":mutations,"traces":[trace],"per_card_breakdown":breakdown,
        "rules_authority":{"comprehensive_rules":"2026-08-07","authority_cutoff":"2026-09-24",
            "source_contract":"RAV #286 enters untapped if its controller pays 2 life as it enters; otherwise it enters tapped, and its tapped mana ability adds blue or black",
            "checked_sections":["106.1 and 106.4: mana production and payment","118.4: paying life","305.1 and 305.4-305.6: lands and intrinsic mana abilities","605.1: mana abilities","CR 614.1c: replacement effects","CR 614.12: replacement effects modifying how permanents enter"]}
    }))
}

fn unique_catalog() -> Result<Vec<CatalogCard>, String> {
    let printings = rav_main_set_catalog();
    let mut names = BTreeSet::new();
    let unique: Vec<_> = printings
        .iter()
        .copied()
        .filter(|card| names.insert(card.name))
        .collect();
    if unique.len() != RAV_MAIN_SET_EXPECTED_UNIQUE_NAME_COUNT
        || printings.len() != RAV_MAIN_SET_EXPECTED_PRINTING_COUNT
        || !unique
            .iter()
            .any(|card| card.collector_number == 286 && card.name == CARD_NAME)
    {
        return Err("RAV catalog identity or denominator differs for Watery Grave".to_owned());
    }
    Ok(unique)
}

fn validate_candidate_semantics() -> Result<(), String> {
    let definition = candidate_watery_grave::definition();
    let registered = card_definitions()
        .into_iter()
        .find(|card| card.id == CARD_ID)
        .ok_or("Watery Grave definition missing from registry")?;
    let entry = candidate_watery_grave::land_entry_binding();
    let registered_entry = rav_land_entry_bindings()
        .into_iter()
        .find(|binding| binding.card_definition == CARD_ID)
        .ok_or("Watery Grave entry binding missing from registry")?;
    let mana = candidate_watery_grave::mana_ability();
    let registered_mana = rav_mana_ability_bindings()
        .into_iter()
        .find(|binding| binding.card_definition == CARD_ID)
        .ok_or("Watery Grave mana binding missing from registry")?;
    let mana_colors_match = matches!(&mana.ability.output, ManaAbilityOutput::Choice(colors)
        if colors == &BTreeSet::from([Color::Blue, Color::Black]));
    if definition != registered
        || definition.id != CARD_ID
        || definition.name != CARD_NAME
        || definition.set_code != "RAV"
        || definition.mana_cost != ManaCost::new(0)
        || !definition.colors.is_empty()
        || definition.mana_colors != BTreeSet::from([Color::Blue, Color::Black])
        || definition.card_types != BTreeSet::from([CardType::Land])
        || definition.is_basic_land
        || definition.power.is_some()
        || definition.toughness.is_some()
        || !definition.keywords.is_empty()
        || !definition.effects.is_empty()
        || definition.supported_rules
            != [
                "full-rules-fidelity",
                "optional-two-life-untapped-entry",
                "blue-or-black-mana-ability",
            ]
    {
        return Err(
            "Watery Grave printed identity or characteristics differ from RAV #286".to_owned(),
        );
    }
    if entry != registered_entry || entry.enters_tapped || entry.optional_life_payment != Some(2) {
        return Err(
            "Watery Grave must enter untapped if its controller pays exactly two life, otherwise tapped".to_owned(),
        );
    }
    let sprite = card_definitions()
        .into_iter()
        .find(|card| card.id == SPRITE_ID)
        .ok_or("Surveilling Sprite definition missing from registry")?;
    let wight = card_definitions()
        .into_iter()
        .find(|card| card.id == WIGHT_ID)
        .ok_or("Roofstalker Wight definition missing from registry")?;
    if sprite.name != "Surveilling Sprite"
        || sprite.mana_cost != ManaCost::with_colors(1, [Color::Blue])
        || sprite.card_types != BTreeSet::from([CardType::Creature])
        || sprite.power != Some(1)
        || sprite.toughness != Some(1)
        || !sprite.keywords.contains(&Keyword::Flying)
        || wight.name != "Roofstalker Wight"
        || wight.mana_cost != ManaCost::with_colors(1, [Color::Black])
        || wight.card_types != BTreeSet::from([CardType::Creature])
        || wight.power != Some(2)
        || wight.toughness != Some(1)
    {
        return Err(
            "Watery Grave trace support creatures differ from their printed costs or stats"
                .to_owned(),
        );
    }
    if mana != registered_mana
        || mana.ability.id != MANA_ABILITY_ID
        || !mana.ability.tap_cost
        || !mana_colors_match
        || mana.ability.amount != 1
        || mana.ability.life_payment.is_some()
        || mana.ability.controller_damage.is_some()
    {
        return Err("Watery Grave must tap to add exactly one blue or black mana".to_owned());
    }
    Ok(())
}

fn validate_candidate_source() -> Result<(), String> {
    let source = std::fs::read_to_string(SOURCE_PATH)
        .map_err(|error| format!("could not read Watery Grave source: {error}"))?;
    let file =
        syn::parse_file(&source).map_err(|_| "Watery Grave source is invalid Rust".to_owned())?;
    if file.items.len() != 5
        || !file
            .attrs
            .iter()
            .all(|attribute| attribute.path().is_ident("doc"))
    {
        return Err(
            "Watery Grave module must contain only the three constructors and their imports"
                .to_owned(),
        );
    }
    let mut functions = BTreeSet::new();
    for item in &file.items {
        match item {
            syn::Item::Use(use_item) if use_item.attrs.is_empty() => {}
            syn::Item::Fn(function) => {
                let name = function.sig.ident.to_string();
                if !functions.insert(name.clone())
                    || function.block.stmts.len() != 1
                    || !matches!(&function.vis, syn::Visibility::Restricted(vis) if vis.path.is_ident("crate"))
                    || !function
                        .attrs
                        .iter()
                        .all(|attr| attr.path().is_ident("must_use") || attr.path().is_ident("doc"))
                {
                    return Err(format!(
                        "Watery Grave {name} is not a crate-visible constructor"
                    ));
                }
                let syn::Stmt::Expr(syn::Expr::Struct(literal), None) = &function.block.stmts[0]
                else {
                    return Err(format!(
                        "Watery Grave {name} must return a direct struct literal"
                    ));
                };
                let expected = match name.as_str() {
                    "definition" => "CardDefinition",
                    "mana_ability" => "ManaAbilityBinding",
                    "land_entry_binding" => "LandEntryBinding",
                    _ => return Err(format!("unexpected Watery Grave constructor: {name}")),
                };
                if !literal.path.is_ident(expected) || literal.rest.is_some() {
                    return Err(format!(
                        "Watery Grave {name} has an unsupported literal shape"
                    ));
                }
            }
            _ => return Err("Watery Grave module contains executable top-level code".to_owned()),
        }
    }
    if functions
        != BTreeSet::from([
            "definition".to_owned(),
            "mana_ability".to_owned(),
            "land_entry_binding".to_owned(),
        ])
    {
        return Err("Watery Grave constructor API differs from its task contract".to_owned());
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Mutation {
    None,
    WrongLifeAmount,
    ForcedTapped,
    WrongManaColors,
    NoEntryChoice,
    NoTapCost,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct LandEntryReceipt {
    card: ObjectId,
    paid_life: Option<u8>,
    tapped_on_entry: bool,
    controller_life_after_entry: i64,
}
#[derive(Clone, Debug, Eq, PartialEq)]
struct ManaReceipt {
    source: ObjectId,
    color: Color,
    tapped: bool,
    event_index: usize,
}
#[derive(Clone, Debug, Eq, PartialEq)]
struct Captured {
    events: Vec<GameEvent>,
    winner: Option<usize>,
    turns: u32,
    accepted_moves: u32,
    rejected_moves: u32,
    land_entries: Vec<LandEntryReceipt>,
    mana_receipts: Vec<ManaReceipt>,
    sprite: Option<ObjectId>,
    wight: Option<ObjectId>,
    life_totals: [i64; 2],
}

fn find_reference() -> Result<(u64, Captured), String> {
    if let Ok(value) = std::env::var("CARDBENCH_WATERY_GRAVE_SEED") {
        let seed = value
            .parse::<u64>()
            .map_err(|_| "CARDBENCH_WATERY_GRAVE_SEED must be an unsigned integer".to_owned())?;
        let capture = run_match(seed, Mutation::None)?;
        validate_trace(&capture, seed)?;
        return Ok((seed, capture));
    }
    for seed in 0..=MAX_SEED {
        if let Ok(capture) = run_match(seed, Mutation::None)
            && validate_trace(&capture, seed).is_ok()
        {
            return Ok((seed, capture));
        }
    }
    Err(format!(
        "no complete Watery Grave pay/decline and mana match found for seeds 0..={MAX_SEED}"
    ))
}

fn mutation_detected(seed: u64, mutation: Mutation) -> Result<bool, String> {
    Ok(match run_match(seed, mutation) {
        Err(_) => true,
        Ok(capture) => validate_trace(&capture, seed).is_err(),
    })
}

fn bindings_with_mutation(mutation: Mutation) -> (Vec<ManaAbilityBinding>, Vec<LandEntryBinding>) {
    let mut mana = rav_mana_ability_bindings();
    if let Some(binding) = mana
        .iter_mut()
        .find(|binding| binding.card_definition == CARD_ID)
    {
        binding.ability = candidate_watery_grave::mana_ability().ability;
        if mutation == Mutation::WrongManaColors {
            binding.ability.output =
                ManaAbilityOutput::Choice(BTreeSet::from([Color::Blue, Color::Red]));
        }
    }
    let mut entries: Vec<_> = rav_land_entry_bindings()
        .into_iter()
        .filter(|binding| binding.card_definition != CARD_ID)
        .collect();
    if mutation != Mutation::NoEntryChoice {
        let mut binding = candidate_watery_grave::land_entry_binding();
        match mutation {
            Mutation::WrongLifeAmount => binding.optional_life_payment = Some(1),
            Mutation::ForcedTapped => binding.enters_tapped = true,
            _ => {}
        }
        entries.push(binding);
    }
    if mutation == Mutation::NoTapCost {
        if let Some(binding) = mana
            .iter_mut()
            .find(|binding| binding.card_definition == CARD_ID)
        {
            binding.ability.tap_cost = false;
        }
    }
    (mana, entries)
}

fn run_match(seed: u64, mutation: Mutation) -> Result<Captured, String> {
    let mut definitions = card_definitions();
    *definitions
        .iter_mut()
        .find(|card| card.id == CARD_ID)
        .ok_or("Watery Grave missing from registry")? = candidate_watery_grave::definition();
    let (mana, entries) = bindings_with_mutation(mutation);
    let mut triggers = rav_triggered_ability_bindings();
    triggers.extend(attachment_triggers());
    let mut game = Game::new_with_all_bindings_triggers_static_continuous_effects_and_land_entries(
        definitions,
        2,
        mana,
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        rav_activated_ability_bindings(),
        triggers,
        rav_static_continuous_effect_bindings(),
        entries,
    )
    .map_err(|error| error.to_string())?;
    game.register_static_library_top_reveal_bindings(rav_static_library_top_reveal_bindings())
        .map_err(|error| error.to_string())?;
    game.register_static_attack_restrictions(rav_static_attack_restriction_bindings())
        .map_err(|error| error.to_string())?;
    game.register_static_creature_spell_cost_modifiers(
        rav_static_creature_spell_cost_modifier_bindings(),
    )
    .map_err(|error| error.to_string())?;
    game.register_attachment_bindings(rav_attachment_bindings())
        .map_err(|error| error.to_string())?;
    game.register_entry_copy_bindings(rav_entry_copy_bindings())
        .map_err(|error| error.to_string())?;
    game.register_entry_coin_flip_bindings(rav_entry_coin_flip_bindings())
        .map_err(|error| error.to_string())?;
    game.register_legendary_permanent_bindings(rav_legendary_permanent_bindings())
        .map_err(|error| error.to_string())?;
    game.register_static_entry_restriction_bindings(rav_static_entry_restriction_bindings())
        .map_err(|error| error.to_string())?;
    game.register_mana_ability_cost_bindings(rav_mana_ability_cost_bindings())
        .map_err(|error| error.to_string())?;
    game.register_cost_reduction_bindings(rav_cost_reduction_bindings())
        .map_err(|error| error.to_string())?;
    game.register_activated_ability_cost_modifier_bindings(
        rav_activated_ability_cost_modifier_bindings(),
    )
    .map_err(|error| error.to_string())?;
    game.register_generalized_activated_ability_cost_bindings(
        rav_generalized_activated_ability_cost_bindings(),
    )
    .map_err(|error| error.to_string())?;
    game.register_damage_replacement_effect_bindings(rav_damage_replacement_effect_bindings())
        .map_err(|error| error.to_string())?;
    game.register_replacement_effect_bindings(rav_replacement_effect_bindings())
        .map_err(|error| error.to_string())?;

    let pilot = DeckList {
        mainboard: vec![
            DeckEntry {
                card: CARD_ID.to_owned(),
                count: 4,
            },
            DeckEntry {
                card: SPRITE_ID.to_owned(),
                count: 4,
            },
            DeckEntry {
                card: WIGHT_ID.to_owned(),
                count: 4,
            },
            DeckEntry {
                card: "RAV-ISLAND".to_owned(),
                count: 24,
            },
            DeckEntry {
                card: "RAV-SWAMP".to_owned(),
                count: 24,
            },
        ],
        sideboard: vec![],
    };
    let opponent = DeckList {
        mainboard: vec![DeckEntry {
            card: "RAV-SWAMP".to_owned(),
            count: 60,
        }],
        sideboard: vec![],
    };
    for (label, deck) in [("Watery Grave pilot", &pilot), ("pass opponent", &opponent)] {
        let count: usize = deck
            .mainboard
            .iter()
            .map(|entry| usize::from(entry.count))
            .sum();
        if count != 60 {
            return Err(format!("{label} deck has {count} cards, expected 60"));
        }
    }
    game.set_shuffle_seed(seed)
        .map_err(|error| error.to_string())?;
    game.load_deck_into_library(PlayerId(0), &pilot)
        .map_err(|error| error.to_string())?;
    game.load_deck_into_library(PlayerId(1), &opponent)
        .map_err(|error| error.to_string())?;
    game.draw_opening_hand(PlayerId(0), 7)
        .map_err(|error| error.to_string())?;
    game.draw_opening_hand(PlayerId(1), 7)
        .map_err(|error| error.to_string())?;
    game.begin_game().map_err(|error| error.to_string())?;
    let mut policies: [Box<dyn CodePolicy>; 2] = [
        Box::new(WateryGravePolicy::new(PlayerId(0))),
        Box::new(PassPolicy {
            player: PlayerId(1),
        }),
    ];
    let mut capture = Captured {
        events: vec![],
        winner: None,
        turns: 0,
        accepted_moves: 0,
        rejected_moves: 0,
        land_entries: vec![],
        mana_receipts: vec![],
        sprite: None,
        wight: None,
        life_totals: [0, 0],
    };
    while !game.is_game_over() {
        if game.turn > 60 || capture.accepted_moves > 8_000 {
            return Err(format!(
                "Watery Grave duel exceeded bounds at turn {}",
                game.turn
            ));
        }
        let player = game.next_policy_player();
        let view = game
            .view_for_player(player)
            .map_err(|error| error.to_string())?;
        let policy = policies
            .get_mut(player.0)
            .ok_or("Watery Grave policy seat missing")?;
        let action = if let Some(action) = policy.propose_pending_decision(&view) {
            action
        } else if view.optional_triggered_ability_choice.is_some() {
            policy.propose_optional_triggered_ability(&view)
        } else if view.draw_replacement_pending {
            policy.propose_draw_replacement(&view)
        } else if view.private_library_choice.is_some() {
            policy.propose_private_library_choice(&view)
        } else if view.private_opponent_library_choice.is_some() {
            policy.propose_private_opponent_library_choice(&view)
        } else if view.library_search_choice.is_some() {
            policy.propose_library_search_choice(&view)
        } else {
            policy.propose_move(&view)
        };
        let land_choice = match &action {
            PolicyAction::PlayLandWithEntryLifePayment { card, pay_life }
                if game
                    .card_definition(*card)
                    .is_ok_and(|definition| definition.id == CARD_ID) =>
            {
                Some((*card, *pay_life))
            }
            _ => None,
        };
        let source_mana = match &action {
            PolicyAction::ActivateBoundManaAbility { activation }
                if activation.ability_id == MANA_ABILITY_ID =>
            {
                Some(activation.source)
            }
            _ => None,
        };
        let cursor = game.event_log.len();
        let action_label = format!("{action:?}");
        game.submit_policy_move(player, policy.id(), action)
            .map_err(|error| {
                format!(
                    "Watery Grave move rejected at turn {} {:?} with {action_label}: {error}",
                    view.turn, view.step
                )
            })?;
        capture.accepted_moves += 1;
        game.validate_invariants()
            .map_err(|error| format!("Watery Grave post-move invariant failed: {error}"))?;
        if let Some((card, pay_life)) = land_choice {
            let paid = game.event_log[cursor..]
                .iter()
                .find_map(|event| match event {
                    GameEvent::LandEntryLifePaid {
                        card: paid_card,
                        amount,
                        ..
                    } if *paid_card == card => Some(*amount),
                    _ => None,
                });
            let tapped = game.object(card).map_err(|error| error.to_string())?.tapped;
            capture.land_entries.push(LandEntryReceipt {
                card,
                paid_life: if pay_life { paid } else { None },
                tapped_on_entry: tapped,
                controller_life_after_entry: game
                    .player(PlayerId(0))
                    .map_err(|error| error.to_string())?
                    .life,
            });
        }
        if let Some(source) = source_mana {
            let tapped = game
                .object(source)
                .map_err(|error| error.to_string())?
                .tapped;
            for (offset, event) in game.event_log[cursor..].iter().enumerate() {
                if let GameEvent::BoundManaAbilityActivated {
                    player: PlayerId(0),
                    source: event_source,
                    ability,
                    color,
                    amount: 1,
                    tapped: was_tapped,
                    ..
                } = event
                    && *event_source == source
                    && *ability == MANA_ABILITY_ID
                {
                    capture.mana_receipts.push(ManaReceipt {
                        source,
                        color: *color,
                        tapped: *was_tapped && tapped,
                        event_index: cursor + offset,
                    });
                }
            }
        }
        for event in &game.event_log[cursor..] {
            if let GameEvent::SpellCast {
                player: PlayerId(0),
                card,
            } = event
            {
                let id = game
                    .card_definition(*card)
                    .map_err(|error| error.to_string())?
                    .id;
                if id == SPRITE_ID {
                    capture.sprite.get_or_insert(*card);
                }
                if id == WIGHT_ID {
                    capture.wight.get_or_insert(*card);
                }
            }
        }
    }
    capture.events = game.event_log.clone();
    capture.winner = game.winner().map(|winner| winner.0);
    capture.turns = game.turn;
    capture.life_totals = [
        game.player(PlayerId(0))
            .map_err(|error| error.to_string())?
            .life,
        game.player(PlayerId(1))
            .map_err(|error| error.to_string())?
            .life,
    ];
    Ok(capture)
}

fn validate_trace(capture: &Captured, seed: u64) -> Result<Value, String> {
    if capture.winner != Some(0) {
        return Err("Watery Grave duel did not reach a clean seat-zero terminal win".to_owned());
    }
    if capture.rejected_moves != 0 {
        return Err("Watery Grave trace has rejected moves".to_owned());
    }
    if capture.land_entries.len() != 2 {
        return Err(
            "trace must make exactly one paid and one declined Watery Grave land play".to_owned(),
        );
    }
    let paid = &capture.land_entries[0];
    let declined = &capture.land_entries[1];
    if paid.card == declined.card
        || paid.paid_life != Some(2)
        || paid.tapped_on_entry
        || declined.paid_life.is_some()
        || !declined.tapped_on_entry
    {
        return Err("Watery Grave must show first pay-two-life/untapped and second decline/enters-tapped branches".to_owned());
    }
    if paid.controller_life_after_entry != 18 {
        return Err(format!(
            "paying two life must immediately set the pilot life total to 18, got {}",
            paid.controller_life_after_entry
        ));
    }
    let sprite = capture.sprite.ok_or("Surveilling Sprite was not cast")?;
    let wight = capture.wight.ok_or("Roofstalker Wight was not cast")?;
    let bound_receipts: Vec<_> = capture
        .mana_receipts
        .iter()
        .filter(|receipt| receipt.tapped)
        .collect();
    let blue = bound_receipts
        .iter()
        .find(|receipt| receipt.color == Color::Blue)
        .ok_or("Watery Grave did not produce blue mana")?;
    let black = bound_receipts
        .iter()
        .find(|receipt| receipt.color == Color::Black)
        .ok_or("Watery Grave did not produce black mana")?;
    if blue.source != paid.card || black.source != paid.card {
        return Err(
            "the paid, untapped Watery Grave did not produce both blue and black mana".to_owned(),
        );
    }
    let blue_mana_event = blue.event_index;
    let black_mana_event = black.event_index;
    let life_receipt = capture.events.iter().position(|event| matches!(event, GameEvent::LandEntryLifePaid { player: PlayerId(0), card, amount: 2 } if *card == paid.card)).ok_or("two-life land-entry receipt missing")?;
    let paid_entry = event_index_after(&capture.events, life_receipt, |event| matches!(event, GameEvent::CardMoved { card, to: Zone::Battlefield } if *card == paid.card)).ok_or("paid Watery Grave battlefield entry missing")?;
    let sprite_payment = event_index(&capture.events, |event| matches!(event, GameEvent::SpellManaPaid { player: PlayerId(0), card, colors } if *card == sprite && colors.len() == 2 && colors.contains(&Color::Blue) && colors.contains(&Color::Black))).ok_or("Surveilling Sprite did not spend Watery Grave blue plus black generic mana")?;
    let sprite_cast = event_index_after(
        &capture.events,
        sprite_payment,
        |event| matches!(event, GameEvent::SpellCast { player: PlayerId(0), card } if *card == sprite),
    )
    .ok_or("Surveilling Sprite cast receipt missing")?;
    let sprite_resolved = event_index_after(
        &capture.events,
        sprite_cast,
        |event| matches!(event, GameEvent::SpellResolved { card } if *card == sprite),
    )
    .ok_or("Surveilling Sprite resolution missing")?;
    let wight_payment = event_index(&capture.events, |event| matches!(event, GameEvent::SpellManaPaid { player: PlayerId(0), card, colors } if *card == wight && colors.len() == 2 && colors.contains(&Color::Black) && colors.contains(&Color::Blue))).ok_or("Roofstalker Wight did not spend Watery Grave black plus blue generic mana")?;
    let wight_cast = event_index_after(&capture.events, wight_payment, |event| matches!(event, GameEvent::SpellCast { player: PlayerId(0), card } if *card == wight)).ok_or("Roofstalker Wight cast receipt missing")?;
    let wight_resolved = event_index_after(
        &capture.events,
        wight_cast,
        |event| matches!(event, GameEvent::SpellResolved { card } if *card == wight),
    )
    .ok_or("Roofstalker Wight resolution missing")?;
    let attack = event_index(&capture.events, |event| matches!(event, GameEvent::AttackersDeclared { player: PlayerId(0), attackers } if attackers.contains(&wight))).ok_or("Roofstalker Wight attack missing")?;
    let damage = event_index_after(&capture.events, attack, |event| matches!(event, GameEvent::DamageDealtToPlayer { source, player: PlayerId(1), amount: 2 } if *source == wight)).ok_or("Roofstalker Wight did not deal its expected 2 damage")?;
    let terminal = event_index(&capture.events, |event| {
        matches!(
            event,
            GameEvent::GameEnded {
                winner: Some(PlayerId(0))
            }
        )
    })
    .ok_or("terminal winner event missing")?;
    if !(paid_entry < blue_mana_event
        && blue_mana_event < sprite_payment
        && sprite_payment < sprite_cast
        && sprite_cast < sprite_resolved
        && black_mana_event < wight_payment
        && wight_payment < wight_cast
        && wight_cast < wight_resolved
        && wight_resolved < attack
        && attack < damage
        && damage < terminal)
    {
        return Err(
            "Watery Grave entry, colored mana, cast, combat, and terminal event order differs"
                .to_owned(),
        );
    }
    let projected = project_events(&capture.events);
    let events = project_relevant(&projected, [paid.card, declined.card, sprite, wight]);
    Ok(json!({
        "id":"rav-watery-grave-life-choice-and-mana","seed":seed,
        "decks":["rav_watery_grave_choice","rav_watery_grave_pass_opponent"],
        "policies":["rav.watery-grave-choice-pilot.v1","rav.watery-grave-pass-opponent.v1"],
        "terminal":"Winner(PlayerId(0))","winner_seat":0,"turns":capture.turns,
        "accepted_moves":capture.accepted_moves,"rejected_moves":capture.rejected_moves,
        "land_entry_choices":[
            {"card_object":paid.card.0,"choice":"pay_two_life","life_paid":2,"entered_tapped":false,"controller_life_after_entry":paid.controller_life_after_entry},
            {"card_object":declined.card.0,"choice":"decline","life_paid":0,"entered_tapped":true,"controller_life_after_entry":declined.controller_life_after_entry}
        ],
        "pilot_life_after_entry_payment":paid.controller_life_after_entry,
        "life_totals":capture.life_totals,
        "watery_grave_mana_receipts":bound_receipts.iter().map(|receipt| json!({"source":receipt.source.0,"color":format!("{:?}",receipt.color),"amount":1,"tapped":receipt.tapped,"event_index":receipt.event_index})).collect::<Vec<_>>(),
        "colored_mana_use_order":{"paid_grave_blue_event_index":blue.event_index,"sprite_payment_event_index":sprite_payment,"paid_grave_black_event_index":black.event_index,"wight_payment_event_index":wight_payment},
        "blue_and_black_mana_used":blue.color == Color::Blue && black.color == Color::Black,
        "surveilling_sprite":{"name":"Surveilling Sprite","object":sprite.0,"mana_cost":"{1}{U}","payment":["Black","Blue"],"power":1,"toughness":1,"flying":true,"cast_and_resolved":true},
        "roofstalker_wight":{"name":"Roofstalker Wight","object":wight.0,"mana_cost":"{1}{B}","payment":["Black","Blue"],"base_power":2,"base_toughness":1,"attack_damage":2},
        "paid_entry_sequence":life_receipt,"terminal_sequence":terminal,
        "events":events,"passed":true
    }))
}

fn project_relevant(events: &[ProjectedEvent], objects: [ObjectId; 4]) -> Vec<ProjectedEvent> {
    events
        .iter()
        .filter(|event| {
            event.kind == "GameEnded"
                || event
                    .facts
                    .objects
                    .iter()
                    .any(|object| objects.iter().any(|target| target.0 == object.0))
        })
        .cloned()
        .collect()
}

fn event_index(events: &[GameEvent], predicate: impl Fn(&GameEvent) -> bool) -> Option<usize> {
    events.iter().position(predicate)
}
fn event_index_after(
    events: &[GameEvent],
    after: usize,
    predicate: impl Fn(&GameEvent) -> bool,
) -> Option<usize> {
    events
        .iter()
        .enumerate()
        .skip(after + 1)
        .find_map(|(index, event)| predicate(event).then_some(index))
}

struct WateryGravePolicy {
    player: PlayerId,
    grave_choices: u8,
    sprite_cast: bool,
    wight_cast: bool,
    grave_blue_produced: bool,
    grave_black_produced: bool,
}
impl WateryGravePolicy {
    fn new(player: PlayerId) -> Self {
        Self {
            player,
            grave_choices: 0,
            sprite_cast: false,
            wight_cast: false,
            grave_blue_produced: false,
            grave_black_produced: false,
        }
    }
    fn play_land(&mut self, view: &GameView) -> Option<PolicyAction> {
        if view.lands_played != 0 {
            return None;
        }
        if self.grave_choices < 2
            && let Some(land) = view
                .hand
                .iter()
                .find(|card| card.definition == Some(CARD_ID))
        {
            let pay_life = self.grave_choices == 0;
            self.grave_choices += 1;
            return Some(PolicyAction::PlayLandWithEntryLifePayment {
                card: land.id,
                pay_life,
            });
        }
        view.hand
            .iter()
            .find(|card| {
                card.definition == Some("RAV-ISLAND") || card.definition == Some("RAV-SWAMP")
            })
            .map(|land| PolicyAction::PlayLand { card: land.id })
    }
    fn cast(
        &mut self,
        view: &GameView,
        definition: &'static str,
        cost: ManaCost,
    ) -> Option<PolicyAction> {
        let card = view
            .hand
            .iter()
            .find(|card| card.definition == Some(definition))?;
        let required = if definition == SPRITE_ID {
            Color::Blue
        } else {
            Color::Black
        };
        let already_sourced = if required == Color::Blue {
            self.grave_blue_produced
        } else {
            self.grave_black_produced
        };
        if !already_sourced {
            if let Some(grave) = view
                .own_battlefield
                .iter()
                .find(|land| land.definition == Some(CARD_ID) && !land.tapped)
            {
                if required == Color::Blue {
                    self.grave_blue_produced = true;
                } else {
                    self.grave_black_produced = true;
                }
                return Some(PolicyAction::ActivateBoundManaAbility {
                    activation: ManaAbilityActivation {
                        source: grave.id,
                        ability_id: MANA_ABILITY_ID,
                        chosen_color: Some(required),
                    },
                });
            }
            return None;
        }
        if let Some(mana_selection) = payment(view, &cost) {
            if definition == SPRITE_ID {
                self.sprite_cast = true;
            } else {
                self.wight_cast = true;
            }
            return Some(PolicyAction::CastWithPayment {
                request: cardbench_magic_engine::CastRequest {
                    card: card.id,
                    targets: vec![],
                    convoke: vec![],
                    payment_mana_abilities: vec![],
                },
                chosen_x: None,
                mana_selection,
            });
        }
        let needed = cost
            .colored
            .iter()
            .any(|color| view.mana_pool.amount(*color) == 0);
        if needed {
            if let Some(grave) = view
                .own_battlefield
                .iter()
                .find(|land| land.definition == Some(CARD_ID) && !land.tapped)
            {
                if required == Color::Blue {
                    self.grave_blue_produced = true;
                } else {
                    self.grave_black_produced = true;
                }
                return Some(PolicyAction::ActivateBoundManaAbility {
                    activation: ManaAbilityActivation {
                        source: grave.id,
                        ability_id: MANA_ABILITY_ID,
                        chosen_color: Some(required),
                    },
                });
            }
            return None;
        }
        let generic_color = if definition == SPRITE_ID {
            Color::Black
        } else {
            Color::Blue
        };
        let basic_id = if generic_color == Color::Black {
            "RAV-SWAMP"
        } else {
            "RAV-ISLAND"
        };
        view.own_battlefield
            .iter()
            .find(|land| land.definition == Some(basic_id) && !land.tapped)
            .map(|land| PolicyAction::ActivateManaAbility {
                land: land.id,
                color: generic_color,
            })
    }
}
impl CodePolicy for WateryGravePolicy {
    fn id(&self) -> &'static str {
        "rav.watery-grave-choice-pilot.v1"
    }
    fn propose_move(&mut self, view: &GameView) -> PolicyAction {
        if view.step == Step::DeclareAttackers
            && view.active_player == self.player
            && !view.attackers_declared
        {
            return PolicyAction::DeclareAttackers {
                attackers: view
                    .own_battlefield
                    .iter()
                    .filter(|card| {
                        card.can_attack
                            && (card.definition == Some(SPRITE_ID)
                                || card.definition == Some(WIGHT_ID))
                    })
                    .map(|card| card.id)
                    .collect(),
            };
        }
        if view.step == Step::DeclareBlockers
            && view.active_player != self.player
            && !view.blockers_declared
        {
            return PolicyAction::DeclareBlockers {
                assignments: vec![],
            };
        }
        if view.player != self.player || view.step != Step::PrecombatMain || view.stack_depth != 0 {
            return PolicyAction::PassPriority;
        }
        if let Some(action) = self.play_land(view) {
            return action;
        }
        if !self.sprite_cast
            && let Some(action) =
                self.cast(view, SPRITE_ID, ManaCost::with_colors(1, [Color::Blue]))
        {
            return action;
        }
        if self.sprite_cast
            && !self.wight_cast
            && let Some(action) =
                self.cast(view, WIGHT_ID, ManaCost::with_colors(1, [Color::Black]))
        {
            return action;
        }
        PolicyAction::PassPriority
    }
}

struct PassPolicy {
    player: PlayerId,
}
impl CodePolicy for PassPolicy {
    fn id(&self) -> &'static str {
        "rav.watery-grave-pass-opponent.v1"
    }
    fn propose_move(&mut self, view: &GameView) -> PolicyAction {
        if view.step == Step::DeclareAttackers
            && view.active_player == self.player
            && !view.attackers_declared
        {
            return PolicyAction::DeclareAttackers { attackers: vec![] };
        }
        if view.step == Step::DeclareBlockers
            && view.active_player != self.player
            && !view.blockers_declared
        {
            return PolicyAction::DeclareBlockers {
                assignments: vec![],
            };
        }
        PolicyAction::PassPriority
    }
}

fn payment(view: &GameView, cost: &ManaCost) -> Option<ManaPaymentSelection> {
    let mut available: Vec<(Color, u8)> = Color::MANA_ALL
        .into_iter()
        .map(|color| (color, view.mana_pool.amount(color)))
        .collect();
    for color in &cost.colored {
        let (_, amount) = available
            .iter_mut()
            .find(|(available_color, _)| available_color == color)?;
        if *amount == 0 {
            return None;
        }
        *amount -= 1;
    }
    let mut generic = Vec::with_capacity(usize::from(cost.generic));
    for _ in 0..cost.generic {
        let (color, amount) = available.iter_mut().max_by_key(|(_, amount)| *amount)?;
        if *amount == 0 {
            return None;
        }
        *amount -= 1;
        generic.push(*color);
    }
    Some(ManaPaymentSelection {
        generic,
        hybrid: vec![],
    })
}

fn card_status(card: &CatalogCard) -> Value {
    let definition_id = match card.semantic_status {
        CardSemanticStatus::ExecutableCompatibilitySlice { definition_id } => Some(definition_id),
        CardSemanticStatus::CatalogOnly { .. } => None,
    };
    let passed = definition_id == Some(CARD_ID);
    json!({"collector_number":card.collector_number,"name":card.name,"definition_id":definition_id,
        "engine_status":match card.semantic_status { CardSemanticStatus::ExecutableCompatibilitySlice { .. } => "executable_compatibility_slice", CardSemanticStatus::CatalogOnly { .. } => "catalog_only" },
        "trace_status":if passed {"passed"} else {"not_qualified"},"credited":false,
        "qualification":if passed {"reference_pilot_pass; independent reviewer approval pending"} else {"not_evaluated"}})
}

fn candidate_kind() -> &'static str {
    if std::env::args().any(|argument| argument == "--candidate") {
        "agent"
    } else {
        "reference"
    }
}
fn output_root() -> Option<std::path::PathBuf> {
    let mut args = std::env::args().skip(1);
    while let Some(argument) = args.next() {
        if argument == "--output-root" {
            return args.next().map(std::path::PathBuf::from);
        }
    }
    None
}
fn write_outputs(root: &std::path::Path, report: &Value) -> std::io::Result<()> {
    std::fs::create_dir_all(root)?;
    std::fs::write(
        root.join("result.json"),
        serde_json::to_vec_pretty(report).expect("report serializes"),
    )?;
    std::fs::write(root.join("reward.txt"), "0.0\n")
}
