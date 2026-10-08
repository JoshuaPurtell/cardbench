//! Full-game diagnostic authority for RAV Sacred Foundry.

use cardbench_magic_engine::{
    CardType, Color, DeckEntry, DeckList, Game, GameEvent, GameView, HybridManaSymbol, Keyword, LandEntryBinding,
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
const CARD_ID: &str = "RAV-SACRED-FOUNDRY";
const CARD_NAME: &str = "Sacred Foundry";
const MANA_ABILITY_ID: &str = "produce-white-or-red";
const HOUND_ID: &str = "RAV-GATE-HOUND";
const RECRUIT_ID: &str = "RAV-BOROS-RECRUIT";
const SOURCE_PATH: &str = "varieties/magic/sets/ravnica_city_of_guilds/src/cards/sacred_foundry.rs";
const MAX_SEED: u64 = 128;

#[path = "../../../sets/ravnica_city_of_guilds/src/cards/sacred_foundry.rs"]
mod candidate_sacred_foundry;

fn main() -> ExitCode {
    match run_authority() {
        Ok(report) => {
            if let Some(root) = output_root()
                && let Err(error) = write_outputs(&root, &report)
            {
                eprintln!("could not write Sacred Foundry PvP result: {error}");
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
            eprintln!("Sacred Foundry PvP authority failed: {error}");
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
        return Err("Sacred Foundry full-game replay differs".to_owned());
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
            return Err(format!("Sacred Foundry semantic mutation escaped: {key}"));
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
        "card":{"collector_number":280,"name":CARD_NAME,"definition_id":CARD_ID},
        "passed":true,"pilot_card_passed":true,"cards_passed":cards_passed,"cards_credited":0,
        "cards_total":catalog.len(),"printings_total":RAV_MAIN_SET_EXPECTED_PRINTING_COUNT,
        "coverage_complete":false,"full_reward":false,"harbor_reward":0.0,"pilot_reward":0.0,
        "viewer_scope":format!("{:?}", ViewerScope::Judge),"qualification_status":"reference_pilot_pass; independent review and candidate trial remain",
        "replay_deterministic":true,"mutation_sensitivity":mutations,"traces":[trace],"per_card_breakdown":breakdown,
        "rules_authority":{"comprehensive_rules":"2026-08-07","authority_cutoff":"2026-09-24",
            "source_contract":"RAV #280 enters untapped if its controller pays 2 life as it enters; otherwise it enters tapped, and its tapped mana ability adds white or red",
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
            .any(|card| card.collector_number == 280 && card.name == CARD_NAME)
    {
        return Err("RAV catalog identity or denominator differs for Sacred Foundry".to_owned());
    }
    Ok(unique)
}

fn validate_candidate_semantics() -> Result<(), String> {
    let definition = candidate_sacred_foundry::definition();
    let registered = card_definitions()
        .into_iter()
        .find(|card| card.id == CARD_ID)
        .ok_or("Sacred Foundry definition missing from registry")?;
    let entry = candidate_sacred_foundry::land_entry_binding();
    let registered_entry = rav_land_entry_bindings()
        .into_iter()
        .find(|binding| binding.card_definition == CARD_ID)
        .ok_or("Sacred Foundry entry binding missing from registry")?;
    let mana = candidate_sacred_foundry::mana_ability();
    let registered_mana = rav_mana_ability_bindings()
        .into_iter()
        .find(|binding| binding.card_definition == CARD_ID)
        .ok_or("Sacred Foundry mana binding missing from registry")?;
    let mana_colors_match = matches!(&mana.ability.output, ManaAbilityOutput::Choice(colors)
        if colors == &BTreeSet::from([Color::White, Color::Red]));
    if definition != registered
        || definition.id != CARD_ID
        || definition.name != CARD_NAME
        || definition.set_code != "RAV"
        || definition.mana_cost != ManaCost::new(0)
        || !definition.colors.is_empty()
        || definition.mana_colors != BTreeSet::from([Color::White, Color::Red])
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
                "white-or-red-mana-ability",
            ]
    {
        return Err(
            "Sacred Foundry printed identity or characteristics differ from RAV #280".to_owned(),
        );
    }
    if entry != registered_entry || entry.enters_tapped || entry.optional_life_payment != Some(2) {
        return Err(
            "Sacred Foundry must enter untapped if its controller pays exactly two life, otherwise tapped".to_owned(),
        );
    }
    let hound = card_definitions()
        .into_iter()
        .find(|card| card.id == HOUND_ID)
        .ok_or("Gate Hound definition missing from registry")?;
    let recruit = card_definitions()
        .into_iter()
        .find(|card| card.id == RECRUIT_ID)
        .ok_or("Boros Recruit definition missing from registry")?;
    if hound.name != "Gate Hound"
        || hound.mana_cost != ManaCost::with_colors(2, [Color::White])
        || hound.card_types != BTreeSet::from([CardType::Creature])
        || hound.power != Some(1)
        || hound.toughness != Some(1)
        || hound.colors != BTreeSet::from([Color::White])
        || !hound.keywords.is_empty()
        || recruit.name != "Boros Recruit"
        || recruit.mana_cost
            != ManaCost::with_hybrid(
                0,
                [],
                [HybridManaSymbol {
                    first: Color::Red,
                    second: Color::White,
                }],
            )
        || recruit.card_types != BTreeSet::from([CardType::Creature])
        || recruit.power != Some(1)
        || recruit.toughness != Some(1)
        || !recruit.keywords.contains(&Keyword::FirstStrike)
    {
        return Err(
            "Sacred Foundry trace support creatures differ from their printed costs or stats"
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
        return Err("Sacred Foundry must tap to add exactly one white or red mana".to_owned());
    }
    Ok(())
}

fn validate_candidate_source() -> Result<(), String> {
    let source = std::fs::read_to_string(SOURCE_PATH)
        .map_err(|error| format!("could not read Sacred Foundry source: {error}"))?;
    let file =
        syn::parse_file(&source).map_err(|_| "Sacred Foundry source is invalid Rust".to_owned())?;
    if file.items.len() != 5
        || !file
            .attrs
            .iter()
            .all(|attribute| attribute.path().is_ident("doc"))
    {
        return Err(
            "Sacred Foundry module must contain only the three constructors and their imports"
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
                        "Sacred Foundry {name} is not a crate-visible constructor"
                    ));
                }
                let syn::Stmt::Expr(syn::Expr::Struct(literal), None) = &function.block.stmts[0]
                else {
                    return Err(format!(
                        "Sacred Foundry {name} must return a direct struct literal"
                    ));
                };
                let expected = match name.as_str() {
                    "definition" => "CardDefinition",
                    "mana_ability" => "ManaAbilityBinding",
                    "land_entry_binding" => "LandEntryBinding",
                    _ => return Err(format!("unexpected Sacred Foundry constructor: {name}")),
                };
                if !literal.path.is_ident(expected) || literal.rest.is_some() {
                    return Err(format!(
                        "Sacred Foundry {name} has an unsupported literal shape"
                    ));
                }
            }
            _ => return Err("Sacred Foundry module contains executable top-level code".to_owned()),
        }
    }
    if functions
        != BTreeSet::from([
            "definition".to_owned(),
            "mana_ability".to_owned(),
            "land_entry_binding".to_owned(),
        ])
    {
        return Err("Sacred Foundry constructor API differs from its task contract".to_owned());
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
    hound: Option<ObjectId>,
    recruit: Option<ObjectId>,
    life_totals: [i64; 2],
}

fn find_reference() -> Result<(u64, Captured), String> {
    if let Ok(value) = std::env::var("CARDBENCH_SACRED_FOUNDRY_SEED") {
        let seed = value
            .parse::<u64>()
            .map_err(|_| "CARDBENCH_SACRED_FOUNDRY_SEED must be an unsigned integer".to_owned())?;
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
        "no complete Sacred Foundry pay/decline and mana match found for seeds 0..={MAX_SEED}"
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
        binding.ability = candidate_sacred_foundry::mana_ability().ability;
        if mutation == Mutation::WrongManaColors {
            binding.ability.output =
                ManaAbilityOutput::Choice(BTreeSet::from([Color::White, Color::Blue]));
        }
    }
    let mut entries: Vec<_> = rav_land_entry_bindings()
        .into_iter()
        .filter(|binding| binding.card_definition != CARD_ID)
        .collect();
    if mutation != Mutation::NoEntryChoice {
        let mut binding = candidate_sacred_foundry::land_entry_binding();
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
        .ok_or("Sacred Foundry missing from registry")? = candidate_sacred_foundry::definition();
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
                card: HOUND_ID.to_owned(),
                count: 4,
            },
            DeckEntry {
                card: RECRUIT_ID.to_owned(),
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
    for (label, deck) in [("Sacred Foundry pilot", &pilot), ("pass opponent", &opponent)] {
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
        Box::new(SacredFoundryPolicy::new(PlayerId(0))),
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
        hound: None,
        recruit: None,
        life_totals: [0, 0],
    };
    while !game.is_game_over() {
        if game.turn > 60 || capture.accepted_moves > 8_000 {
            return Err(format!(
                "Sacred Foundry duel exceeded bounds at turn {}",
                game.turn
            ));
        }
        let player = game.next_policy_player();
        let view = game
            .view_for_player(player)
            .map_err(|error| error.to_string())?;
        let policy = policies
            .get_mut(player.0)
            .ok_or("Sacred Foundry policy seat missing")?;
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
                    "Sacred Foundry move rejected at turn {} {:?} with {action_label}: {error}",
                    view.turn, view.step
                )
            })?;
        capture.accepted_moves += 1;
        game.validate_invariants()
            .map_err(|error| format!("Sacred Foundry post-move invariant failed: {error}"))?;
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
                if id == HOUND_ID {
                    capture.hound.get_or_insert(*card);
                }
                if id == RECRUIT_ID {
                    capture.recruit.get_or_insert(*card);
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
        return Err("Sacred Foundry duel did not reach a clean seat-zero terminal win".to_owned());
    }
    if capture.rejected_moves != 0 {
        return Err("Sacred Foundry trace has rejected moves".to_owned());
    }
    if capture.land_entries.len() != 2 {
        return Err(
            "trace must make exactly one paid and one declined Sacred Foundry land play".to_owned(),
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
        return Err("Sacred Foundry must show first pay-two-life/untapped and second decline/enters-tapped branches".to_owned());
    }
    if paid.controller_life_after_entry != 18 {
        return Err(format!(
            "paying two life must immediately set the pilot life total to 18, got {}",
            paid.controller_life_after_entry
        ));
    }
    let hound = capture.hound.ok_or("Gate Hound was not cast")?;
    let recruit = capture.recruit.ok_or("Boros Recruit was not cast")?;
    let bound_receipts: Vec<_> = capture
        .mana_receipts
        .iter()
        .filter(|receipt| receipt.tapped)
        .collect();
    let white = bound_receipts
        .iter()
        .find(|receipt| receipt.color == Color::White)
        .ok_or("Sacred Foundry did not produce white mana")?;
    let red = bound_receipts
        .iter()
        .find(|receipt| receipt.color == Color::Red)
        .ok_or("Sacred Foundry did not produce red mana")?;
    if white.source != paid.card || red.source != paid.card {
        return Err(
            "the paid, untapped Sacred Foundry did not produce both white and red mana".to_owned(),
        );
    }
    let white_mana_event = white.event_index;
    let red_mana_event = red.event_index;
    let life_receipt = capture.events.iter().position(|event| matches!(event, GameEvent::LandEntryLifePaid { player: PlayerId(0), card, amount: 2 } if *card == paid.card)).ok_or("two-life land-entry receipt missing")?;
    let paid_entry = event_index_after(&capture.events, life_receipt, |event| matches!(event, GameEvent::CardMoved { card, to: Zone::Battlefield } if *card == paid.card)).ok_or("paid Sacred Foundry battlefield entry missing")?;
    let hound_payment = event_index(&capture.events, |event| matches!(event, GameEvent::SpellManaPaid { player: PlayerId(0), card, colors } if *card == hound && colors.len() == 3 && colors.contains(&Color::White))).ok_or("Gate Hound did not spend Sacred Foundry white plus two generic mana")?;
    let hound_cast = event_index_after(
        &capture.events,
        hound_payment,
        |event| matches!(event, GameEvent::SpellCast { player: PlayerId(0), card } if *card == hound),
    )
    .ok_or("Gate Hound cast receipt missing")?;
    let hound_resolved = event_index_after(
        &capture.events,
        hound_cast,
        |event| matches!(event, GameEvent::SpellResolved { card } if *card == hound),
    )
    .ok_or("Gate Hound resolution missing")?;
    let recruit_payment = event_index(&capture.events, |event| matches!(event, GameEvent::SpellManaPaid { player: PlayerId(0), card, colors } if *card == recruit && colors.as_slice() == [Color::Red])).ok_or("Boros Recruit did not spend Sacred Foundry red for its hybrid symbol")?;
    let recruit_cast = event_index_after(&capture.events, recruit_payment, |event| matches!(event, GameEvent::SpellCast { player: PlayerId(0), card } if *card == recruit)).ok_or("Boros Recruit cast receipt missing")?;
    let recruit_resolved = event_index_after(
        &capture.events,
        recruit_cast,
        |event| matches!(event, GameEvent::SpellResolved { card } if *card == recruit),
    )
    .ok_or("Boros Recruit resolution missing")?;
    let attack = event_index(&capture.events, |event| matches!(event, GameEvent::AttackersDeclared { player: PlayerId(0), attackers } if attackers.contains(&recruit))).ok_or("Boros Recruit attack missing")?;
    let damage = event_index_after(&capture.events, attack, |event| matches!(event, GameEvent::DamageDealtToPlayer { source, player: PlayerId(1), amount: 1 } if *source == recruit)).ok_or("Boros Recruit did not deal its expected 1 damage")?;
    let terminal = event_index(&capture.events, |event| {
        matches!(
            event,
            GameEvent::GameEnded {
                winner: Some(PlayerId(0))
            }
        )
    })
    .ok_or("terminal winner event missing")?;
    if !(paid_entry < white_mana_event
        && white_mana_event < hound_payment
        && hound_payment < hound_cast
        && hound_cast < hound_resolved
        && red_mana_event < recruit_payment
        && recruit_payment < recruit_cast
        && recruit_cast < recruit_resolved
        && recruit_resolved < attack
        && attack < damage
        && damage < terminal)
    {
        return Err(
            "Sacred Foundry entry, colored mana, cast, combat, and terminal event order differs"
                .to_owned(),
        );
    }
    let projected = project_events(&capture.events);
    let events = project_relevant(&projected, [paid.card, declined.card, hound, recruit]);
    Ok(json!({
        "id":"rav-sacred-foundry-life-choice-and-mana","seed":seed,
        "decks":["rav_sacred_foundry_choice","rav_sacred_foundry_pass_opponent"],
        "policies":["rav.sacred-foundry-choice-pilot.v1","rav.sacred-foundry-pass-opponent.v1"],
        "terminal":"Winner(PlayerId(0))","winner_seat":0,"turns":capture.turns,
        "accepted_moves":capture.accepted_moves,"rejected_moves":capture.rejected_moves,
        "land_entry_choices":[
            {"card_object":paid.card.0,"choice":"pay_two_life","life_paid":2,"entered_tapped":false,"controller_life_after_entry":paid.controller_life_after_entry},
            {"card_object":declined.card.0,"choice":"decline","life_paid":0,"entered_tapped":true,"controller_life_after_entry":declined.controller_life_after_entry}
        ],
        "pilot_life_after_entry_payment":paid.controller_life_after_entry,
        "life_totals":capture.life_totals,
        "sacred_foundry_mana_receipts":bound_receipts.iter().map(|receipt| json!({"source":receipt.source.0,"color":format!("{:?}",receipt.color),"amount":1,"tapped":receipt.tapped,"event_index":receipt.event_index})).collect::<Vec<_>>(),
        "colored_mana_use_order":{"paid_foundry_white_event_index":white.event_index,"gate_hound_payment_event_index":hound_payment,"paid_foundry_red_event_index":red.event_index,"boros_recruit_payment_event_index":recruit_payment},
        "white_and_red_mana_used":white.color == Color::White && red.color == Color::Red,
        "gate_hound":{"name":"Gate Hound","object":hound.0,"mana_cost":"{2}{W}","payment":["White plus two generic"],"power":1,"toughness":1,"cast_and_resolved":true},
        "boros_recruit":{"name":"Boros Recruit","object":recruit.0,"mana_cost":"{R/W}","hybrid_choice":"Red","payment":["Red"],"power":1,"toughness":1,"first_strike":true,"cast_and_resolved":true,"attack_damage":1},
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

struct SacredFoundryPolicy {
    player: PlayerId,
    foundry_choices: u8,
    hound_cast: bool,
    recruit_cast: bool,
    foundry_white_produced: bool,
    foundry_red_produced: bool,
}
impl SacredFoundryPolicy {
    fn new(player: PlayerId) -> Self {
        Self {
            player,
            foundry_choices: 0,
            hound_cast: false,
            recruit_cast: false,
            foundry_white_produced: false,
            foundry_red_produced: false,
        }
    }
    fn play_land(&mut self, view: &GameView) -> Option<PolicyAction> {
        if view.lands_played != 0 {
            return None;
        }
        if self.foundry_choices < 2
            && let Some(land) = view
                .hand
                .iter()
                .find(|card| card.definition == Some(CARD_ID))
        {
            let pay_life = self.foundry_choices == 0;
            self.foundry_choices += 1;
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
        let required = if definition == HOUND_ID {
            Color::White
        } else {
            Color::Red
        };
        let already_sourced = if required == Color::White {
            self.foundry_white_produced
        } else {
            self.foundry_red_produced
        };
        if !already_sourced {
            if let Some(foundry) = view
                .own_battlefield
                .iter()
                .find(|land| land.definition == Some(CARD_ID) && !land.tapped)
            {
                if required == Color::White {
                    self.foundry_white_produced = true;
                } else {
                    self.foundry_red_produced = true;
                }
                return Some(PolicyAction::ActivateBoundManaAbility {
                    activation: ManaAbilityActivation {
                        source: foundry.id,
                        ability_id: MANA_ABILITY_ID,
                        chosen_color: Some(required),
                    },
                });
            }
            return None;
        }
        let mana_selection = if definition == RECRUIT_ID {
            (view.mana_pool.amount(Color::Red) > 0).then_some(ManaPaymentSelection {
                generic: vec![],
                hybrid: vec![Color::Red],
            })
        } else {
            payment(view, &cost)
        };
        if let Some(mana_selection) = mana_selection {
            if definition == HOUND_ID {
                self.hound_cast = true;
            } else {
                self.recruit_cast = true;
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
            if let Some(foundry) = view
                .own_battlefield
                .iter()
                .find(|land| land.definition == Some(CARD_ID) && !land.tapped)
            {
                if required == Color::White {
                    self.foundry_white_produced = true;
                } else {
                    self.foundry_red_produced = true;
                }
                return Some(PolicyAction::ActivateBoundManaAbility {
                    activation: ManaAbilityActivation {
                        source: foundry.id,
                        ability_id: MANA_ABILITY_ID,
                        chosen_color: Some(required),
                    },
                });
            }
            return None;
        }
        let generic_color = if definition == HOUND_ID {
            Color::Blue
        } else {
            Color::Black
        };
        let basic_id = if generic_color == Color::Blue {
            "RAV-ISLAND"
        } else {
            "RAV-SWAMP"
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
impl CodePolicy for SacredFoundryPolicy {
    fn id(&self) -> &'static str {
        "rav.sacred-foundry-choice-pilot.v1"
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
                            && (card.definition == Some(HOUND_ID)
                                || card.definition == Some(RECRUIT_ID))
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
        if !self.hound_cast
            && let Some(action) =
                self.cast(view, HOUND_ID, ManaCost::with_colors(2, [Color::White]))
        {
            return action;
        }
        if self.hound_cast
            && !self.recruit_cast
            && let Some(action) =
                self.cast(view, RECRUIT_ID, ManaCost::with_hybrid(0, [], [HybridManaSymbol { first: Color::Red, second: Color::White }]))
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
        "rav.sacred-foundry-pass-opponent.v1"
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
