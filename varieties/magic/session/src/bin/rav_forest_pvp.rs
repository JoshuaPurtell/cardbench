//! Full-game RAV Forest trace authority.
//!
//! The trace uses a typed Forest as the explicit green source for a legal
//! creature spell, then completes a normal two-player constructed game.

use cardbench_magic_engine::{
    BasicLandType, CardType, CastRequest, Color, DeckEntry, DeckList, GameEvent, GameView,
    ManaCost, ManaPaymentSelection, ObjectId, PlayerId, PolicyAction, Step, Zone,
};
use cardbench_magic_policies::{CatalogPolicyProfile, CodePolicy, RavCatalogPolicy};
use cardbench_magic_protocol::ViewerScope;
use cardbench_magic_rav::{
    CardSemanticStatus, CatalogCard, RAV_MAIN_SET_EXPECTED_PRINTING_COUNT,
    RAV_MAIN_SET_EXPECTED_UNIQUE_NAME_COUNT, card_definitions, new_rav_game, rav_main_set_catalog,
};
use cardbench_magic_session::{ProjectedEvent, project_events};
use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::process::ExitCode;
use syn::{Expr, Item, Lit, Meta, Stmt, Type, Visibility};

const TASK_ID: &str = "cardbench/magic/card";
const CARD_ID: &str = "RAV-FOREST";
const CREATURES: [(&str, u8); 2] = [
    ("RAV-ELVISH-SKYSWEEPER", 0),
    ("RAV-ELVES-OF-DEEP-SHADOW", 0),
];
const SOURCE_PATH: &str = "varieties/magic/sets/ravnica_city_of_guilds/src/cards/forest.rs";
const MAX_TRACE_SEED: u64 = 32;

#[path = "../../../sets/ravnica_city_of_guilds/src/cards/forest.rs"]
mod candidate_forest;

fn main() -> ExitCode {
    match run_authority() {
        Ok(report) => {
            if let Some(root) = output_root()
                && let Err(error) = write_outputs(&root, &report)
            {
                eprintln!("could not write Forest PvP result: {error}");
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
            eprintln!("Forest PvP authority failed: {error}");
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
            RAV_MAIN_SET_EXPECTED_PRINTING_COUNT
        ));
    }

    let (seed, captured) = find_trace()?;
    let trace = validate_trace(&captured, seed)?;
    let replay = run_match(seed)?;
    if captured.events != replay.events
        || captured.terminal != replay.terminal
        || captured.winner != replay.winner
        || captured.forest != replay.forest
        || captured.creature != replay.creature
        || captured.creature_pt != replay.creature_pt
        || captured.forest_type_valid != replay.forest_type_valid
    {
        return Err("Forest full-game replay was not deterministic".to_owned());
    }

    let breakdown: Vec<Value> = catalog.iter().map(card_status).collect();
    let cards_passed = breakdown
        .iter()
        .filter(|card| card["trace_status"] == "passed")
        .count();
    let mut wrong_color = candidate_forest::definition();
    wrong_color.mana_colors = BTreeSet::from([Color::Red]);
    let mut wrong_type = candidate_forest::basic_land_type_binding();
    wrong_type.land_type = BasicLandType::Mountain;
    let mutation_detected =
        !forest_identity_valid(&wrong_color, &candidate_forest::basic_land_type_binding())
            && !forest_identity_valid(&candidate_forest::definition(), &wrong_type);
    Ok(json!({
        "schema_version": "cardbench.magic.card-pvp.check.v1",
        "task_id": TASK_ID,
        "expansion": "RAV",
        "card": {"collector_number": 303, "printings": 4, "definition_id": CARD_ID, "name": "Forest"},
        "candidate": {"kind": candidate_kind(), "card_definition": CARD_ID},
        "candidate_kind": candidate_kind(),
        "viewer_scope": format!("{:?}", ViewerScope::Judge),
        "rules_authority": {
            "comprehensive_rules": "2026-08-07",
            "checked_sections": [
                "100.2a and 100.2c: constructed deck composition and basic-land quantities",
                "305.1 and 305.6: land play and basic land types",
                "106.1 and 106.4: mana production and spell payment",
                "117: priority and turn-based actions",
                "601.2: casting and paying spell costs",
                "605.1a and 605.3: intrinsic basic-land mana abilities",
                "302.1-302.2: creature spell resolution and battlefield entry",
            ],
            "source_contract": "RAV Forest identity, typed Forest land type, and intrinsic green mana",
            "authority_cutoff": "2026-09-23"
        },
        "passed": cards_passed == 1,
        "pilot_card_passed": cards_passed == 1,
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
            "forest_type_and_mana_color_mutation_detected": mutation_detected,
            "forest_intrinsic_activation_required": trace["passed"].as_bool() == Some(true),
            "sealed_mutation_authority": false
        },
        "traces": [trace],
        "per_card_breakdown": breakdown
    }))
}

fn validate_candidate_source() -> Result<(), String> {
    let source = std::fs::read_to_string(SOURCE_PATH)
        .map_err(|error| format!("could not read Forest source: {error}"))?;
    let file = syn::parse_file(&source)
        .map_err(|_| "Forest source is not valid declarative Rust".to_owned())?;
    if !file.attrs.iter().all(is_doc_attribute) || file.items.len() != 4 {
        return Err("Forest source has an unsupported module shape".to_owned());
    }
    let mut functions = BTreeSet::new();
    let mut imports = BTreeSet::new();
    for item in &file.items {
        match item {
            Item::Use(item)
                if matches!(&item.vis, Visibility::Inherited) && item.attrs.is_empty() =>
            {
                if !collect_use_paths(&mut Vec::new(), &item.tree, &mut imports) {
                    return Err("Forest source has an unsupported import".to_owned());
                }
            }
            Item::Fn(function) => {
                let name = function.sig.ident.to_string();
                if !functions.insert(name.clone()) || !function_is_literal_only(function, &name) {
                    return Err(format!(
                        "Forest function `{name}` must be a crate-private pure card literal"
                    ));
                }
            }
            _ => return Err("Forest source contains executable top-level code".to_owned()),
        }
    }
    let expected_imports: BTreeSet<Vec<String>> = [
        vec!["std", "collections", "BTreeSet"],
        vec!["cardbench_magic_engine", "BasicLandType"],
        vec!["cardbench_magic_engine", "BasicLandTypeBinding"],
        vec!["cardbench_magic_engine", "CardDefinition"],
        vec!["cardbench_magic_engine", "CardType"],
        vec!["cardbench_magic_engine", "Color"],
        vec!["cardbench_magic_engine", "ManaCost"],
    ]
    .into_iter()
    .map(|path| path.into_iter().map(str::to_owned).collect())
    .collect();
    if imports != expected_imports
        || functions
            != BTreeSet::from([
                "definition".to_owned(),
                "basic_land_type_binding".to_owned(),
            ])
    {
        return Err("Forest source is missing its two declarative functions".to_owned());
    }
    Ok(())
}

fn collect_use_paths(
    prefix: &mut Vec<String>,
    tree: &syn::UseTree,
    output: &mut BTreeSet<Vec<String>>,
) -> bool {
    match tree {
        syn::UseTree::Path(path) => {
            prefix.push(path.ident.to_string());
            let valid = collect_use_paths(prefix, &path.tree, output);
            prefix.pop();
            valid
        }
        syn::UseTree::Name(name) => {
            prefix.push(name.ident.to_string());
            output.insert(prefix.clone());
            prefix.pop();
            true
        }
        syn::UseTree::Group(group) => group
            .items
            .iter()
            .all(|item| collect_use_paths(prefix, item, output)),
        syn::UseTree::Rename(_) | syn::UseTree::Glob(_) => false,
    }
}

fn function_is_literal_only(function: &syn::ItemFn, expected_name: &str) -> bool {
    let return_type = match expected_name {
        "definition" => "CardDefinition",
        "basic_land_type_binding" => "BasicLandTypeBinding",
        _ => return false,
    };
    let attrs_ok = function.attrs.iter().all(|attribute| {
        is_doc_attribute(attribute)
            || attribute.path().is_ident("must_use") && matches!(attribute.meta, Meta::Path(_))
    });
    let crate_visibility = matches!(
        &function.vis,
        Visibility::Restricted(visibility) if visibility.path.is_ident("crate")
    );
    if !attrs_ok
        || !crate_visibility
        || function.sig.ident != expected_name
        || function.sig.constness.is_some()
        || function.sig.asyncness.is_some()
        || function.sig.unsafety.is_some()
        || function.sig.abi.is_some()
        || !function.sig.generics.params.is_empty()
        || function.sig.generics.where_clause.is_some()
        || !function.sig.inputs.is_empty()
        || function.sig.variadic.is_some()
        || !matches!(&function.sig.output, syn::ReturnType::Type(_, ty) if type_name(ty).as_deref() == Some(return_type))
        || function.block.stmts.len() != 1
    {
        return false;
    }
    let Stmt::Expr(Expr::Struct(literal), None) = &function.block.stmts[0] else {
        return false;
    };
    let expected_struct = if expected_name == "definition" {
        "CardDefinition"
    } else {
        "BasicLandTypeBinding"
    };
    if !path_is(
        literal.path.segments.iter().map(|s| s.ident.to_string()),
        expected_struct,
    ) || literal.rest.is_some()
        || !literal.attrs.is_empty()
    {
        return false;
    }
    let field_names: BTreeSet<String> = literal
        .fields
        .iter()
        .filter_map(|field| field.member.to_token_name())
        .collect();
    let expected_fields: BTreeSet<String> = if expected_name == "definition" {
        [
            "id",
            "name",
            "set_code",
            "mana_cost",
            "colors",
            "mana_colors",
            "card_types",
            "is_basic_land",
            "supported_rules",
            "power",
            "toughness",
            "keywords",
            "effects",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect()
    } else {
        ["card_definition", "land_type"]
            .into_iter()
            .map(str::to_owned)
            .collect()
    };
    if field_names != expected_fields {
        return false;
    }
    literal
        .fields
        .iter()
        .all(|field| safe_expression(&field.expr))
}

trait MemberName {
    fn to_token_name(&self) -> Option<String>;
}
impl MemberName for syn::Member {
    fn to_token_name(&self) -> Option<String> {
        match self {
            syn::Member::Named(ident) => Some(ident.to_string()),
            syn::Member::Unnamed(_) => None,
        }
    }
}

fn safe_expression(expression: &Expr) -> bool {
    match expression {
        Expr::Lit(literal) => matches!(literal.lit, Lit::Str(_) | Lit::Int(_) | Lit::Bool(_)),
        Expr::Path(path) => {
            let segments: Vec<String> = path
                .path
                .segments
                .iter()
                .map(|segment| segment.ident.to_string())
                .collect();
            [
                vec!["None".to_owned()],
                vec!["Color".to_owned(), "Green".to_owned()],
                vec!["Color".to_owned(), "Red".to_owned()],
                vec!["CardType".to_owned(), "Land".to_owned()],
                vec!["BasicLandType".to_owned(), "Forest".to_owned()],
                vec!["BasicLandType".to_owned(), "Mountain".to_owned()],
            ]
            .contains(&segments)
        }
        Expr::Call(call) => {
            let Expr::Path(path) = &*call.func else {
                return false;
            };
            let segments: Vec<String> = path
                .path
                .segments
                .iter()
                .map(|segment| segment.ident.to_string())
                .collect();
            let allowed = [
                vec!["ManaCost".to_owned(), "new".to_owned()],
                vec!["BTreeSet".to_owned(), "new".to_owned()],
                vec!["BTreeSet".to_owned(), "from".to_owned()],
            ]
            .contains(&segments);
            allowed && call.args.iter().all(safe_expression)
        }
        Expr::Array(array) => array.elems.iter().all(safe_expression),
        Expr::Reference(reference) => safe_expression(&reference.expr),
        Expr::Macro(mac) => mac.mac.path.is_ident("vec") && mac.mac.tokens.is_empty(),
        _ => false,
    }
}

fn is_doc_attribute(attribute: &syn::Attribute) -> bool {
    attribute.path().is_ident("doc")
        && matches!(&attribute.meta, Meta::NameValue(value) if matches!(&value.value, Expr::Lit(lit) if matches!(lit.lit, Lit::Str(_))))
}

fn type_name(ty: &Type) -> Option<String> {
    let Type::Path(path) = ty else {
        return None;
    };
    path.path
        .segments
        .last()
        .map(|segment| segment.ident.to_string())
}

fn path_is<'a>(segments: impl Iterator<Item = String>, expected: &str) -> bool {
    let found: Vec<String> = segments.collect();
    found.len() == 1 && found[0] == expected
}

fn forest_identity_valid(
    definition: &cardbench_magic_engine::CardDefinition,
    binding: &cardbench_magic_engine::BasicLandTypeBinding,
) -> bool {
    definition.id == CARD_ID
        && definition.name == "Forest"
        && definition.set_code == "RAV"
        && definition.mana_cost == ManaCost::new(0)
        && definition.colors.is_empty()
        && definition.mana_colors == BTreeSet::from([Color::Green])
        && definition.card_types == BTreeSet::from([CardType::Land])
        && definition.is_basic_land
        && definition.power.is_none()
        && definition.toughness.is_none()
        && definition.keywords.is_empty()
        && definition.effects.is_empty()
        && definition.supported_rules
            == [
                "full-rules-fidelity",
                "basic-land-type-line",
                "intrinsic-single-color-mana-ability",
                "basic-land-deck-construction",
            ]
        && binding.card_definition == CARD_ID
        && binding.land_type == BasicLandType::Forest
}

fn validate_candidate_definition() -> Result<(), String> {
    let candidate = candidate_forest::definition();
    let binding = candidate_forest::basic_land_type_binding();
    let registered = card_definitions()
        .into_iter()
        .find(|card| card.id == CARD_ID)
        .ok_or("Forest is absent from the RAV definition registry")?;
    if candidate != registered || !forest_identity_valid(&candidate, &binding) {
        return Err(
            "candidate Forest definition or intrinsic mana binding differs from its RAV identity"
                .to_owned(),
        );
    }
    Ok(())
}

#[derive(Clone)]
struct CapturedMatch {
    events: Vec<GameEvent>,
    terminal: String,
    winner: Option<usize>,
    turns: u32,
    accepted_moves: u32,
    forest: Option<ObjectId>,
    creature: Option<ObjectId>,
    creature_pt: Option<(i32, i32)>,
    forest_type_valid: bool,
}

fn find_trace() -> Result<(u64, CapturedMatch), String> {
    if let Ok(value) = std::env::var("CARDBENCH_FOREST_SEED") {
        let seed = value
            .parse::<u64>()
            .map_err(|_| "CARDBENCH_FOREST_SEED must be an unsigned integer".to_owned())?;
        let captured = run_match(seed)?;
        validate_trace(&captured, seed)?;
        return Ok((seed, captured));
    }
    let mut last_error = String::new();
    for seed in 0..=MAX_TRACE_SEED {
        match run_match(seed).and_then(|captured| validate_trace(&captured, seed).map(|_| captured))
        {
            Ok(captured) => return Ok((seed, captured)),
            Err(error) => last_error = error,
        }
    }
    Err(format!(
        "no clean Forest full-game trace found for seeds 0..={MAX_TRACE_SEED}: {last_error}"
    ))
}

fn run_match(seed: u64) -> Result<CapturedMatch, String> {
    let forest_deck = DeckList {
        mainboard: vec![
            DeckEntry {
                card: CREATURES[0].0.to_owned(),
                count: 4,
            },
            DeckEntry {
                card: CREATURES[1].0.to_owned(),
                count: 4,
            },
            DeckEntry {
                card: CARD_ID.to_owned(),
                count: 52,
            },
        ],
        sideboard: vec![],
    };
    let passive_deck = DeckList {
        mainboard: vec![DeckEntry {
            card: "RAV-PLAINS".to_owned(),
            count: 60,
        }],
        sideboard: vec![],
    };
    for (label, deck) in [
        ("Forest pilot", &forest_deck),
        ("passive opponent", &passive_deck),
    ] {
        let count: usize = deck
            .mainboard
            .iter()
            .map(|entry| usize::from(entry.count))
            .sum();
        if count != 60 {
            return Err(format!("{label} deck has {count} cards; expected 60"));
        }
    }
    let mut game = new_rav_game(2).map_err(|error| error.to_string())?;
    game.set_shuffle_seed(seed)
        .map_err(|error| error.to_string())?;
    game.load_deck_into_library(PlayerId(0), &forest_deck)
        .map_err(|error| error.to_string())?;
    game.load_deck_into_library(PlayerId(1), &passive_deck)
        .map_err(|error| error.to_string())?;
    game.draw_opening_hand(PlayerId(0), 7)
        .map_err(|error| error.to_string())?;
    game.draw_opening_hand(PlayerId(1), 7)
        .map_err(|error| error.to_string())?;
    game.begin_game().map_err(|error| error.to_string())?;

    let mut policies: [Box<dyn CodePolicy>; 2] = [
        Box::new(ForestPolicy::new(PlayerId(0))),
        Box::new(PassPolicy),
    ];
    let mut accepted = 0_u32;
    while !game.is_game_over() {
        if game.turn > 50 || accepted >= 3000 {
            let view = game
                .view_for_player(PlayerId(0))
                .map_err(|error| error.to_string())?;
            let casts = game.event_log.iter().filter(|event| matches!(event, GameEvent::SpellCast { player: PlayerId(0), card } if game.card_definition(*card).is_ok_and(|definition| CREATURES.iter().any(|(id, _)| *id == definition.id)))).count();
            let damage: i32 = game
                .event_log
                .iter()
                .filter_map(|event| match event {
                    GameEvent::DamageDealtToPlayer {
                        player: PlayerId(1),
                        amount,
                        ..
                    } => Some(*amount),
                    _ => None,
                })
                .sum();
            return Err(format!(
                "Forest game exceeded its turn/action bound: turn={}, accepted={}, hand={}, battlefield={}, casts={}, opponent_life={:?}, damage={damage}",
                game.turn,
                accepted,
                view.hand.len(),
                view.own_battlefield.len(),
                casts,
                view.opponent_life
            ));
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
        game.submit_policy_move(player, policy.id(), action)
            .map_err(|error| format!("policy move rejected: {error}"))?;
        accepted += 1;
        game.validate_invariants()
            .map_err(|error| format!("post-move invariant failure: {error}"))?;
    }
    let events = game.event_log.clone();
    let forest = events.iter().find_map(|event| match event {
        GameEvent::CardMoved {
            card,
            to: Zone::Battlefield,
        } if game
            .card_definition(*card)
            .is_ok_and(|definition| definition.id == CARD_ID) =>
        {
            Some(*card)
        }
        _ => None,
    });
    let creature = events.iter().find_map(|event| match event {
        GameEvent::SpellCast {
            player: PlayerId(0),
            card,
        } if game
            .card_definition(*card)
            .is_ok_and(|definition| CREATURES.iter().any(|(id, _)| *id == definition.id)) =>
        {
            Some(*card)
        }
        _ => None,
    });
    Ok(CapturedMatch {
        events,
        terminal: game.winner().map_or_else(
            || "Draw".to_owned(),
            |winner| format!("Winner(PlayerId({}))", winner.0),
        ),
        winner: game.winner().map(|winner| winner.0),
        turns: game.turn,
        accepted_moves: accepted,
        forest,
        creature,
        creature_pt: creature.and_then(|card| {
            game.characteristics(card).ok().and_then(|characteristics| {
                Some((
                    characteristics.power? as i32,
                    characteristics.toughness? as i32,
                ))
            })
        }),
        forest_type_valid: forest.is_some_and(|forest| {
            game.view_for_player(PlayerId(0)).is_ok_and(|view| {
                view.own_battlefield.iter().any(|card| {
                    card.id == forest
                        && card.basic_land_type == Some(BasicLandType::Forest)
                        && card.mana_colors.contains(&Color::Green)
                })
            })
        }),
    })
}

struct ForestPolicy {
    player: PlayerId,
    inner: RavCatalogPolicy,
}
struct PassPolicy;

impl ForestPolicy {
    fn new(player: PlayerId) -> Self {
        Self {
            player,
            inner: RavCatalogPolicy::new(player, CatalogPolicyProfile::Curve),
        }
    }
}

impl CodePolicy for ForestPolicy {
    fn id(&self) -> &'static str {
        "rav.forest-green-mana.v1"
    }
    fn propose_move(&mut self, view: &GameView) -> PolicyAction {
        if view.step == Step::DeclareAttackers
            && view.active_player == self.player
            && view.player == self.player
            && !view.attackers_declared
        {
            return PolicyAction::DeclareAttackers {
                attackers: view
                    .own_battlefield
                    .iter()
                    .filter(|card| card.can_attack)
                    .map(|card| card.id)
                    .collect(),
            };
        }
        if view.player == self.player
            && view.decision_player == self.player
            && view.active_player == self.player
            && view.priority == self.player
            && view.stack_depth == 0
            && matches!(view.step, Step::PrecombatMain | Step::PostcombatMain)
        {
            if view.lands_played == 0
                && let Some(forest) = view
                    .hand
                    .iter()
                    .find(|card| card.definition == Some(CARD_ID))
            {
                return PolicyAction::PlayLand { card: forest.id };
            }
            let selected = CREATURES.iter().find_map(|(definition, generic)| {
                view.hand
                    .iter()
                    .find(|card| card.definition == Some(*definition))
                    .map(|card| (card, *generic))
            });
            if let Some((creature, generic)) = selected {
                if view.mana_pool.amount(Color::Green) >= 1 && view.mana_pool.total() >= generic + 1
                {
                    return PolicyAction::CastWithPayment {
                        request: CastRequest {
                            card: creature.id,
                            targets: vec![],
                            convoke: vec![],
                            payment_mana_abilities: vec![],
                        },
                        chosen_x: None,
                        mana_selection: ManaPaymentSelection {
                            generic: vec![Color::Green; usize::from(generic)],
                            hybrid: vec![],
                        },
                    };
                }
                if let Some(forest) = view
                    .own_battlefield
                    .iter()
                    .find(|card| card.definition == Some(CARD_ID) && !card.tapped)
                {
                    return PolicyAction::ActivateManaAbility {
                        land: forest.id,
                        color: Color::Green,
                    };
                }
            }
        }
        self.inner.propose_move(view)
    }
}

impl CodePolicy for PassPolicy {
    fn id(&self) -> &'static str {
        "rav.forest-pass-opponent.v1"
    }
    fn propose_move(&mut self, view: &GameView) -> PolicyAction {
        match view.step {
            Step::DeclareAttackers
                if view.active_player == view.player && !view.attackers_declared =>
            {
                PolicyAction::DeclareAttackers { attackers: vec![] }
            }
            Step::DeclareBlockers
                if view.active_player != view.player && !view.blockers_declared =>
            {
                PolicyAction::DeclareBlockers {
                    assignments: vec![],
                }
            }
            _ => PolicyAction::PassPriority,
        }
    }
}

fn validate_trace(captured: &CapturedMatch, seed: u64) -> Result<Value, String> {
    if captured.terminal != "Winner(PlayerId(0))" || captured.winner != Some(0) {
        return Err("Forest match did not reach a seat-zero terminal win".to_owned());
    }
    let forest = captured
        .forest
        .ok_or("Forest did not enter the battlefield")?;
    let creature = captured.creature.ok_or("green creature was not cast")?;
    let creature_pt = captured
        .creature_pt
        .ok_or("resolved green creature characteristics missing")?;
    let hand = captured.events.iter().position(|event| matches!(event, GameEvent::CardMoved { card, to: Zone::Hand } if *card == forest)).ok_or("Forest opening-hand receipt missing")?;
    let moved = captured.events.iter().position(|event| matches!(event, GameEvent::CardMoved { card, to: Zone::Battlefield } if *card == forest)).ok_or("Forest battlefield entry receipt missing")?;
    if hand >= moved {
        return Err("Forest did not move from its opening hand to the battlefield".to_owned());
    }
    if !captured.forest_type_valid {
        return Err("live Forest lacks its typed Forest land type or green mana color".to_owned());
    }
    let paid = captured.events.iter().position(|event| matches!(event, GameEvent::SpellManaPaid { player: PlayerId(0), card, colors } if *card == creature && colors.contains(&Color::Green))).ok_or("Forest-backed green creature payment missing")?;
    let activated = captured.events.iter().enumerate().take(paid).filter_map(|(index, event)| matches!(event, GameEvent::ManaAbilityActivated { player: PlayerId(0), land, color: Color::Green } if *land == forest).then_some(index)).last().ok_or("Forest intrinsic green activation missing before payment")?;
    let produced = captured
        .events
        .iter()
        .enumerate()
        .skip(activated + 1)
        .take(paid.saturating_sub(activated + 1))
        .find_map(|(index, event)| {
            matches!(
                event,
                GameEvent::ManaAdded {
                    player: PlayerId(0),
                    color: Color::Green,
                    amount: 1
                }
            )
            .then_some(index)
        })
        .ok_or("Forest green mana output missing before payment")?;
    let cast = captured.events.iter().position(|event| matches!(event, GameEvent::SpellCast { player: PlayerId(0), card } if *card == creature)).ok_or("green creature SpellCast receipt missing")?;
    let resolved = captured
        .events
        .iter()
        .position(|event| matches!(event, GameEvent::SpellResolved { card } if *card == creature))
        .ok_or("green creature SpellResolved receipt missing")?;
    let entered = captured.events.iter().position(|event| matches!(event, GameEvent::CardMoved { card, to: Zone::Battlefield, .. } if *card == creature)).ok_or("green creature battlefield entry receipt missing")?;
    let projections = project_events(&captured.events);
    if projections[produced].turn != projections[paid].turn
        || !(moved < activated
            && activated + 1 == produced
            && produced < paid
            && paid < cast
            && cast < resolved
            && resolved < entered)
    {
        return Err(
            "Forest play, mana production, spell payment, and entry order differs".to_owned(),
        );
    }
    let combat_damage = captured
        .events
        .iter()
        .enumerate()
        .find_map(|(index, event)| match event {
            GameEvent::DamageDealtToPlayer {
                source,
                player: PlayerId(1),
                amount,
            } if *amount > 0
                && captured.events[..index].iter().any(
                    |prior| matches!(prior, GameEvent::SpellCast { card, .. } if card == source),
                ) =>
            {
                Some(index)
            }
            _ => None,
        })
        .ok_or("Forest pilot did not use a creature in combat")?;
    let terminal = captured
        .events
        .iter()
        .position(|event| {
            matches!(
                event,
                GameEvent::GameEnded {
                    winner: Some(PlayerId(0))
                }
            )
        })
        .ok_or("Forest match terminal event missing")?;
    let ordered = [
        hand,
        moved,
        activated,
        produced,
        paid,
        cast,
        resolved,
        entered,
        combat_damage,
        terminal,
    ];
    let required: BTreeSet<usize> = ordered.into_iter().collect();
    let relevant: Vec<ProjectedEvent> = projections
        .into_iter()
        .filter(|event| required.contains(&(event.sequence as usize)))
        .collect();
    Ok(json!({
        "id": "rav-forest-intrinsic-green-mana",
        "seed": seed,
        "decks": ["rav-forest-creature-pilot-60", "rav-plains-pass-60"],
        "policies": ["rav.forest-green-mana.v1", "rav.forest-pass-opponent.v1"],
        "terminal": captured.terminal,
        "winner_seat": captured.winner,
        "turns": captured.turns,
        "accepted_moves": captured.accepted_moves,
        "rejected_moves": 0,
        "forest_object": forest.0,
        "green_creature_object": creature.0,
        "forest_type": "Forest",
        "mana_produced": "Green",
        "creature_characteristics": {"power": creature_pt.0, "toughness": creature_pt.1},
        "ordered_event_sequence": ordered,
        "events": relevant,
        "passed": true
    }))
}

fn card_status(card: &CatalogCard) -> Value {
    let definition_id = match card.semantic_status {
        CardSemanticStatus::ExecutableCompatibilitySlice { definition_id } => Some(definition_id),
        CardSemanticStatus::CatalogOnly { .. } => None,
    };
    let passed = definition_id == Some(CARD_ID);
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
    if std::env::args().any(|argument| argument == "--candidate") {
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
