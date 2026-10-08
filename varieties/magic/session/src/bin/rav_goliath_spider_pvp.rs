//! Full-game RAV Goliath Spider Reach trace authority.

use cardbench_magic_engine::{
    BasicLandType, CardDefinition, CardType, Color, CombatBlock, DeckEntry, DeckList, Game,
    GameEvent, GameView, Keyword, ManaCost, ManaPaymentSelection, ObjectId, PlayerId, PolicyAction,
    Step, Zone,
};
use cardbench_magic_policies::{CodePolicy, DeckMatchConfig};
use cardbench_magic_protocol::ViewerScope;
use cardbench_magic_rav::{
    CardSemanticStatus, CatalogCard, RAV_MAIN_SET_EXPECTED_PRINTING_COUNT,
    RAV_MAIN_SET_EXPECTED_UNIQUE_NAME_COUNT, card_definitions, rav_basic_land_type_bindings,
    rav_main_set_catalog,
};
use cardbench_magic_session::{ProjectedEvent, project_events};
use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::process::ExitCode;
use syn::{Expr, Item, Lit, Meta, PathArguments, Stmt, Type, UseTree, Visibility};

const TASK_ID: &str = "cardbench/magic/card";
const SOURCE_PATH: &str = "varieties/magic/sets/ravnica_city_of_guilds/src/cards/goliath_spider.rs";
const DECK_P0: &str = "rav_goliath_spider_reach_defense";
const DECK_P1: &str = "rav_skyknight_legionnaire_flying_attack";
const MAX_SEED: u64 = 128;

#[path = "../../../sets/ravnica_city_of_guilds/src/cards/goliath_spider.rs"]
mod candidate_goliath_spider;

fn main() -> ExitCode {
    match run_authority() {
        Ok(report) => {
            if let Some(root) = output_root()
                && let Err(error) = write_outputs(&root, &report)
            {
                eprintln!("could not write Goliath Spider result: {error}");
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
            eprintln!("Goliath Spider PvP authority failed: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run_authority() -> Result<Value, String> {
    validate_candidate_source()?;
    validate_candidate_definition()?;
    let printings = rav_main_set_catalog();
    let mut names = BTreeSet::new();
    let catalog: Vec<CatalogCard> = printings
        .iter()
        .copied()
        .filter(|card| names.insert(card.name))
        .collect();
    if catalog.len() != RAV_MAIN_SET_EXPECTED_UNIQUE_NAME_COUNT
        || printings.len() != RAV_MAIN_SET_EXPECTED_PRINTING_COUNT
    {
        return Err(format!(
            "RAV catalog has {} names / {} printings; expected {} / {}",
            catalog.len(),
            printings.len(),
            RAV_MAIN_SET_EXPECTED_UNIQUE_NAME_COUNT,
            RAV_MAIN_SET_EXPECTED_PRINTING_COUNT,
        ));
    }

    let (seed, game) = find_trace()?;
    let trace = validate_trace(&game, seed)?;
    let replay = run_match(candidate_goliath_spider::definition(), seed, false)?;
    if game.events != replay.events
        || game.terminal != replay.terminal
        || game.winner != replay.winner
        || game.goliath_spider != replay.goliath_spider
        || game.skyknight != replay.skyknight
        || game.block_attempt_accepted != replay.block_attempt_accepted
        || game.block_attempt_rejected != replay.block_attempt_rejected
    {
        return Err("Goliath Spider Reach PvP replay was not deterministic".to_owned());
    }

    let mutation_detected = reach_removal_mutation_fails(seed)?;
    if !mutation_detected {
        return Err("removing Goliath Spider Reach escaped the PvP trace".to_owned());
    }
    let breakdown: Vec<Value> = catalog.iter().map(card_status).collect();
    let cards_passed = breakdown
        .iter()
        .filter(|card| card["trace_status"] == "passed")
        .count();
    Ok(json!({
        "schema_version": "cardbench.magic.card-pvp.check.v1",
        "task_id": TASK_ID,
        "expansion": "RAV",
        "card": {"collector_number": 168, "definition_id": "RAV-GOLIATH-SPIDER", "name": "Goliath Spider"},
        "candidate": {"kind": candidate_kind(), "card_definition": "RAV-GOLIATH-SPIDER"},
        "candidate_kind": candidate_kind(),
        "viewer_scope": format!("{:?}", ViewerScope::Judge),
        "rules_authority": {
            "comprehensive_rules": "2026-08-07",
            "checked_sections": [
                "105.2 and 202.1-202.2: green identity and colored payment",
                "117: priority and timing",
                "302.1-302.6: creature permanents, combat, and summoning sickness",
                "508: declaring attackers",
                "509.1: declaring blockers and evasion restrictions",
                "601.2 and 608.3a: casting and resolving creature spells",
                "702.9: Flying",
                "702.17: Reach"
            ],
            "source_contract": "RAV printing identity and the Reach/Flying blocking interaction",
            "authority_cutoff": "2026-09-24"
        },
        "passed": true,
        "pilot_card_passed": true,
        "replay_deterministic": true,
        "cards_passed": cards_passed,
        "cards_credited": 0,
        "cards_total": catalog.len(),
        "printings_total": printings.len(),
        "coverage_complete": false,
        "full_reward": false,
        "harbor_reward": 0.0,
        "pilot_reward": 0.0,
        "qualification_status": qualification_status(),
        "mutation_sensitivity": {
            "reach_removed_mutation_detected": mutation_detected,
            "sealed_mutation_authority": false
        },
        "traces": [trace],
        "per_card_breakdown": breakdown
    }))
}

/// Candidate source is only evaluated after proving it is a literal-only card definition.
fn validate_candidate_source() -> Result<(), String> {
    let source = std::fs::read_to_string(SOURCE_PATH)
        .map_err(|error| format!("could not read Goliath Spider source: {error}"))?;
    let file = syn::parse_file(&source)
        .map_err(|_| "Goliath Spider source is not valid declarative Rust".to_owned())?;
    if !file.attrs.iter().all(is_doc_attribute) {
        return Err("Goliath Spider module attributes differ from the task contract".to_owned());
    }
    let expected_imports: BTreeSet<Vec<String>> = [
        vec!["std", "collections", "BTreeSet"],
        vec!["cardbench_magic_engine", "CardDefinition"],
        vec!["cardbench_magic_engine", "CardType"],
        vec!["cardbench_magic_engine", "Color"],
        vec!["cardbench_magic_engine", "Keyword"],
        vec!["cardbench_magic_engine", "ManaCost"],
    ]
    .into_iter()
    .map(|path| path.into_iter().map(str::to_owned).collect())
    .collect();
    let mut imports = BTreeSet::new();
    let mut definition = None;
    for item in &file.items {
        match item {
            Item::Use(item)
                if item.attrs.is_empty() && matches!(&item.vis, Visibility::Inherited) =>
            {
                let Some(paths) = flattened_use_paths(item) else {
                    return Err("Goliath Spider source has an unsupported import".to_owned());
                };
                imports.extend(paths);
            }
            Item::Fn(item) if item.sig.ident == "definition" && definition.is_none() => {
                definition = Some(item)
            }
            _ => return Err("Goliath Spider source contains executable top-level code".to_owned()),
        }
    }
    if imports != expected_imports || file.items.len() != 3 {
        return Err(
            "Goliath Spider imports or item structure differ from the task contract".to_owned(),
        );
    }
    validate_definition_function(
        definition.ok_or_else(|| "Goliath Spider definition() is missing".to_owned())?,
    )
}

fn validate_definition_function(function: &syn::ItemFn) -> Result<(), String> {
    let safe_attrs = function.attrs.iter().all(|attribute| {
        is_doc_attribute(attribute)
            || attribute.path().is_ident("must_use") && matches!(attribute.meta, Meta::Path(_))
    });
    let crate_visibility =
        matches!(&function.vis, Visibility::Restricted(value) if value.path.is_ident("crate"));
    if !safe_attrs
        || !crate_visibility
        || function.sig.constness.is_some()
        || function.sig.asyncness.is_some()
        || function.sig.unsafety.is_some()
        || function.sig.abi.is_some()
        || !function.sig.generics.params.is_empty()
        || function.sig.generics.where_clause.is_some()
        || !function.sig.inputs.is_empty()
        || function.sig.variadic.is_some()
        || !matches!(&function.sig.output, syn::ReturnType::Type(_, ty) if type_is(ty, &["CardDefinition"]))
        || function.block.stmts.len() != 1
    {
        return Err("Goliath Spider definition() has an unsupported signature".to_owned());
    }
    let Stmt::Expr(Expr::Struct(value), None) = &function.block.stmts[0] else {
        return Err("Goliath Spider definition() must return one direct literal".to_owned());
    };
    if !path_is(&value.path, &["CardDefinition"]) || value.rest.is_some() || !value.attrs.is_empty()
    {
        return Err("Goliath Spider definition literal type differs".to_owned());
    }
    let fields = literal_fields(value)?;
    if fields.len() != 13 {
        return Err("Goliath Spider definition fields differ from CardDefinition".to_owned());
    }
    check_string(fields["id"], "RAV-GOLIATH-SPIDER")?;
    check_string(fields["name"], "Goliath Spider")?;
    check_string(fields["set_code"], "RAV")?;
    check_cost(fields["mana_cost"])?;
    check_single_set_path(fields["colors"], &["Color", "Green"])?;
    check_empty_set(fields["mana_colors"])?;
    check_single_set_path(fields["card_types"], &["CardType", "Creature"])?;
    check_bool(fields["is_basic_land"], false)?;
    check_rules(fields["supported_rules"])?;
    check_some_integer(fields["power"], 7)?;
    check_some_integer(fields["toughness"], 6)?;
    check_keyword_or_empty(fields["keywords"])?;
    check_empty_vec(fields["effects"])?;
    Ok(())
}

fn validate_candidate_definition() -> Result<(), String> {
    let submitted = candidate_goliath_spider::definition();
    let registered = card_definitions()
        .into_iter()
        .find(|card| card.id == "RAV-GOLIATH-SPIDER")
        .ok_or_else(|| "RAV-GOLIATH-SPIDER is missing from the executable registry".to_owned())?;
    if submitted.id != registered.id
        || submitted.name != "Goliath Spider"
        || submitted.set_code != "RAV"
        || submitted.mana_cost != ManaCost::with_colors(6, [Color::Green, Color::Green])
        || submitted.colors != BTreeSet::from([Color::Green])
        || !submitted.mana_colors.is_empty()
        || submitted.card_types != BTreeSet::from([CardType::Creature])
        || submitted.is_basic_land
        || submitted.power != Some(7)
        || submitted.toughness != Some(6)
        || !submitted.effects.is_empty()
        || submitted.keywords != [Keyword::Reach] && !submitted.keywords.is_empty()
    {
        return Err("RAV-GOLIATH-SPIDER identity or printed characteristics differ".to_owned());
    }
    Ok(())
}

fn new_game(definition: CardDefinition) -> Result<Game, String> {
    let mut definitions = card_definitions();
    let target = definitions
        .iter_mut()
        .find(|card| card.id == "RAV-GOLIATH-SPIDER")
        .ok_or_else(|| "RAV-GOLIATH-SPIDER is missing from the executable registry".to_owned())?;
    *target = definition;
    Game::new_with_basic_land_types(definitions, 2, rav_basic_land_type_bindings())
        .map_err(|error| error.to_string())
}

fn find_trace() -> Result<(u64, CapturedMatch), String> {
    if let Ok(value) = std::env::var("CARDBENCH_GOLIATH_SPIDER_SEED") {
        let seed = value
            .parse::<u64>()
            .map_err(|_| "CARDBENCH_GOLIATH_SPIDER_SEED must be an unsigned integer".to_owned())?;
        return Ok((
            seed,
            run_match(candidate_goliath_spider::definition(), seed, false)?,
        ));
    }
    for seed in 0..=MAX_SEED {
        if let Ok(preview) = run_match(candidate_goliath_spider::definition(), seed, true)
            && preview.block_attempt_seen
            && preview.goliath_spider.is_some()
            && preview.skyknight.is_some()
            && preview
                .skyknight
                .zip(preview.goliath_spider)
                .is_some_and(|(skyknight, goliath)| {
                    let skyknight_cast = event_index(&preview.events, |event| {
                        matches!(event,
                    GameEvent::SpellCast { player: PlayerId(1), card } if *card == skyknight)
                    });
                    let goliath_entered = event_index(&preview.events, |event| {
                        matches!(event,
                    GameEvent::CardMoved { card, to: Zone::Battlefield } if *card == goliath)
                    });
                    skyknight_cast
                        .zip(goliath_entered)
                        .is_some_and(|(cast, entered)| cast < entered)
                })
        {
            return Ok((
                seed,
                run_match(candidate_goliath_spider::definition(), seed, false)?,
            ));
        }
    }
    Err(format!(
        "no Skyknight-versus-Goliath block challenge found for seeds 0..={MAX_SEED}"
    ))
}

fn reach_removal_mutation_fails(seed: u64) -> Result<bool, String> {
    let mut mutated = candidate_goliath_spider::definition();
    mutated
        .keywords
        .retain(|keyword| *keyword != Keyword::Reach);
    let captured = run_match(mutated, seed, false)?;
    Ok(validate_trace(&captured, seed).err().is_some_and(|error| {
        error.contains("Goliath Spider did not legally block the attacking Skyknight Legionnaire")
    }))
}

#[derive(Clone)]
struct CapturedMatch {
    events: Vec<GameEvent>,
    terminal: String,
    winner: Option<usize>,
    turns: u32,
    accepted_moves: u32,
    rejected_moves: u32,
    goliath_spider: Option<ObjectId>,
    skyknight: Option<ObjectId>,
    block_attempt_seen: bool,
    block_attempt_accepted: bool,
    block_attempt_rejected: bool,
    block_rejection_reason: Option<String>,
    attempt_event_count_before: Option<usize>,
    attempt_event_count_after: Option<usize>,
    goliath_live: bool,
    skyknight_graveyard: bool,
    live_reach: bool,
    live_power_toughness: Option<(i32, i32)>,
    mana_paid_goliath: Vec<Color>,
    mana_paid_skyknight: Vec<Color>,
}

fn run_match(
    definition: CardDefinition,
    seed: u64,
    preview: bool,
) -> Result<CapturedMatch, String> {
    let goliath_deck = DeckList {
        mainboard: vec![
            DeckEntry {
                card: "RAV-GOLIATH-SPIDER".to_owned(),
                count: 4,
            },
            DeckEntry {
                card: "RAV-FOREST".to_owned(),
                count: 56,
            },
        ],
        sideboard: vec![],
    };
    let flyer_deck = DeckList {
        mainboard: vec![
            DeckEntry {
                card: "RAV-SKYKNIGHT-LEGIONNAIRE".to_owned(),
                count: 4,
            },
            DeckEntry {
                card: "RAV-PLAINS".to_owned(),
                count: 28,
            },
            DeckEntry {
                card: "RAV-MOUNTAIN".to_owned(),
                count: 28,
            },
        ],
        sideboard: vec![],
    };
    for (name, deck) in [(DECK_P0, &goliath_deck), (DECK_P1, &flyer_deck)] {
        let count: usize = deck
            .mainboard
            .iter()
            .map(|entry| usize::from(entry.count))
            .sum();
        if count != 60 {
            return Err(format!("{name} deck has {count} cards, expected 60"));
        }
    }
    let config = DeckMatchConfig {
        shuffle_seed: seed,
        ..DeckMatchConfig::default()
    };
    let mut game = new_game(definition.clone())?;
    game.set_shuffle_seed(seed)
        .map_err(|error| error.to_string())?;
    game.load_deck_into_library(PlayerId(0), &goliath_deck)
        .map_err(|error| error.to_string())?;
    game.load_deck_into_library(PlayerId(1), &flyer_deck)
        .map_err(|error| error.to_string())?;
    game.draw_opening_hand(PlayerId(0), config.opening_hand_size)
        .map_err(|error| error.to_string())?;
    game.draw_opening_hand(PlayerId(1), config.opening_hand_size)
        .map_err(|error| error.to_string())?;
    game.begin_game().map_err(|error| error.to_string())?;

    let mut policies: [Box<dyn CodePolicy>; 2] = [
        Box::new(GoliathPolicy::new(PlayerId(0), definition.mana_cost)),
        Box::new(SkyknightPolicy::new(PlayerId(1))),
    ];
    let mut attempted_moves = 0_u32;
    let mut accepted_moves = 0_u32;
    let mut block_attempt_seen = false;
    let mut block_attempt_accepted = false;
    let mut block_attempt_rejected = false;
    let mut block_rejection_reason = None;
    let mut attempt_event_count_before = None;
    let mut attempt_event_count_after = None;
    let mut goliath = None;
    let mut skyknight = None;

    while !game.is_game_over() {
        if game.turn > config.max_turns || accepted_moves >= config.max_policy_moves {
            return Err("Goliath Spider PvP match exceeded its development bound".to_owned());
        }
        if preview && game.turn > 20 {
            break;
        }
        let player = game.next_policy_player();
        let view = game
            .view_for_player(player)
            .map_err(|error| error.to_string())?;
        let policy = policies.get_mut(player.0).ok_or("policy seat missing")?;
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
        let challenge = player == PlayerId(0)
            && matches!(&action, PolicyAction::DeclareBlockers { assignments } if assignments.len() == 1);
        if challenge {
            block_attempt_seen = true;
            attempt_event_count_before = Some(game.event_log.len());
            if let PolicyAction::DeclareBlockers { assignments } = &action {
                skyknight = assignments.first().map(|block| block.attacker);
                goliath = assignments.first().map(|block| block.blocker);
            }
        }
        attempted_moves += 1;
        match game.submit_policy_move(player, policy.id(), action) {
            Ok(()) => {
                accepted_moves += 1;
                if challenge {
                    block_attempt_accepted = true;
                    attempt_event_count_after = Some(game.event_log.len());
                    if preview {
                        break;
                    }
                }
            }
            Err(error) if challenge => {
                let diagnostic = error.to_string();
                if !diagnostic.contains("flying attacker can be blocked only by flying or reach") {
                    return Err(format!(
                        "Reach block challenge rejected for an unexpected reason: {diagnostic}"
                    ));
                }
                block_attempt_rejected = true;
                block_rejection_reason = Some(diagnostic);
                attempt_event_count_after = Some(game.event_log.len());
                if attempt_event_count_before != attempt_event_count_after {
                    return Err("rejected flying-block attempt changed the event log".to_owned());
                }
                if preview {
                    break;
                }
            }
            Err(error) => {
                return Err(format!(
                    "policy move rejected on turn {} at {:?}: {error}",
                    view.turn, view.step
                ));
            }
        }
        game.validate_invariants()
            .map_err(|error| format!("post-move invariant failure: {error}"))?;
    }

    let events = game.event_log.clone();
    goliath = events
        .iter()
        .find_map(|event| match event {
            GameEvent::SpellCast {
                player: PlayerId(0),
                card,
            } if game
                .card_definition(*card)
                .is_ok_and(|d| d.id == "RAV-GOLIATH-SPIDER") =>
            {
                Some(*card)
            }
            _ => None,
        })
        .or(goliath);
    skyknight = events
        .iter()
        .find_map(|event| match event {
            GameEvent::SpellCast {
                player: PlayerId(1),
                card,
            } if game
                .card_definition(*card)
                .is_ok_and(|d| d.id == "RAV-SKYKNIGHT-LEGIONNAIRE") =>
            {
                Some(*card)
            }
            _ => None,
        })
        .or(skyknight);
    let goliath_live = goliath.is_some_and(|card| game.zone_of(card) == Some(Zone::Battlefield));
    let skyknight_graveyard = skyknight.is_some_and(|card| {
        events.iter().any(|event| {
            matches!(event, GameEvent::CardMoved { card: moved, to: Zone::Graveyard } if *moved == card)
        })
    });
    let characteristics = goliath.and_then(|card| game.characteristics(card).ok());
    let live_reach = characteristics
        .as_ref()
        .is_some_and(|c| c.keywords.contains(&Keyword::Reach));
    let live_power_toughness = characteristics.and_then(|c| Some((c.power?, c.toughness?)));
    let mana_paid_goliath = goliath
        .and_then(|card| {
            events.iter().find_map(|event| match event {
                GameEvent::SpellManaPaid {
                    player: PlayerId(0),
                    card: paid,
                    colors,
                } if *paid == card => Some(colors.clone()),
                _ => None,
            })
        })
        .unwrap_or_default();
    let mana_paid_skyknight = skyknight
        .and_then(|card| {
            events.iter().find_map(|event| match event {
                GameEvent::SpellManaPaid {
                    player: PlayerId(1),
                    card: paid,
                    colors,
                } if *paid == card => Some(colors.clone()),
                _ => None,
            })
        })
        .unwrap_or_default();
    let winner = game.winner().map(|player| player.0);
    Ok(CapturedMatch {
        events,
        terminal: if preview && !game.is_game_over() {
            "InProgress".to_owned()
        } else {
            winner.map_or_else(
                || "Draw".to_owned(),
                |seat| format!("Winner(PlayerId({seat}))"),
            )
        },
        winner,
        turns: game.turn,
        accepted_moves,
        rejected_moves: attempted_moves.saturating_sub(accepted_moves),
        goliath_spider: goliath,
        skyknight,
        block_attempt_seen,
        block_attempt_accepted,
        block_attempt_rejected,
        block_rejection_reason,
        attempt_event_count_before,
        attempt_event_count_after,
        goliath_live,
        skyknight_graveyard,
        live_reach,
        live_power_toughness,
        mana_paid_goliath,
        mana_paid_skyknight,
    })
}

struct GoliathPolicy {
    player: PlayerId,
    cost: ManaCost,
    cast_once: bool,
    challenge_once: bool,
    attack_unlocked: bool,
}

impl GoliathPolicy {
    fn new(player: PlayerId, cost: ManaCost) -> Self {
        Self {
            player,
            cost,
            cast_once: false,
            challenge_once: false,
            attack_unlocked: false,
        }
    }

    fn payment(&self, view: &GameView) -> Option<ManaPaymentSelection> {
        let mut available: Vec<(Color, u8)> = Color::MANA_ALL
            .into_iter()
            .map(|color| (color, view.mana_pool.amount(color)))
            .collect();
        for color in &self.cost.colored {
            let (_, amount) = available
                .iter_mut()
                .find(|(available_color, _)| available_color == color)?;
            if *amount == 0 {
                return None;
            }
            *amount -= 1;
        }
        let mut generic = Vec::with_capacity(usize::from(self.cost.generic));
        for _ in 0..self.cost.generic {
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

    fn play_land(&self, view: &GameView) -> Option<PolicyAction> {
        if view.lands_played != 0 {
            return None;
        }
        view.hand
            .iter()
            .find(|card| card.basic_land_type == Some(BasicLandType::Forest))
            .map(|card| PolicyAction::PlayLand { card: card.id })
    }

    fn add_mana(&self, view: &GameView) -> Option<PolicyAction> {
        let land = view
            .own_battlefield
            .iter()
            .find(|card| !card.tapped && card.basic_land_type == Some(BasicLandType::Forest))?;
        Some(PolicyAction::ActivateManaAbility {
            land: land.id,
            color: Color::Green,
        })
    }
}

impl CodePolicy for GoliathPolicy {
    fn id(&self) -> &'static str {
        "rav.goliath-spider-reach-defense.v1"
    }

    fn propose_move(&mut self, view: &GameView) -> PolicyAction {
        if view.decision_player != self.player {
            return PolicyAction::PassPriority;
        }
        if view.step == Step::DeclareAttackers
            && view.active_player == self.player
            && !view.attackers_declared
        {
            if !self.attack_unlocked {
                return PolicyAction::DeclareAttackers { attackers: vec![] };
            }
            return PolicyAction::DeclareAttackers {
                attackers: view
                    .own_battlefield
                    .iter()
                    .filter(|card| card.definition == Some("RAV-GOLIATH-SPIDER") && card.can_attack)
                    .map(|card| card.id)
                    .collect(),
            };
        }
        if view.step == Step::DeclareBlockers
            && view.active_player != self.player
            && !view.blockers_declared
        {
            if !self.challenge_once {
                let attacker = view
                    .combat_attackers
                    .iter()
                    .find(|card| card.definition == Some("RAV-SKYKNIGHT-LEGIONNAIRE"));
                let blocker = view
                    .own_battlefield
                    .iter()
                    .find(|card| card.definition == Some("RAV-GOLIATH-SPIDER") && card.can_block);
                if let (Some(attacker), Some(blocker)) = (attacker, blocker) {
                    self.challenge_once = true;
                    self.attack_unlocked = true;
                    return PolicyAction::DeclareBlockers {
                        assignments: vec![CombatBlock {
                            attacker: attacker.id,
                            blocker: blocker.id,
                        }],
                    };
                }
            }
            return PolicyAction::DeclareBlockers {
                assignments: vec![],
            };
        }
        if view.active_player != self.player
            || view.priority != self.player
            || view.step != Step::PrecombatMain
            || view.stack_depth != 0
        {
            return PolicyAction::PassPriority;
        }
        if let Some(land) = self.play_land(view) {
            return land;
        }
        if !self.cast_once
            && let Some(card) = view
                .hand
                .iter()
                .find(|card| card.definition == Some("RAV-GOLIATH-SPIDER"))
        {
            if let Some(mana_selection) = self.payment(view) {
                self.cast_once = true;
                return PolicyAction::CastWithPayment {
                    request: cardbench_magic_engine::CastRequest {
                        card: card.id,
                        targets: vec![],
                        convoke: vec![],
                        payment_mana_abilities: vec![],
                    },
                    chosen_x: None,
                    mana_selection,
                };
            }
            if let Some(action) = self.add_mana(view) {
                return action;
            }
        }
        self.add_mana(view).unwrap_or(PolicyAction::PassPriority)
    }
}

struct SkyknightPolicy {
    player: PlayerId,
    cost: ManaCost,
    cast_once: bool,
}

impl SkyknightPolicy {
    fn new(player: PlayerId) -> Self {
        Self {
            player,
            cost: ManaCost::with_colors(1, [Color::Red, Color::White]),
            cast_once: false,
        }
    }

    fn payment(&self, view: &GameView) -> Option<ManaPaymentSelection> {
        let mut available: Vec<(Color, u8)> = Color::MANA_ALL
            .into_iter()
            .map(|color| (color, view.mana_pool.amount(color)))
            .collect();
        for color in &self.cost.colored {
            let (_, amount) = available
                .iter_mut()
                .find(|(available_color, _)| available_color == color)?;
            if *amount == 0 {
                return None;
            }
            *amount -= 1;
        }
        let mut generic = Vec::with_capacity(usize::from(self.cost.generic));
        for _ in 0..self.cost.generic {
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

    fn play_land(&self, view: &GameView) -> Option<PolicyAction> {
        if view.lands_played != 0 {
            return None;
        }
        let has_red = view
            .own_battlefield
            .iter()
            .any(|card| card.basic_land_type == Some(BasicLandType::Mountain));
        let has_white = view
            .own_battlefield
            .iter()
            .any(|card| card.basic_land_type == Some(BasicLandType::Plains));
        view.hand
            .iter()
            .find(|card| {
                (!has_red && card.basic_land_type == Some(BasicLandType::Mountain))
                    || (!has_white && card.basic_land_type == Some(BasicLandType::Plains))
            })
            .or_else(|| {
                view.hand
                    .iter()
                    .find(|card| card.card_types.contains(&CardType::Land))
            })
            .map(|card| PolicyAction::PlayLand { card: card.id })
    }

    fn add_mana(&self, view: &GameView) -> Option<PolicyAction> {
        for color in [Color::Red, Color::White] {
            let has_color = self
                .cost
                .colored
                .iter()
                .filter(|needed| **needed == color)
                .count()
                <= view.mana_pool.amount(color) as usize;
            if !has_color {
                let land_type = if color == Color::Red {
                    BasicLandType::Mountain
                } else {
                    BasicLandType::Plains
                };
                if let Some(land) = view
                    .own_battlefield
                    .iter()
                    .find(|card| !card.tapped && card.basic_land_type == Some(land_type))
                {
                    return Some(PolicyAction::ActivateManaAbility {
                        land: land.id,
                        color,
                    });
                }
            }
        }
        view.own_battlefield
            .iter()
            .find(|card| {
                !card.tapped
                    && matches!(
                        card.basic_land_type,
                        Some(BasicLandType::Mountain | BasicLandType::Plains)
                    )
            })
            .map(|land| PolicyAction::ActivateManaAbility {
                land: land.id,
                color: if land.basic_land_type == Some(BasicLandType::Mountain) {
                    Color::Red
                } else {
                    Color::White
                },
            })
    }
}

impl CodePolicy for SkyknightPolicy {
    fn id(&self) -> &'static str {
        "rav.skyknight-legionnaire-flying-attack.v1"
    }

    fn propose_move(&mut self, view: &GameView) -> PolicyAction {
        if view.decision_player != self.player {
            return PolicyAction::PassPriority;
        }
        if view.step == Step::DeclareAttackers
            && view.active_player == self.player
            && !view.attackers_declared
        {
            return PolicyAction::DeclareAttackers {
                attackers: view
                    .own_battlefield
                    .iter()
                    .filter(|card| {
                        card.definition == Some("RAV-SKYKNIGHT-LEGIONNAIRE") && card.can_attack
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
        if self.cast_once
            || view.active_player != self.player
            || view.priority != self.player
            || view.step != Step::PrecombatMain
            || view.stack_depth != 0
        {
            return PolicyAction::PassPriority;
        }
        if let Some(land) = self.play_land(view) {
            return land;
        }
        if let Some(card) = view
            .hand
            .iter()
            .find(|card| card.definition == Some("RAV-SKYKNIGHT-LEGIONNAIRE"))
        {
            if let Some(mana_selection) = self.payment(view) {
                self.cast_once = true;
                return PolicyAction::CastWithPayment {
                    request: cardbench_magic_engine::CastRequest {
                        card: card.id,
                        targets: vec![],
                        convoke: vec![],
                        payment_mana_abilities: vec![],
                    },
                    chosen_x: None,
                    mana_selection,
                };
            }
            if let Some(action) = self.add_mana(view) {
                return action;
            }
        }
        self.add_mana(view).unwrap_or(PolicyAction::PassPriority)
    }
}

fn validate_trace(captured: &CapturedMatch, seed: u64) -> Result<Value, String> {
    if !captured.block_attempt_seen
        || captured.goliath_spider.is_none()
        || captured.skyknight.is_none()
    {
        return Err(
            "Goliath Spider trace did not stage a Skyknight flying-block challenge".to_owned(),
        );
    }
    if !captured.block_attempt_accepted {
        return Err(
            "Goliath Spider did not legally block the attacking Skyknight Legionnaire".to_owned(),
        );
    }
    if captured.block_attempt_rejected || captured.rejected_moves != 0 {
        return Err("Goliath Spider Reach challenge had an unexpected rejected action".to_owned());
    }
    if captured.terminal != "Winner(PlayerId(0))" || captured.winner != Some(0) {
        return Err(
            "Goliath Spider PvP match did not reach a clean seat-zero terminal win".to_owned(),
        );
    }
    if !captured.goliath_live || !captured.skyknight_graveyard || !captured.live_reach {
        return Err(
            "Goliath Spider or the blocked Skyknight ended in the wrong live zone or state"
                .to_owned(),
        );
    }
    if captured.live_power_toughness != Some((7, 6)) {
        return Err(format!(
            "resolved Goliath Spider is not a 7/6: {:?}",
            captured.live_power_toughness
        ));
    }
    let goliath = captured.goliath_spider.expect("validated Goliath object");
    let skyknight = captured.skyknight.expect("validated Skyknight object");
    let green_spent = captured
        .mana_paid_goliath
        .iter()
        .filter(|color| **color == Color::Green)
        .count();
    let red_spent = captured
        .mana_paid_skyknight
        .iter()
        .filter(|color| **color == Color::Red)
        .count();
    let white_spent = captured
        .mana_paid_skyknight
        .iter()
        .filter(|color| **color == Color::White)
        .count();
    if green_spent < 2 || captured.mana_paid_goliath.len() != 8 {
        return Err(format!(
            "Goliath Spider cast paid {:?}; six generic and two green symbols are required",
            captured.mana_paid_goliath
        ));
    }
    if red_spent < 1 || white_spent < 1 || captured.mana_paid_skyknight.len() != 3 {
        return Err(format!(
            "Skyknight cast paid {:?}; one red, one white, and one generic symbol are required",
            captured.mana_paid_skyknight
        ));
    }
    let projected = project_events(&captured.events);
    let goliath_paid = event_index(&captured.events, |event| matches!(event, GameEvent::SpellManaPaid { player: PlayerId(0), card, .. } if *card == goliath)).ok_or("Goliath Spider SpellManaPaid missing")?;
    let goliath_cast = event_index(&captured.events, |event| matches!(event, GameEvent::SpellCast { player: PlayerId(0), card } if *card == goliath)).ok_or("Goliath Spider SpellCast missing")?;
    let goliath_resolved = event_index(
        &captured.events,
        |event| matches!(event, GameEvent::SpellResolved { card } if *card == goliath),
    )
    .ok_or("Goliath Spider SpellResolved missing")?;
    let goliath_entered = event_index(&captured.events, |event| matches!(event, GameEvent::CardMoved { card, to: Zone::Battlefield } if *card == goliath)).ok_or("Goliath Spider battlefield entry missing")?;
    let skyknight_paid = event_index(&captured.events, |event| matches!(event, GameEvent::SpellManaPaid { player: PlayerId(1), card, .. } if *card == skyknight)).ok_or("Skyknight SpellManaPaid missing")?;
    let skyknight_cast = event_index(&captured.events, |event| matches!(event, GameEvent::SpellCast { player: PlayerId(1), card } if *card == skyknight)).ok_or("Skyknight SpellCast missing")?;
    let skyknight_resolved = event_index(
        &captured.events,
        |event| matches!(event, GameEvent::SpellResolved { card } if *card == skyknight),
    )
    .ok_or("Skyknight SpellResolved missing")?;
    let skyknight_entered = event_index(&captured.events, |event| matches!(event, GameEvent::CardMoved { card, to: Zone::Battlefield } if *card == skyknight)).ok_or("Skyknight battlefield entry missing")?;
    let blockers = event_index(&captured.events, |event| matches!(event, GameEvent::BlockersDeclared { player: PlayerId(0), assignments } if assignments.contains(&(skyknight, goliath)))).ok_or("Goliath Spider was not declared as Skyknight's blocker")?;
    let attack = captured.events[..blockers].iter().rposition(|event| matches!(event, GameEvent::AttackersDeclared { player: PlayerId(1), attackers } if attackers.contains(&skyknight))).ok_or("Skyknight attacker declaration before the block is missing")?;
    let skyknight_damage = event_index_after(&captured.events, blockers, |event| matches!(event, GameEvent::DamageDealtToPermanent { source, permanent, amount: 2 } if *source == skyknight && *permanent == goliath)).ok_or("blocked Skyknight damage to Goliath Spider missing")?;
    let spider_damage = event_index_after(&captured.events, blockers, |event| matches!(event, GameEvent::DamageDealtToPermanent { source, permanent, amount: 7 } if *source == goliath && *permanent == skyknight)).ok_or("Goliath Spider combat damage to Skyknight missing")?;
    let skyknight_died = event_index_after(&captured.events, spider_damage, |event| matches!(event, GameEvent::CardMoved { card, to: Zone::Graveyard } if *card == skyknight)).ok_or("blocked Skyknight did not move to its graveyard")?;
    let spider_attack = event_index_after(&captured.events, skyknight_died, |event| matches!(event, GameEvent::AttackersDeclared { player: PlayerId(0), attackers } if attackers.contains(&goliath))).ok_or("Goliath Spider did not attack after blocking")?;
    let player_damage = event_index_after(&captured.events, spider_attack, |event| matches!(event, GameEvent::DamageDealtToPlayer { source, player: PlayerId(1), amount: 7 } if *source == goliath)).ok_or("Goliath Spider combat damage after its block is missing")?;
    let ended = event_index(&captured.events, |event| {
        matches!(
            event,
            GameEvent::GameEnded {
                winner: Some(PlayerId(0))
            }
        )
    })
    .ok_or("terminal seat-zero GameEnded missing")?;
    if !(skyknight_paid < skyknight_cast
        && skyknight_cast < skyknight_resolved
        && skyknight_resolved < skyknight_entered
        && goliath_paid < goliath_cast
        && goliath_cast < goliath_resolved
        && goliath_resolved < goliath_entered
        && skyknight_entered < attack
        && goliath_entered < attack
        && attack < blockers
        && blockers < skyknight_damage
        && skyknight_damage < spider_damage
        && spider_damage < skyknight_died
        && skyknight_died < spider_attack
        && spider_attack < player_damage
        && player_damage < ended)
    {
        return Err(
            "Goliath Spider cast, flying combat, blocker, damage, and terminal event order differs"
                .to_owned(),
        );
    }
    let relevant = project_relevant(&projected, goliath, skyknight);
    let expected = [
        "SpellManaPaid",
        "SpellCast",
        "SpellResolved",
        "CardMoved",
        "SpellManaPaid",
        "SpellCast",
        "SpellResolved",
        "CardMoved",
        "AttackersDeclared",
        "BlockersDeclared",
        "DamageDealtToPermanent",
        "DamageDealtToPermanent",
        "CardMoved",
        "AttackersDeclared",
        "DamageDealtToPlayer",
        "GameEnded",
    ];
    let kinds: Vec<_> = relevant.iter().map(|event| event.kind.as_str()).collect();
    if kinds != expected {
        return Err(format!(
            "Goliath Spider ordered projected events differ: {kinds:?}"
        ));
    }
    Ok(json!({
        "id": "rav-goliath-spider-reach-blocks-flying-skyknight",
        "seed": seed,
        "decks": [DECK_P0, DECK_P1],
        "policies": ["rav.goliath-spider-reach-defense.v1", "rav.skyknight-legionnaire-flying-attack.v1"],
        "terminal": captured.terminal,
        "winner_seat": captured.winner,
        "turns": captured.turns,
        "accepted_moves": captured.accepted_moves,
        "rejected_moves": captured.rejected_moves,
        "digest": event_digest(&captured.events),
        "goliath_spider_object": goliath.0,
        "skyknight_legionnaire_object": skyknight.0,
        "goliath_cast_payment": {"required": {"generic_symbols": 6, "green_symbols": 2, "total_symbols": 8}, "spent": {"green": green_spent, "total": captured.mana_paid_goliath.len()}},
        "skyknight_cast_payment": {"required": {"generic_symbols": 1, "red_symbols": 1, "white_symbols": 1, "total_symbols": 3}, "spent": {"red": red_spent, "white": white_spent, "total": captured.mana_paid_skyknight.len()}},
        "goliath_remains_live": captured.goliath_live,
        "live_reach": captured.live_reach,
        "resolved_power_toughness": {"power": captured.live_power_toughness.unwrap().0, "toughness": captured.live_power_toughness.unwrap().1},
        "block_attempt": {
            "flying_attacker": skyknight.0,
            "reach_blocker": goliath.0,
            "accepted": captured.block_attempt_accepted,
            "rejected": captured.block_attempt_rejected,
            "engine_reason": captured.block_rejection_reason,
            "event_count_before": captured.attempt_event_count_before,
            "event_count_after": captured.attempt_event_count_after,
            "blocked_flyer_in_graveyard": captured.skyknight_graveyard
        },
        "ordered_event_sequence": relevant.iter().map(|event| event.sequence).collect::<Vec<_>>(),
        "events": relevant,
        "passed": true
    }))
}

fn event_index(events: &[GameEvent], predicate: impl Fn(&GameEvent) -> bool) -> Option<usize> {
    events.iter().position(predicate)
}

fn event_index_after(
    events: &[GameEvent],
    start: usize,
    predicate: impl Fn(&GameEvent) -> bool,
) -> Option<usize> {
    events
        .iter()
        .enumerate()
        .skip(start + 1)
        .find_map(|(index, event)| predicate(event).then_some(index))
}

fn project_relevant(
    events: &[ProjectedEvent],
    goliath: ObjectId,
    skyknight: ObjectId,
) -> Vec<ProjectedEvent> {
    let mut goliath_entered = false;
    let mut challenge_attack_seen = false;
    let mut challenge_block_seen = false;
    let mut spider_attack_seen = false;
    let mut spider_damage_seen = false;
    events
        .iter()
        .filter(|event| match event.kind.as_str() {
            "SpellManaPaid" | "SpellCast" | "SpellResolved" | "CardMoved" => {
                let is_goliath = event
                    .canonical
                    .contains(&format!("card: ObjectId({})", goliath.0));
                let is_skyknight = event
                    .canonical
                    .contains(&format!("card: ObjectId({})", skyknight.0));
                let moved_to_battlefield =
                    event.kind == "CardMoved" && event.canonical.contains("to: Battlefield");
                if is_goliath && moved_to_battlefield {
                    goliath_entered = true;
                }
                (is_goliath || is_skyknight)
                    && (event.kind != "CardMoved"
                        || moved_to_battlefield
                        || is_skyknight
                            && challenge_block_seen
                            && event.canonical.contains("to: Graveyard"))
            }
            "AttackersDeclared" if goliath_entered && !challenge_attack_seen => {
                let is_challenge = event.canonical.contains("player: PlayerId(1)")
                    && event
                        .canonical
                        .contains(&format!("ObjectId({})", skyknight.0));
                challenge_attack_seen = is_challenge;
                is_challenge
            }
            "AttackersDeclared" if challenge_block_seen && !spider_attack_seen => {
                let is_spider_attack = event.canonical.contains("player: PlayerId(0)")
                    && event
                        .canonical
                        .contains(&format!("ObjectId({})", goliath.0));
                spider_attack_seen = is_spider_attack;
                is_spider_attack
            }
            "AttackersDeclared" => false,
            "BlockersDeclared" if challenge_attack_seen && !challenge_block_seen => {
                let is_block = event.canonical.contains("player: PlayerId(0)")
                    && event
                        .canonical
                        .contains(&format!("ObjectId({})", goliath.0))
                    && event
                        .canonical
                        .contains(&format!("ObjectId({})", skyknight.0));
                challenge_block_seen = is_block;
                is_block
            }
            "BlockersDeclared" => false,
            "DamageDealtToPermanent" => {
                challenge_block_seen
                    && ((event
                        .canonical
                        .contains(&format!("source: ObjectId({})", goliath.0))
                        && event
                            .canonical
                            .contains(&format!("permanent: ObjectId({})", skyknight.0)))
                        || (event
                            .canonical
                            .contains(&format!("source: ObjectId({})", skyknight.0))
                            && event
                                .canonical
                                .contains(&format!("permanent: ObjectId({})", goliath.0))))
            }
            "DamageDealtToPlayer" if spider_attack_seen && !spider_damage_seen => {
                let is_spider_damage = event
                    .canonical
                    .contains(&format!("source: ObjectId({})", goliath.0))
                    && event.canonical.contains("player: PlayerId(1)");
                spider_damage_seen = is_spider_damage;
                is_spider_damage
            }
            "DamageDealtToPlayer" => false,
            "GameEnded" => true,
            _ => false,
        })
        .cloned()
        .collect()
}

fn event_digest(events: &[GameEvent]) -> String {
    use std::fmt::Write as _;
    let mut data = String::new();
    for event in events {
        let _ = writeln!(data, "{event:?}");
    }
    let hash = data
        .as_bytes()
        .iter()
        .fold(0xcbf29ce484222325_u64, |hash, byte| {
            (hash ^ u64::from(*byte)).wrapping_mul(0x100000001b3)
        });
    format!("fnv1a64:{hash:016x}")
}

fn card_status(card: &CatalogCard) -> Value {
    let definition_id = match card.semantic_status {
        CardSemanticStatus::ExecutableCompatibilitySlice { definition_id } => Some(definition_id),
        CardSemanticStatus::CatalogOnly { .. } => None,
    };
    let passed = definition_id == Some("RAV-GOLIATH-SPIDER");
    json!({
        "collector_number": card.collector_number,
        "name": card.name,
        "definition_id": definition_id,
        "engine_status": match card.semantic_status { CardSemanticStatus::ExecutableCompatibilitySlice { .. } => "executable_compatibility_slice", CardSemanticStatus::CatalogOnly { .. } => "catalog_only" },
        "trace_status": if passed { "passed" } else { "not_qualified" },
        "credited": false,
        "qualification": if passed { "pilot_trace_passed; independent reviewer approval pending" } else { "not_evaluated" }
    })
}

fn candidate_kind() -> &'static str {
    if std::env::args().any(|arg| arg == "--candidate") {
        "candidate"
    } else {
        "reference"
    }
}
fn qualification_status() -> &'static str {
    if candidate_kind() == "candidate" {
        "candidate_pilot_pass; independent reviewer approval remains"
    } else {
        "reference_pilot_pass; independent review and candidate trial remain"
    }
}
fn output_root() -> Option<std::path::PathBuf> {
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        if arg == "--output-root" {
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

fn flattened_use_paths(item: &syn::ItemUse) -> Option<Vec<Vec<String>>> {
    if item.leading_colon.is_some() {
        return None;
    }
    let mut paths = Vec::new();
    flatten_use_tree(&[], &item.tree, &mut paths).then_some(paths)
}
fn flatten_use_tree(prefix: &[String], tree: &UseTree, paths: &mut Vec<Vec<String>>) -> bool {
    match tree {
        UseTree::Path(path) => {
            let mut next = prefix.to_vec();
            next.push(path.ident.to_string());
            flatten_use_tree(&next, &path.tree, paths)
        }
        UseTree::Name(name) => {
            let mut path = prefix.to_vec();
            path.push(name.ident.to_string());
            paths.push(path);
            true
        }
        UseTree::Group(group) => group
            .items
            .iter()
            .all(|item| flatten_use_tree(prefix, item, paths)),
        _ => false,
    }
}
fn is_doc_attribute(attribute: &syn::Attribute) -> bool {
    attribute.path().is_ident("doc") && matches!(attribute.meta, Meta::NameValue(_))
}
fn type_is(ty: &Type, expected: &[&str]) -> bool {
    matches!(ty, Type::Path(path) if path.qself.is_none() && path_is(&path.path, expected))
}
fn path_is(path: &syn::Path, expected: &[&str]) -> bool {
    path.leading_colon.is_none()
        && path.segments.len() == expected.len()
        && path
            .segments
            .iter()
            .zip(expected)
            .all(|(segment, expected)| {
                segment.ident == *expected && matches!(&segment.arguments, PathArguments::None)
            })
}
fn literal_fields(
    value: &syn::ExprStruct,
) -> Result<std::collections::BTreeMap<String, &Expr>, String> {
    let mut fields = std::collections::BTreeMap::new();
    for field in &value.fields {
        let syn::Member::Named(name) = &field.member else {
            return Err("Goliath Spider fields must be named".to_owned());
        };
        if field.colon_token.is_none() || fields.insert(name.to_string(), &field.expr).is_some() {
            return Err("Goliath Spider fields must be explicit and unique".to_owned());
        }
    }
    Ok(fields)
}
fn check_string(expr: &Expr, expected: &str) -> Result<(), String> {
    if matches!(expr, Expr::Lit(value) if value.attrs.is_empty() && matches!(&value.lit, Lit::Str(lit) if lit.value() == expected))
    {
        Ok(())
    } else {
        Err(format!("Goliath Spider string field must be {expected:?}"))
    }
}
fn check_bool(expr: &Expr, expected: bool) -> Result<(), String> {
    if matches!(expr, Expr::Lit(value) if value.attrs.is_empty() && matches!(&value.lit, Lit::Bool(lit) if lit.value == expected))
    {
        Ok(())
    } else {
        Err("Goliath Spider boolean field differs".to_owned())
    }
}
fn check_path(expr: &Expr, expected: &[&str]) -> bool {
    matches!(expr, Expr::Path(path) if path.attrs.is_empty() && path.qself.is_none() && path_is(&path.path, expected))
}
fn call_expr<'a>(expr: &'a Expr, path: &[&str]) -> Option<Vec<&'a Expr>> {
    match expr {
        Expr::Call(call) if call.attrs.is_empty() && check_path(&call.func, path) => {
            Some(call.args.iter().collect())
        }
        _ => None,
    }
}
fn array_items(expr: &Expr) -> Option<Vec<&Expr>> {
    match expr {
        Expr::Array(array) if array.attrs.is_empty() => Some(array.elems.iter().collect()),
        _ => None,
    }
}
fn check_single_set_path(expr: &Expr, expected: &[&str]) -> Result<(), String> {
    let args = call_expr(expr, &["BTreeSet", "from"])
        .ok_or_else(|| "Goliath Spider set must use BTreeSet::from".to_owned())?;
    let items = if args.len() == 1 {
        array_items(args[0])
    } else {
        None
    };
    if items.is_some_and(|items| items.len() == 1 && check_path(items[0], expected)) {
        Ok(())
    } else {
        Err("Goliath Spider set value differs".to_owned())
    }
}
fn check_empty_set(expr: &Expr) -> Result<(), String> {
    if call_expr(expr, &["BTreeSet", "new"]).is_some_and(|args| args.is_empty()) {
        Ok(())
    } else {
        Err("Goliath Spider mana colors must be empty".to_owned())
    }
}
fn check_cost(expr: &Expr) -> Result<(), String> {
    let args = call_expr(expr, &["ManaCost", "with_colors"])
        .ok_or_else(|| "Goliath Spider cost must use ManaCost::with_colors".to_owned())?;
    if args.len() != 2
        || !matches!(args[0], Expr::Lit(value) if value.attrs.is_empty() && matches!(&value.lit, Lit::Int(lit) if lit.base10_parse::<u8>().ok() == Some(6)))
    {
        return Err("Goliath Spider generic cost must be six".to_owned());
    }
    let colors = array_items(args[1]);
    if colors.is_some_and(|colors| {
        colors.len() == 2
            && colors
                .iter()
                .all(|color| check_path(color, &["Color", "Green"]))
    }) {
        Ok(())
    } else {
        Err("Goliath Spider colored cost must be two green".to_owned())
    }
}
fn check_rules(expr: &Expr) -> Result<(), String> {
    let Expr::Reference(reference) = expr else {
        return Err("Goliath Spider rules must be a static slice".to_owned());
    };
    let items = array_items(&reference.expr)
        .ok_or_else(|| "Goliath Spider rules must be a string array".to_owned())?;
    let expected = [
        "full-rules-fidelity",
        "colored-cost-casting",
        "base-characteristics",
        "reach",
    ];
    if reference.attrs.is_empty()
        && reference.mutability.is_none()
        && items.len() == expected.len()
        && items
            .iter()
            .zip(expected)
            .all(|(item, value)| check_string(item, value).is_ok())
    {
        Ok(())
    } else {
        Err("Goliath Spider supported rules differ".to_owned())
    }
}
fn check_some_integer(expr: &Expr, expected: i16) -> Result<(), String> {
    let args = call_expr(expr, &["Some"])
        .ok_or_else(|| "Goliath Spider power and toughness must be literals".to_owned())?;
    if args.len() == 1
        && matches!(args[0], Expr::Lit(value) if value.attrs.is_empty() && matches!(&value.lit, Lit::Int(lit) if lit.base10_parse::<i16>().ok() == Some(expected)))
    {
        Ok(())
    } else {
        Err("Goliath Spider power or toughness differs".to_owned())
    }
}
fn check_keyword_or_empty(expr: &Expr) -> Result<(), String> {
    let Expr::Macro(mac) = expr else {
        return Err("Goliath Spider keyword list must use literal vec!".to_owned());
    };
    let tokens = mac.mac.tokens.to_string().replace(' ', "");
    if mac.attrs.is_empty()
        && mac.mac.path.is_ident("vec")
        && (tokens.is_empty() || tokens == "Keyword::Reach")
    {
        Ok(())
    } else {
        Err("Goliath Spider keyword must be Reach or omitted for semantic rejection".to_owned())
    }
}
fn check_empty_vec(expr: &Expr) -> Result<(), String> {
    match expr {
        Expr::Macro(mac)
            if mac.attrs.is_empty()
                && mac.mac.path.is_ident("vec")
                && mac.mac.tokens.is_empty() =>
        {
            Ok(())
        }
        _ => Err("Goliath Spider effects must be empty".to_owned()),
    }
}
