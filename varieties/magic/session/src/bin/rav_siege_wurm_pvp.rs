//! Full-game diagnostic authority for RAV Siege Wurm.

use std::collections::BTreeSet;
use std::process::ExitCode;

use cardbench_magic_engine::{
    CardDefinition, CardType, CastRequest, Color, CombatBlock, ConvokeContribution, ConvokePayment,
    DeckEntry, DeckList, Game, GameEvent, GameView, Keyword, ManaCost, ObjectId, PlayerId,
    PolicyAction, Step, Zone,
};
use cardbench_magic_policies::CodePolicy;
use cardbench_magic_protocol::ViewerScope;
use cardbench_magic_rav::{
    CardSemanticStatus, CatalogCard, RAV_MAIN_SET_EXPECTED_PRINTING_COUNT,
    RAV_MAIN_SET_EXPECTED_UNIQUE_NAME_COUNT, card_definitions, rav_main_set_catalog,
};
use cardbench_magic_session::project_events;
use serde_json::{Value, json};
use syn::parse::Parser;
use syn::{Expr, Item, Stmt, UseTree, Visibility};

const TASK_ID: &str = "cardbench/magic/card";
const CARD_ID: &str = "RAV-SIEGE-WURM";
const BLOCKER_ID: &str = "RAV-WATCHWOLF";
const SOURCE_PATH: &str = "varieties/magic/sets/ravnica_city_of_guilds/src/cards/siege_wurm.rs";
const MAX_TRACE_SEED: u64 = 64;

#[path = "../../../sets/ravnica_city_of_guilds/src/cards/siege_wurm.rs"]
mod candidate_siege_wurm;

fn main() -> ExitCode {
    match run_authority() {
        Ok(report) => {
            if let Some(root) = output_root()
                && let Err(error) = write_outputs(&root, &report)
            {
                eprintln!("could not write Siege Wurm result: {error}");
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
            eprintln!("Siege Wurm PvP authority failed: {error}");
            ExitCode::FAILURE
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Mutation {
    None,
    NoConvoke,
    NoTrample,
    HigherCost,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Captured {
    events: Vec<GameEvent>,
    winner: Option<usize>,
    turns: u32,
    accepted: u32,
    rejected: u32,
    spell: Option<ObjectId>,
    spell_zone: Option<Zone>,
    paid_colors: Vec<Color>,
    convokers: Vec<(ObjectId, Option<Color>, bool, &'static str, usize)>,
    development_spells: Vec<(ObjectId, &'static str)>,
    candidate_power: Option<i32>,
    candidate_toughness: Option<i32>,
    candidate_has_convoke: bool,
    candidate_has_trample: bool,
    blocker: Option<ObjectId>,
    blocker_cast: Option<ObjectId>,
    blocker_resolved: bool,
    blocker_zone: Option<Zone>,
    blocker_graveyard_move_observed: bool,
    blocker_damage: Vec<(ObjectId, i32)>,
    player_damage: Vec<i32>,
    blocker_sbas: Vec<ObjectId>,
    blocker_assigned: bool,
    resolved: bool,
    opponent_life: Option<i64>,
}

fn run_authority() -> Result<Value, String> {
    validate_candidate_source()?;
    validate_candidate_registry()?;
    let printings = rav_main_set_catalog();
    let mut names = BTreeSet::new();
    let catalog: Vec<_> = printings
        .iter()
        .copied()
        .filter(|card| names.insert(card.name))
        .collect();
    if catalog.len() != RAV_MAIN_SET_EXPECTED_UNIQUE_NAME_COUNT
        || printings.len() != RAV_MAIN_SET_EXPECTED_PRINTING_COUNT
        || catalog.len() != 291
        || printings.len() != 306
    {
        return Err("RAV catalog denominator differs from the shared source contract".to_owned());
    }
    let (seed, reference) = find_trace()?;
    let trace = validate_trace(&reference, seed)?;
    if !same_match(&reference, &run_match(seed, Mutation::None)?) {
        return Err("Siege Wurm shuffled full-game replay was not deterministic".to_owned());
    }
    let mutations = json!({
        "convoke_removed_mutation_detected_in_game": mutation_detected(seed, Mutation::NoConvoke)?,
        "trample_removed_mutation_detected_in_combat": mutation_detected(seed, Mutation::NoTrample)?,
        "printed_cost_changed_mutation_detected": mutation_detected(seed, Mutation::HigherCost)?,
        "sealed_mutation_authority": false
    });
    for key in [
        "convoke_removed_mutation_detected_in_game",
        "trample_removed_mutation_detected_in_combat",
        "printed_cost_changed_mutation_detected",
    ] {
        if mutations[key] != true {
            return Err(format!("Siege Wurm gameplay mutant escaped: {key}"));
        }
    }
    let breakdown: Vec<_> = catalog.iter().map(card_status).collect();
    Ok(json!({
        "schema_version": "cardbench.magic.card-pvp.check.v1",
        "task_id": TASK_ID,
        "expansion": "RAV",
        "card": {"collector_number": 183, "printings": 1, "definition_id": CARD_ID, "name": "Siege Wurm"},
        "candidate": {"kind": candidate_kind(), "card_definition": CARD_ID},
        "candidate_kind": candidate_kind(),
        "viewer_scope": format!("{:?}", ViewerScope::Judge),
        "rules_authority": {
            "comprehensive_rules": "2026-08-07",
            "checked_sections": [
                "601.2: casting a creature spell and paying its costs",
                "702.50: Convoke",
                "702.19: Trample",
                "508.1: declaring attackers",
                "509.1: declaring blockers",
                "510.1: combat damage assignment",
                "704.5g: lethal-damage state-based actions"
            ],
            "source_contract": "RAV Siege Wurm #183 is a green 5/5 creature costing {5}{G}{G} with Convoke and Trample.",
            "authority_cutoff": "2026-09-24",
            "no_provider_trial": true
        },
        "passed": true,
        "pilot_card_passed": true,
        "deterministic_replay": true,
        "cards_passed": 1,
        "cards_credited": 0,
        "cards_total": catalog.len(),
        "printings_total": printings.len(),
        "coverage_complete": false,
        "full_reward": false,
        "harbor_reward": 0.0,
        "pilot_reward": 0.0,
        "qualification_status": "diagnostic_only; independent review and candidate trial pending",
        "mutation_sensitivity": mutations,
        "traces": [trace],
        "per_card_breakdown": breakdown
    }))
}

fn candidate_kind() -> &'static str {
    if std::env::args().any(|argument| argument == "--candidate") {
        "agent"
    } else {
        "reference"
    }
}

fn validate_candidate_registry() -> Result<(), String> {
    let candidate = candidate_siege_wurm::definition();
    let registered = card_definitions()
        .into_iter()
        .find(|definition| definition.id == CARD_ID)
        .ok_or("Siege Wurm candidate or registry differs from exact RAV #183 identity")?;
    if candidate != expected_definition() || registered != candidate {
        return Err(
            "Siege Wurm candidate or registry differs from exact RAV #183 identity".to_owned(),
        );
    }
    Ok(())
}

fn expected_definition() -> CardDefinition {
    CardDefinition {
        id: CARD_ID,
        name: "Siege Wurm",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(5, [Color::Green, Color::Green]),
        colors: BTreeSet::from([Color::Green]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Creature]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "convoke",
            "base-characteristics",
            "trample",
        ],
        power: Some(5),
        toughness: Some(5),
        keywords: vec![Keyword::Convoke, Keyword::Trample],
        effects: vec![],
    }
}

fn validate_candidate_source() -> Result<(), String> {
    let source = std::fs::read_to_string(SOURCE_PATH)
        .map_err(|error| format!("could not read Siege Wurm source: {error}"))?;
    let file =
        syn::parse_file(&source).map_err(|_| "Siege Wurm source is not valid Rust".to_owned())?;
    if !file
        .attrs
        .iter()
        .all(|attribute| attribute.path().is_ident("doc"))
        || file.items.len() != 3
    {
        return Err(
            "Siege Wurm module must contain only private imports and one direct-literal definition"
                .to_owned(),
        );
    }
    let mut imports = 0;
    let mut definitions = 0;
    for item in &file.items {
        match item {
            Item::Use(item)
                if item.attrs.is_empty() && matches!(&item.vis, Visibility::Inherited) =>
            {
                imports += 1;
                if !safe_use_tree(&item.tree) {
                    return Err("Siege Wurm source contains an unsupported import".to_owned());
                }
            }
            Item::Fn(function) => {
                definitions += 1;
                if !function.attrs.iter().all(|attribute| {
                    attribute.path().is_ident("must_use") || attribute.path().is_ident("doc")
                }) || function.sig.ident != "definition"
                    || !matches!(&function.vis, Visibility::Restricted(vis) if vis.path.is_ident("crate"))
                    || !function.sig.inputs.is_empty()
                    || function.sig.constness.is_some()
                    || function.sig.asyncness.is_some()
                    || function.sig.unsafety.is_some()
                    || !function.sig.generics.params.is_empty()
                    || function.block.stmts.len() != 1
                    || !matches!(&function.block.stmts[0], Stmt::Expr(Expr::Struct(literal), None) if literal.path.segments.last().is_some_and(|segment| segment.ident == "CardDefinition") && literal.rest.is_none() && literal.fields.iter().all(|field| safe_expression(&field.expr)))
                {
                    return Err(
                        "Siege Wurm must expose one crate-private direct-literal CardDefinition"
                            .to_owned(),
                    );
                }
            }
            _ => {
                return Err(
                    "Siege Wurm source contains an executable or unsupported item".to_owned(),
                );
            }
        }
    }
    if imports != 2 || definitions != 1 {
        return Err(
            "Siege Wurm imports or definition count differs from the source contract".to_owned(),
        );
    }
    Ok(())
}

fn safe_use_tree(tree: &UseTree) -> bool {
    match tree {
        UseTree::Path(path) => {
            matches!(
                path.ident.to_string().as_str(),
                "std" | "collections" | "cardbench_magic_engine"
            ) && safe_use_tree(&path.tree)
        }
        UseTree::Name(name) => matches!(
            name.ident.to_string().as_str(),
            "BTreeSet" | "CardDefinition" | "CardType" | "Color" | "Keyword" | "ManaCost"
        ),
        UseTree::Group(group) => group.items.iter().all(safe_use_tree),
        UseTree::Rename(_) | UseTree::Glob(_) => false,
    }
}

fn safe_expression(expression: &Expr) -> bool {
    match expression {
        Expr::Lit(literal) => matches!(
            literal.lit,
            syn::Lit::Str(_) | syn::Lit::Int(_) | syn::Lit::Bool(_)
        ),
        Expr::Path(path) => {
            let segments: Vec<_> = path
                .path
                .segments
                .iter()
                .map(|segment| segment.ident.to_string())
                .collect();
            path_is(&segments, &["None"])
                || path_is(&segments, &["Color", "Green"])
                || path_is(&segments, &["CardType", "Creature"])
                || path_is(&segments, &["Keyword", "Convoke"])
                || path_is(&segments, &["Keyword", "Trample"])
        }
        Expr::Call(call) => {
            let Expr::Path(path) = &*call.func else {
                return false;
            };
            let segments: Vec<_> = path
                .path
                .segments
                .iter()
                .map(|segment| segment.ident.to_string())
                .collect();
            (path_is(&segments, &["ManaCost", "with_colors"])
                || path_is(&segments, &["BTreeSet", "from"])
                || path_is(&segments, &["BTreeSet", "new"])
                || path_is(&segments, &["Some"]))
                && call.args.iter().all(safe_expression)
        }
        Expr::Array(array) => array.elems.iter().all(safe_expression),
        Expr::Reference(reference) => safe_expression(&reference.expr),
        Expr::Macro(mac) if mac.mac.path.is_ident("vec") => {
            syn::punctuated::Punctuated::<Expr, syn::Token![,]>::parse_terminated
                .parse2(mac.mac.tokens.clone())
                .is_ok_and(|values| values.iter().all(safe_expression))
        }
        _ => false,
    }
}

fn path_is(actual: &[String], expected: &[&str]) -> bool {
    actual.len() == expected.len()
        && actual
            .iter()
            .zip(expected)
            .all(|(actual, expected)| actual == expected)
}

fn deck(entries: &[(&str, u8)]) -> DeckList {
    DeckList {
        mainboard: entries
            .iter()
            .map(|(card, count)| DeckEntry {
                card: (*card).to_owned(),
                count: *count,
            })
            .collect(),
        sideboard: vec![],
    }
}

fn new_game(mutation: Mutation) -> Result<Game, String> {
    let mut definition = candidate_siege_wurm::definition();
    match mutation {
        Mutation::None => {}
        Mutation::NoConvoke => definition
            .keywords
            .retain(|keyword| *keyword != Keyword::Convoke),
        Mutation::NoTrample => definition
            .keywords
            .retain(|keyword| *keyword != Keyword::Trample),
        Mutation::HigherCost => {
            definition.mana_cost = ManaCost::with_colors(6, [Color::Green, Color::Green])
        }
    }
    let mut definitions = card_definitions();
    *definitions
        .iter_mut()
        .find(|card| card.id == CARD_ID)
        .ok_or("Siege Wurm definition missing during game construction")? = definition;
    Game::new(definitions, 2).map_err(|error| error.to_string())
}

fn run_match(seed: u64, mutation: Mutation) -> Result<Captured, String> {
    let mut game = new_game(mutation)?;
    let pilot = deck(&[
        ("RAV-GOLGARI-BROWNSCALE", 4),
        ("RAV-BIRDS-OF-PARADISE", 4),
        ("RAV-TRANSLUMINANT", 4),
        (CARD_ID, 4),
        ("RAV-SCATTER-THE-SEEDS", 4),
        ("RAV-FOREST", 40),
    ]);
    let opponent = deck(&[(BLOCKER_ID, 12), ("RAV-PLAINS", 24), ("RAV-FOREST", 24)]);
    for (label, list) in [
        ("Selesnya Siege pilot", &pilot),
        ("Watchwolf opponent", &opponent),
    ] {
        if list
            .mainboard
            .iter()
            .map(|entry| usize::from(entry.count))
            .sum::<usize>()
            != 60
        {
            return Err(format!("{label} fixture must contain exactly 60 cards"));
        }
    }
    game.set_shuffle_seed(seed)
        .map_err(|error| error.to_string())?;
    for (player, list) in [(PlayerId(0), &pilot), (PlayerId(1), &opponent)] {
        game.load_deck_into_library(player, list)
            .map_err(|error| error.to_string())?;
        game.draw_opening_hand(player, 7)
            .map_err(|error| error.to_string())?;
    }
    game.begin_game().map_err(|error| error.to_string())?;
    let mut policies: [Box<dyn CodePolicy>; 2] = [
        Box::new(SiegeDeckPolicy::new()),
        Box::new(WatchwolfBlockerPolicy::new(PlayerId(1))),
    ];
    let mut captured = Captured {
        events: vec![],
        winner: None,
        turns: 0,
        accepted: 0,
        rejected: 0,
        spell: None,
        spell_zone: None,
        paid_colors: vec![],
        convokers: vec![],
        development_spells: vec![],
        candidate_power: None,
        candidate_toughness: None,
        candidate_has_convoke: false,
        candidate_has_trample: false,
        blocker: None,
        blocker_cast: None,
        blocker_resolved: false,
        blocker_zone: None,
        blocker_graveyard_move_observed: false,
        blocker_damage: vec![],
        player_damage: vec![],
        blocker_sbas: vec![],
        blocker_assigned: false,
        resolved: false,
        opponent_life: None,
    };
    while !game.is_game_over() {
        if game.turn > 42 || captured.accepted >= 8_000 {
            let view = game
                .view_for_player(PlayerId(0))
                .map_err(|error| error.to_string())?;
            return Err(format!(
                "Siege Wurm PvP exceeded bound at turn {} (candidate={:?}, development={:?}, battlefield={:?}, hand={:?}, mana={:?}, opponent_life={})",
                game.turn,
                captured.spell,
                captured.development_spells,
                view.own_battlefield
                    .iter()
                    .map(|card| card.definition)
                    .collect::<Vec<_>>(),
                view.hand
                    .iter()
                    .map(|card| card.definition)
                    .collect::<Vec<_>>(),
                view.mana_pool,
                game.player(PlayerId(1))
                    .map_err(|error| error.to_string())?
                    .life
            ));
        }
        let player = game.next_policy_player();
        let view = game
            .view_for_player(player)
            .map_err(|error| error.to_string())?;
        let policy = policies
            .get_mut(player.0)
            .ok_or("Siege Wurm policy seat missing")?;
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
        let requested_action = format!("{action:?}");
        let before = game.event_log.len();
        game.submit_policy_move(player, policy.id(), action)
            .map_err(|error| {
                format!(
                    "Siege Wurm policy move rejected on turn {} at {:?} (pool {:?}, action {}): {error}",
                    view.turn, view.step, view.mana_pool, requested_action
                )
            })?;
        captured.accepted += 1;
        game.validate_invariants()
            .map_err(|error| error.to_string())?;
        observe(&game, &game.event_log[before..], before, &mut captured)?;
    }
    captured.events = game.event_log.clone();
    captured.winner = game.winner().map(|winner| winner.0);
    captured.turns = game.turn;
    captured.spell_zone = captured.spell.and_then(|spell| game.zone_of(spell));
    captured.blocker_zone = captured
        .blocker_graveyard_move_observed
        .then_some(Zone::Graveyard);
    captured.opponent_life = game.player(PlayerId(1)).ok().map(|player| player.life);
    Ok(captured)
}

fn observe(
    game: &Game,
    events: &[GameEvent],
    event_offset: usize,
    captured: &mut Captured,
) -> Result<(), String> {
    let mut pending_convoke = Vec::new();
    for (local_index, event) in events.iter().enumerate() {
        match event {
            GameEvent::ConvokeUsed {
                player: PlayerId(0),
                creature,
                contribution,
            } => {
                let tapped = game
                    .object(*creature)
                    .map_err(|error| error.to_string())?
                    .tapped;
                let definition = game
                    .card_definition(*creature)
                    .map(|definition| definition.id)
                    .or_else(|_| {
                        game.object(*creature)
                            .ok()
                            .and_then(|object| object.token.as_ref().map(|token| token.name))
                            .ok_or_else(|| {
                                "Convoke contributor has no card or token identity".to_owned()
                            })
                    })?;
                pending_convoke.push((
                    *creature,
                    *contribution,
                    tapped,
                    definition,
                    event_offset + local_index,
                ));
            }
            GameEvent::SpellManaPaid {
                player: PlayerId(0),
                card,
                colors,
            } if game
                .card_definition(*card)
                .is_ok_and(|definition| definition.id == CARD_ID) =>
            {
                captured.spell = Some(*card);
                captured.paid_colors = colors.clone();
            }
            GameEvent::SpellCast {
                player: PlayerId(0),
                card,
            } if game
                .card_definition(*card)
                .is_ok_and(|definition| definition.id == CARD_ID) =>
            {
                captured.spell = Some(*card);
                captured.convokers = pending_convoke.clone();
                pending_convoke.clear();
            }
            GameEvent::SpellCast {
                player: PlayerId(0),
                card,
            } => {
                if let Ok(definition) = game.card_definition(*card) {
                    captured.development_spells.push((*card, definition.id));
                }
                pending_convoke.clear();
            }
            GameEvent::SpellCast {
                player: PlayerId(1),
                card,
            } if game
                .card_definition(*card)
                .is_ok_and(|definition| definition.id == BLOCKER_ID) =>
            {
                captured.blocker_cast = Some(*card)
            }
            GameEvent::SpellResolved { card } if Some(*card) == captured.blocker_cast => {
                captured.blocker_resolved = true
            }
            GameEvent::SpellResolved { card } if Some(*card) == captured.spell => {
                captured.resolved = true
            }
            GameEvent::CardMoved {
                card,
                to: Zone::Battlefield,
            } if Some(*card) == captured.spell => {
                let characteristics = game
                    .characteristics(*card)
                    .map_err(|error| error.to_string())?;
                captured.candidate_power = characteristics.power;
                captured.candidate_toughness = characteristics.toughness;
                captured.candidate_has_convoke =
                    characteristics.keywords.contains(&Keyword::Convoke);
                captured.candidate_has_trample =
                    characteristics.keywords.contains(&Keyword::Trample);
            }
            GameEvent::BlockersDeclared {
                player: PlayerId(1),
                assignments,
            } if captured.spell.is_some_and(|spell| {
                assignments.iter().any(|(attacker, _)| *attacker == spell)
            }) =>
            {
                if let Some((_, blocker)) = assignments
                    .iter()
                    .find(|(attacker, _)| Some(*attacker) == captured.spell)
                {
                    captured.blocker = Some(*blocker);
                    captured.blocker_assigned = true;
                }
            }
            GameEvent::DamageDealtToPermanent {
                source,
                permanent,
                amount,
            } if Some(*source) == captured.spell => {
                captured.blocker_damage.push((*permanent, *amount));
            }
            GameEvent::DamageDealtToPlayer {
                source,
                player: PlayerId(1),
                amount,
            } if Some(*source) == captured.spell => captured.player_damage.push(*amount),
            GameEvent::StateBasedAction {
                card,
                reason: "creature has lethal damage",
            } if Some(*card) == captured.blocker => captured.blocker_sbas.push(*card),
            GameEvent::CardMoved {
                card,
                to: Zone::Graveyard,
            } if Some(*card) == captured.blocker => {
                captured.blocker_graveyard_move_observed = true;
            }
            _ => {}
        }
    }
    Ok(())
}

struct SiegeDeckPolicy {
    candidate: Option<ObjectId>,
}
impl SiegeDeckPolicy {
    fn new() -> Self {
        Self { candidate: None }
    }
}
impl CodePolicy for SiegeDeckPolicy {
    fn id(&self) -> &'static str {
        "rav.siege-wurm-full-game-convoke.v1"
    }
    fn propose_move(&mut self, view: &GameView) -> PolicyAction {
        if view.player == PlayerId(0)
            && view.step == Step::DeclareAttackers
            && view.active_player == PlayerId(0)
            && !view.attackers_declared
        {
            return PolicyAction::DeclareAttackers {
                attackers: view
                    .own_battlefield
                    .iter()
                    .filter(|card| Some(card.id) == self.candidate && card.can_attack)
                    .map(|card| card.id)
                    .collect(),
            };
        }
        if view.player != PlayerId(0)
            || view.decision_player != PlayerId(0)
            || view.priority != PlayerId(0)
            || view.active_player != PlayerId(0)
            || !view.step.is_main()
            || view.stack_depth != 0
        {
            return PolicyAction::PassPriority;
        }
        if view.lands_played == 0
            && let Some(land) = view.hand.iter().find(|card| {
                card.card_types.contains(&CardType::Land)
                    && card.mana_colors.contains(&Color::Green)
            })
        {
            return PolicyAction::PlayLand { card: land.id };
        }

        if self.candidate.is_some() {
            return PolicyAction::PassPriority;
        }

        if let Some(card) = view
            .hand
            .iter()
            .find(|card| card.definition == Some(CARD_ID))
            && let Some(convoke) = full_wurm_convoke(view)
        {
            let request = CastRequest {
                card: card.id,
                targets: vec![],
                convoke,
                payment_mana_abilities: vec![],
            };
            self.candidate = Some(card.id);
            return PolicyAction::Cast(request);
        }

        if let Some(card) = view
            .hand
            .iter()
            .find(|card| card.definition == Some("RAV-BIRDS-OF-PARADISE"))
            && can_pay_green_cost(view, 1, 0)
        {
            return PolicyAction::Cast(CastRequest {
                card: card.id,
                targets: vec![],
                convoke: vec![],
                payment_mana_abilities: vec![],
            });
        }

        if let Some(card) = view
            .hand
            .iter()
            .find(|card| card.definition == Some("RAV-TRANSLUMINANT"))
            && can_pay_green_cost(view, 1, 1)
        {
            return PolicyAction::Cast(CastRequest {
                card: card.id,
                targets: vec![],
                convoke: vec![],
                payment_mana_abilities: vec![],
            });
        }

        if let Some(card) = view
            .hand
            .iter()
            .find(|card| card.definition == Some("RAV-GOLGARI-BROWNSCALE"))
            && can_pay_green_cost(view, 2, 1)
        {
            return PolicyAction::Cast(CastRequest {
                card: card.id,
                targets: vec![],
                convoke: vec![],
                payment_mana_abilities: vec![],
            });
        }

        if let Some(card) = view
            .hand
            .iter()
            .find(|card| card.definition == Some("RAV-SCATTER-THE-SEEDS"))
            && can_pay_green_cost(view, 2, 3)
        {
            return PolicyAction::Cast(CastRequest {
                card: card.id,
                targets: vec![],
                convoke: vec![],
                payment_mana_abilities: vec![],
            });
        }

        if let Some(land) = view.own_battlefield.iter().find(|card| {
            !card.tapped
                && card.card_types.contains(&CardType::Land)
                && card.mana_colors.contains(&Color::Green)
        }) && view.hand.iter().any(|card| {
            matches!(
                card.definition,
                Some(
                    "RAV-GOLGARI-BROWNSCALE"
                        | "RAV-SCATTER-THE-SEEDS"
                        | "RAV-TRANSLUMINANT"
                        | "RAV-BIRDS-OF-PARADISE"
                )
            )
        }) {
            return PolicyAction::ActivateManaAbility {
                land: land.id,
                color: Color::Green,
            };
        }
        PolicyAction::PassPriority
    }
}

fn can_pay_green_cost(view: &GameView, colored_green: u8, generic: u8) -> bool {
    view.mana_pool.amount(Color::Green) >= colored_green
        && view.mana_pool.total().saturating_sub(colored_green) >= generic
}

fn full_wurm_convoke(view: &GameView) -> Option<Vec<ConvokePayment>> {
    if view.mana_pool.total() != 0 {
        return None;
    }
    let creatures: Vec<_> = view
        .own_battlefield
        .iter()
        .filter(|card| !card.tapped && card.card_types.contains(&CardType::Creature))
        .collect();
    let green: Vec<_> = creatures
        .iter()
        .filter(|card| card.colors.contains(&Color::Green))
        .take(2)
        .collect();
    if green.len() != 2 {
        return None;
    }
    let green_ids: BTreeSet<_> = green.iter().map(|card| card.id).collect();
    let generic: Vec<_> = creatures
        .iter()
        .filter(|card| !green_ids.contains(&card.id))
        .take(5)
        .collect();
    if generic.len() != 5 {
        return None;
    }
    Some(
        green
            .into_iter()
            .map(|card| ConvokePayment {
                creature: card.id,
                contribution: ConvokeContribution::Color(Color::Green),
            })
            .chain(generic.into_iter().map(|card| ConvokePayment {
                creature: card.id,
                contribution: ConvokeContribution::Generic,
            }))
            .collect(),
    )
}

struct WatchwolfBlockerPolicy {
    player: PlayerId,
    cast_watchwolf: bool,
}
impl WatchwolfBlockerPolicy {
    fn new(player: PlayerId) -> Self {
        Self {
            player,
            cast_watchwolf: false,
        }
    }
    fn play_land(&self, view: &GameView) -> Option<PolicyAction> {
        if view.lands_played != 0 {
            return None;
        }
        let has_green = view.own_battlefield.iter().any(|card| {
            card.card_types.contains(&CardType::Land) && card.mana_colors.contains(&Color::Green)
        });
        let has_white = view.own_battlefield.iter().any(|card| {
            card.card_types.contains(&CardType::Land) && card.mana_colors.contains(&Color::White)
        });
        let preferred = if !has_green {
            "RAV-FOREST"
        } else if !has_white {
            "RAV-PLAINS"
        } else {
            "RAV-FOREST"
        };
        view.hand
            .iter()
            .find(|card| card.definition == Some(preferred))
            .or_else(|| {
                view.hand
                    .iter()
                    .find(|card| card.card_types.contains(&CardType::Land))
            })
            .map(|card| PolicyAction::PlayLand { card: card.id })
    }
}
impl CodePolicy for WatchwolfBlockerPolicy {
    fn id(&self) -> &'static str {
        "rav.watchwolf-full-game-blocker.v1"
    }
    fn propose_move(&mut self, view: &GameView) -> PolicyAction {
        if view.player != self.player || view.decision_player != self.player {
            return PolicyAction::PassPriority;
        }
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
            let attacker = view
                .combat_attackers
                .iter()
                .find(|card| card.definition == Some(CARD_ID));
            let blocker = view
                .own_battlefield
                .iter()
                .find(|card| card.definition == Some(BLOCKER_ID) && card.can_block);
            let assignments = match (attacker, blocker) {
                (Some(attacker), Some(blocker)) => vec![CombatBlock {
                    attacker: attacker.id,
                    blocker: blocker.id,
                }],
                _ => vec![],
            };
            return PolicyAction::DeclareBlockers { assignments };
        }
        if view.priority != self.player
            || view.active_player != self.player
            || !view.step.is_main()
            || view.stack_depth != 0
        {
            return PolicyAction::PassPriority;
        }
        if let Some(action) = self.play_land(view) {
            return action;
        }
        if !self.cast_watchwolf
            && view
                .hand
                .iter()
                .any(|card| card.definition == Some(BLOCKER_ID))
        {
            if view.mana_pool.amount(Color::Green) == 0 {
                if let Some(land) = view.own_battlefield.iter().find(|card| {
                    !card.tapped
                        && card.card_types.contains(&CardType::Land)
                        && card.mana_colors.contains(&Color::Green)
                }) {
                    return PolicyAction::ActivateManaAbility {
                        land: land.id,
                        color: Color::Green,
                    };
                }
            }
            if view.mana_pool.amount(Color::White) == 0 {
                if let Some(land) = view.own_battlefield.iter().find(|card| {
                    !card.tapped
                        && card.card_types.contains(&CardType::Land)
                        && card.mana_colors.contains(&Color::White)
                }) {
                    return PolicyAction::ActivateManaAbility {
                        land: land.id,
                        color: Color::White,
                    };
                }
            }
            if view.mana_pool.amount(Color::Green) > 0 && view.mana_pool.amount(Color::White) > 0 {
                let card = view
                    .hand
                    .iter()
                    .find(|card| card.definition == Some(BLOCKER_ID))
                    .expect("checked hand");
                self.cast_watchwolf = true;
                return PolicyAction::Cast(CastRequest {
                    card: card.id,
                    targets: vec![],
                    convoke: vec![],
                    payment_mana_abilities: vec![],
                });
            }
        }
        PolicyAction::PassPriority
    }
}

fn find_trace() -> Result<(u64, Captured), String> {
    if let Ok(value) = std::env::var("CARDBENCH_SIEGE_WURM_SEED") {
        let seed = value
            .parse::<u64>()
            .map_err(|_| "CARDBENCH_SIEGE_WURM_SEED must be an unsigned integer".to_owned())?;
        let captured = run_match(seed, Mutation::None)?;
        validate_trace(&captured, seed)?;
        return Ok((seed, captured));
    }
    let mut last_error = String::new();
    for seed in 0..=MAX_TRACE_SEED {
        match run_match(seed, Mutation::None)
            .and_then(|captured| validate_trace(&captured, seed).map(|_| captured))
        {
            Ok(captured) => return Ok((seed, captured)),
            Err(error) => last_error = error,
        }
    }
    Err(format!(
        "no Siege Wurm Convoke/Trample full-game trace found for seeds 0..={MAX_TRACE_SEED}: {last_error}"
    ))
}

fn validate_trace(captured: &Captured, seed: u64) -> Result<Value, String> {
    if captured.winner != Some(0) || captured.rejected != 0 || captured.accepted == 0 {
        return Err("Siege Wurm duel did not end in a clean P0 terminal win".to_owned());
    }
    let spell = captured
        .spell
        .ok_or("Siege Wurm was not cast from the shuffled pilot deck")?;
    let blocker = captured.blocker.ok_or_else(|| format!(
        "a RAV Watchwolf did not block Siege Wurm; cast={:?}, resolved={}, turns={}, opponent_life={:?}, development={:?}, combat={:?}",
        captured.blocker_cast, captured.blocker_resolved, captured.turns, captured.opponent_life, captured.development_spells,
        captured.events.iter().filter(|event| matches!(event, GameEvent::AttackersDeclared { .. } | GameEvent::BlockersDeclared { .. } | GameEvent::SpellCast { .. })).collect::<Vec<_>>()
    ))?;
    if captured.spell_zone != Some(Zone::Battlefield) || !captured.resolved {
        return Err("Siege Wurm did not resolve and remain on the battlefield".to_owned());
    }
    if candidate_siege_wurm::definition() != expected_definition() {
        return Err(
            "Siege Wurm candidate or registry differs from exact RAV #183 identity".to_owned(),
        );
    }
    if (captured.candidate_power, captured.candidate_toughness) != (Some(5), Some(5))
        || !captured.candidate_has_convoke
        || !captured.candidate_has_trample
    {
        return Err("live Siege Wurm characteristics or keywords differ from the 5/5 Convoke/Trample source contract".to_owned());
    }
    if captured.convokers.len() != 7
        || captured
            .convokers
            .iter()
            .map(|(id, _, _, _, _)| *id)
            .collect::<BTreeSet<_>>()
            .len()
            != 7
        || captured
            .convokers
            .iter()
            .any(|(_, _, tapped, _, _)| !tapped)
    {
        return Err(format!(
            "Siege Wurm must tap seven distinct RAV creatures for Convoke: {:?}",
            captured.convokers
        ));
    }
    if captured
        .convokers
        .iter()
        .filter(|(_, color, _, _, _)| color.is_none())
        .count()
        != 5
        || captured
            .convokers
            .iter()
            .filter(|(_, color, _, _, _)| *color == Some(Color::Green))
            .count()
            != 2
        || !captured.paid_colors.is_empty()
        || captured
            .development_spells
            .iter()
            .all(|(_, id)| *id != "RAV-SCATTER-THE-SEEDS")
    {
        return Err(format!(
            "exact five generic plus two green Convoke payments and a resolved RAV token-development cast are required; paid={:?}, contributors={:?}, spells={:?}",
            captured.paid_colors, captured.convokers, captured.development_spells
        ));
    }
    if captured.blocker_cast != Some(blocker)
        || !captured.blocker_resolved
        || captured.blocker_zone != Some(Zone::Graveyard)
        || !captured.blocker_assigned
        || captured.blocker_damage != [(blocker, 3)]
        || captured.blocker_sbas != [blocker]
    {
        return Err(format!(
            "Siege Wurm failed its cast Watchwolf/blocked Trample/SBA trace: blocker={blocker:?}, cast={:?}/{}, zone={:?}, assigned={}, damage={:?}, SBA={:?}",
            captured.blocker_cast,
            captured.blocker_resolved,
            captured.blocker_zone,
            captured.blocker_assigned,
            captured.blocker_damage,
            captured.blocker_sbas
        ));
    }
    if captured.player_damage.first() != Some(&2)
        || captured
            .player_damage
            .iter()
            .map(|amount| i64::from(*amount))
            .sum::<i64>()
            < 20
        || !captured.opponent_life.is_some_and(|life| life <= 0)
    {
        return Err(format!(
            "Siege Wurm failed Trample-through and terminal combat: damage={:?}, life={:?}",
            captured.player_damage, captured.opponent_life
        ));
    }

    let events = &captured.events;
    let convoke_indices: Vec<_> = captured
        .convokers
        .iter()
        .map(|(_, _, _, _, index)| *index)
        .collect();
    if convoke_indices.iter().any(|index| {
        !matches!(
            events.get(*index),
            Some(GameEvent::ConvokeUsed {
                player: PlayerId(0),
                ..
            })
        )
    }) {
        return Err(
            "captured Siege Wurm contributors do not map to ConvokeUsed receipts".to_owned(),
        );
    }
    let cast = unique_index(events, |event| matches!(event, GameEvent::SpellCast { player: PlayerId(0), card } if *card == spell)).ok_or("Siege Wurm SpellCast receipt missing")?;
    let resolved = unique_index(
        events,
        |event| matches!(event, GameEvent::SpellResolved { card } if *card == spell),
    )
    .ok_or("Siege Wurm SpellResolved receipt missing")?;
    let entered = unique_index(events, |event| matches!(event, GameEvent::CardMoved { card, to: Zone::Battlefield } if *card == spell)).ok_or("Siege Wurm battlefield entry receipt missing")?;
    let blocker_cast = unique_index(events, |event| matches!(event, GameEvent::SpellCast { player: PlayerId(1), card } if *card == blocker)).ok_or("Watchwolf was not cast from the shuffled opponent deck")?;
    let blocker_resolved = unique_index(
        events,
        |event| matches!(event, GameEvent::SpellResolved { card } if *card == blocker),
    )
    .ok_or("Watchwolf SpellResolved receipt missing")?;
    let blocker_entered = unique_index(events, |event| matches!(event, GameEvent::CardMoved { card, to: Zone::Battlefield } if *card == blocker)).ok_or("Watchwolf battlefield entry receipt missing")?;
    let attack = first_index(events, |event| matches!(event, GameEvent::AttackersDeclared { player: PlayerId(0), attackers } if attackers.contains(&spell))).ok_or("Siege Wurm did not attack")?;
    let blockers = unique_index(events, |event| matches!(event, GameEvent::BlockersDeclared { player: PlayerId(1), assignments } if assignments.iter().any(|(attacker, assigned_blocker)| *attacker == spell && *assigned_blocker == blocker))).ok_or("Watchwolf did not block Siege Wurm")?;
    let blocker_damage = unique_index(events, |event| matches!(event, GameEvent::DamageDealtToPermanent { source, permanent, amount: 3 } if *source == spell && *permanent == blocker)).ok_or("Siege Wurm did not assign lethal 3 damage to Watchwolf")?;
    let trample_damage = unique_index(events, |event| matches!(event, GameEvent::DamageDealtToPlayer { source, player: PlayerId(1), amount: 2 } if *source == spell)).ok_or("Siege Wurm did not trample exactly 2 damage over Watchwolf")?;
    let blocker_sba = unique_index(events, |event| matches!(event, GameEvent::StateBasedAction { card, reason: "creature has lethal damage" } if *card == blocker)).ok_or("Watchwolf lethal-damage state-based action missing")?;
    let blocker_grave = unique_index(events, |event| matches!(event, GameEvent::CardMoved { card, to: Zone::Graveyard } if *card == blocker)).ok_or("Watchwolf did not move to its owner's graveyard")?;
    let terminal = unique_index(events, |event| {
        matches!(
            event,
            GameEvent::GameEnded {
                winner: Some(PlayerId(0))
            }
        )
    })
    .ok_or("terminal P0 GameEnded receipt missing")?;
    if convoke_indices.iter().any(|index| *index >= cast)
        || !(blocker_cast < blocker_resolved
            && blocker_resolved < blocker_entered
            && convoke_indices.iter().all(|index| *index < cast)
            && cast < resolved
            && resolved < entered
            && entered < attack
            && attack < blockers
            && blockers < blocker_damage
            && blocker_damage < trample_damage
            && trample_damage < blocker_sba
            && blocker_sba < blocker_grave
            && blocker_grave < terminal)
    {
        return Err(
            "Siege Wurm deck development/Convoke/cast/combat/SBA/terminal event order invalid"
                .to_owned(),
        );
    }
    let mut selected: BTreeSet<usize> = convoke_indices.into_iter().collect();
    selected.extend([
        cast,
        resolved,
        entered,
        blocker_cast,
        blocker_resolved,
        blocker_entered,
        attack,
        blockers,
        blocker_damage,
        trample_damage,
        blocker_sba,
        blocker_grave,
        terminal,
    ]);
    for (index, event) in events.iter().enumerate() {
        if matches!(event, GameEvent::DamageDealtToPlayer { source, player: PlayerId(1), .. } if *source == spell)
        {
            selected.insert(index);
        }
    }
    for (card, definition) in &captured.development_spells {
        if matches!(
            *definition,
            "RAV-SCATTER-THE-SEEDS"
                | "RAV-GOLGARI-BROWNSCALE"
                | "RAV-TRANSLUMINANT"
                | "RAV-BIRDS-OF-PARADISE"
        ) {
            for (index, event) in events.iter().enumerate() {
                if matches!(event, GameEvent::SpellCast { player: PlayerId(0), card: candidate } if candidate == card)
                    || matches!(event, GameEvent::SpellResolved { card: candidate } if candidate == card)
                    || matches!(event, GameEvent::CardMoved { card: candidate, to: Zone::Battlefield } if candidate == card)
                {
                    selected.insert(index);
                }
            }
        }
    }
    let projected = project_selected(events, &selected)?;
    Ok(json!({
        "id": "rav-siege-wurm-seven-convoke-trample-terminal",
        "seed": seed,
        "decks": ["rav-siege-wurm-convoke-60", "rav-watchwolf-12-60"],
        "policies": ["rav.siege-wurm-full-game-convoke.v1", "rav.watchwolf-full-game-blocker.v1"],
        "terminal": "Winner(PlayerId(0))",
        "winner_seat": captured.winner,
        "turns": captured.turns,
        "accepted_moves": captured.accepted,
        "rejected_moves": captured.rejected,
        "spell_object": spell.0,
        "spell_zone": format!("{:?}", captured.spell_zone),
        "mana_paid": captured.paid_colors.iter().map(|color| format!("{color:?}")).collect::<Vec<_>>(),
        "development_spells": captured.development_spells.iter().map(|(object, definition)| json!({"object": object.0, "definition": definition})).collect::<Vec<_>>(),
        "convoke_payments": captured.convokers.iter().map(|(creature, contribution, tapped, definition, _)| json!({"creature": creature.0, "definition": definition, "contribution": contribution.map(|color| format!("{color:?}")).unwrap_or_else(|| "Generic".to_owned()), "tapped": tapped})).collect::<Vec<_>>(),
        "live_power_toughness": [captured.candidate_power, captured.candidate_toughness],
        "live_convoke": captured.candidate_has_convoke,
        "live_trample": captured.candidate_has_trample,
        "blocker_object": blocker.0,
        "blocker_definition": BLOCKER_ID,
        "blocker_zone": format!("{:?}", captured.blocker_zone),
        "blocker_damage": captured.blocker_damage.iter().map(|(object, amount)| json!({"object": object.0, "amount": amount})).collect::<Vec<_>>(),
        "trample_damage_to_opponent": captured.player_damage,
        "opponent_life_after_combat": captured.opponent_life,
        "ordered_event_sequence": selected.iter().copied().collect::<Vec<_>>(),
        "events": projected,
        "passed": true
    }))
}

fn project_selected(
    events: &[GameEvent],
    selected: &BTreeSet<usize>,
) -> Result<Vec<Value>, String> {
    let projected = project_events(events);
    let mut output = Vec::new();
    for index in selected {
        let mut value = serde_json::to_value(
            projected
                .get(*index)
                .ok_or("projected event index missing")?,
        )
        .map_err(|error| error.to_string())?;
        if let GameEvent::ConvokeUsed {
            player,
            creature,
            contribution,
        } = events.get(*index).ok_or("raw event index missing")?
        {
            value["facts"] = json!({"seats": [player.0], "objects": [creature.0], "contribution": contribution.map(|color| format!("{color:?}")).unwrap_or_else(|| "Generic".to_owned())});
        }
        output.push(value);
    }
    Ok(output)
}

fn mutation_detected(seed: u64, mutation: Mutation) -> Result<bool, String> {
    Ok(run_match(seed, mutation)
        .and_then(|captured| validate_trace(&captured, seed).map(|_| captured))
        .is_err())
}
fn same_match(left: &Captured, right: &Captured) -> bool {
    left == right
}
fn unique_index(events: &[GameEvent], predicate: impl Fn(&GameEvent) -> bool) -> Option<usize> {
    let mut indices = events
        .iter()
        .enumerate()
        .filter_map(|(index, event)| predicate(event).then_some(index));
    let first = indices.next()?;
    indices.next().is_none().then_some(first)
}
fn first_index(events: &[GameEvent], predicate: impl Fn(&GameEvent) -> bool) -> Option<usize> {
    events.iter().position(predicate)
}
fn card_status(card: &CatalogCard) -> Value {
    let definition_id = match card.semantic_status {
        CardSemanticStatus::ExecutableCompatibilitySlice { definition_id } => Some(definition_id),
        CardSemanticStatus::CatalogOnly { .. } => None,
    };
    let passed = definition_id == Some(CARD_ID);
    json!({"collector_number": card.collector_number, "name": card.name, "definition_id": definition_id, "engine_status": match card.semantic_status { CardSemanticStatus::ExecutableCompatibilitySlice { .. } => "executable_compatibility_slice", CardSemanticStatus::CatalogOnly { .. } => "catalog_only" }, "trace_status": if passed { "passed" } else { "not_qualified" }, "credited": false, "qualification": if passed { "diagnostic_pilot_trace_passed; independent review pending" } else { "not_evaluated"}})
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
