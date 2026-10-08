//! Full-game diagnostic authority for RAV Cerulean Sphinx.

use cardbench_magic_engine::{
    CardType, Color, DeckEntry, DeckList, Effect, Game, GameEvent, GameView, Keyword, ManaCost,
    ManaPaymentSelection, ObjectId, PlayerId, PolicyAction, Step, Target, Zone,
};
use cardbench_magic_policies::{CodePolicy, DeckMatchConfig};
use cardbench_magic_protocol::ViewerScope;
use cardbench_magic_rav::{
    CardSemanticStatus, CatalogCard, RAV_MAIN_SET_EXPECTED_PRINTING_COUNT,
    RAV_MAIN_SET_EXPECTED_UNIQUE_NAME_COUNT, card_definitions, rav_activated_ability_bindings,
    rav_additional_spell_cost_bindings, rav_basic_land_type_bindings, rav_main_set_catalog,
    rav_mana_ability_bindings,
};
use cardbench_magic_session::project_events;
use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::process::ExitCode;
use syn::{Expr, Item, Stmt, UseTree, Visibility};

const TASK_ID: &str = "cardbench/magic/card";
const CARD_ID: &str = "RAV-CERULEAN-SPHINX";
const CARD_NAME: &str = "Cerulean Sphinx";
const ABILITY_ID: &str = "shuffle-source-into-owner-library";
const CONSCRIPTION_ID: &str = "RAV-FLASH-CONSCRIPTION";
const SOURCE_PATH: &str =
    "varieties/magic/sets/ravnica_city_of_guilds/src/cards/cerulean_sphinx.rs";
const MAX_TRACE_SEED: u64 = 32;

#[path = "../../../sets/ravnica_city_of_guilds/src/cards/cerulean_sphinx.rs"]
mod candidate_cerulean_sphinx;

fn main() -> ExitCode {
    match run_authority() {
        Ok(report) => {
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
            eprintln!("Cerulean Sphinx PvP authority failed: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run_authority() -> Result<Value, String> {
    validate_candidate_source()?;
    validate_candidate_bindings()?;
    let catalog = unique_catalog()?;
    let (seed, captured) = find_trace()?;
    let trace = validate_trace(&captured, seed)?;
    let replay = run_match(seed, Mutation::None)?;
    if !same_match(&captured, &replay) {
        return Err("Cerulean Sphinx full-game replay was not deterministic".to_owned());
    }
    let mutations = json!({
        "flying_omitted_mutation_detected_in_game": mutation_detected(seed, Mutation::NoFlying)?,
        "source_shuffle_effect_removed_mutation_detected_in_game": mutation_detected(seed, Mutation::NoShuffle)?,
        "activation_cost_changed_mutation_detected_in_game": mutation_detected(seed, Mutation::WrongAbilityCost)?,
        "sealed_mutation_authority": false
    });
    for key in [
        "flying_omitted_mutation_detected_in_game",
        "source_shuffle_effect_removed_mutation_detected_in_game",
        "activation_cost_changed_mutation_detected_in_game",
    ] {
        if mutations[key] != true {
            return Err(format!(
                "Cerulean Sphinx mutation escaped gameplay oracle: {key}"
            ));
        }
    }
    let breakdown: Vec<_> = catalog.iter().map(card_status).collect();
    let cards_passed = breakdown
        .iter()
        .filter(|card| card["trace_status"] == "passed")
        .count();
    Ok(json!({
        "schema_version":"cardbench.magic.card-pvp.check.v1",
        "task_id":TASK_ID,
        "expansion":"RAV",
        "card":{"collector_number":39,"printings":1,"definition_id":CARD_ID,"name":CARD_NAME},
        "candidate":{"kind":candidate_kind(),"card_definition":CARD_ID},
        "candidate_kind":candidate_kind(),
        "viewer_scope":format!("{:?}", ViewerScope::Judge),
        "rules_authority":{
            "comprehensive_rules":"2026-08-07",
            "checked_sections":["302.6 Creature summoning sickness", "601.2 Casting spells and paying costs", "602.1-602.2 Activating abilities", "608.2 Resolving abilities", "701.17 Shuffling", "702.9 Flying", "510 Combat damage"],
            "source_contract":"RAV Cerulean Sphinx #39 is a {4}{U}{U} 5/5 blue Creature with flying and {U}: shuffle Cerulean Sphinx into its owner's library.",
            "authority_cutoff":"2026-09-24",
            "no_provider_trial":true
        },
        "passed":true,
        "pilot_card_passed":true,
        "replay_deterministic":true,
        "cards_passed":cards_passed,
        "cards_credited":0,
        "cards_total":catalog.len(),
        "printings_total":RAV_MAIN_SET_EXPECTED_PRINTING_COUNT,
        "coverage_complete":false,
        "full_reward":false,
        "harbor_reward":0.0,
        "pilot_reward":0.0,
        "qualification_status":"diagnostic_only; independent review and candidate trial pending",
        "mutation_sensitivity":mutations,
        "traces":[trace],
        "per_card_breakdown":breakdown
    }))
}

fn validate_candidate_source() -> Result<(), String> {
    let source = std::fs::read_to_string(SOURCE_PATH)
        .map_err(|error| format!("could not read Cerulean Sphinx source: {error}"))?;
    let file = syn::parse_file(&source)
        .map_err(|_| "Cerulean Sphinx source is not valid Rust".to_owned())?;
    if !file
        .attrs
        .iter()
        .all(|attribute| attribute.path().is_ident("doc"))
    {
        return Err("Cerulean Sphinx has an unsupported module attribute".to_owned());
    }
    let expected_imports: BTreeSet<Vec<String>> = [
        vec!["cardbench_magic_engine", "ActivatedAbility"],
        vec!["cardbench_magic_engine", "ActivatedAbilityBinding"],
        vec!["cardbench_magic_engine", "CardDefinition"],
        vec!["cardbench_magic_engine", "CardType"],
        vec!["cardbench_magic_engine", "Color"],
        vec!["cardbench_magic_engine", "Effect"],
        vec!["cardbench_magic_engine", "Keyword"],
        vec!["cardbench_magic_engine", "ManaCost"],
        vec!["std", "collections", "BTreeSet"],
    ]
    .into_iter()
    .map(|path| path.into_iter().map(str::to_owned).collect())
    .collect();
    let mut imports = BTreeSet::new();
    let mut functions = BTreeSet::new();
    for item in &file.items {
        match item {
            Item::Use(item)
                if item.attrs.is_empty() && matches!(&item.vis, Visibility::Inherited) =>
            {
                let mut paths = Vec::new();
                if item.leading_colon.is_some() || !flatten_use_tree(&[], &item.tree, &mut paths) {
                    return Err("Cerulean Sphinx source has unsupported imports".to_owned());
                }
                imports.extend(paths);
            }
            Item::Fn(function)
                if matches!(&function.vis, Visibility::Restricted(visibility) if visibility.path.is_ident("crate"))
                    && function.sig.inputs.is_empty()
                    && function.sig.generics.params.is_empty()
                    && function.sig.generics.where_clause.is_none()
                    && function.sig.constness.is_none()
                    && function.sig.asyncness.is_none()
                    && function.sig.unsafety.is_none()
                    && function.sig.abi.is_none()
                    && function.sig.variadic.is_none()
                    && function.block.stmts.len() == 1
                    && function.attrs.iter().all(|attribute| {
                        attribute.path().is_ident("doc") || attribute.path().is_ident("must_use")
                    }) =>
            {
                let name = function.sig.ident.to_string();
                let output = match name.as_str() {
                    "definition" => "CardDefinition",
                    "activated_ability" => "ActivatedAbilityBinding",
                    _ => return Err("Cerulean Sphinx has an unsupported function".to_owned()),
                };
                let Stmt::Expr(Expr::Struct(literal), None) = &function.block.stmts[0] else {
                    return Err(
                        "Cerulean Sphinx functions must return direct typed literals".to_owned(),
                    );
                };
                if !type_is(&function.sig.output, output)
                    || !path_is_type(&literal.path, output)
                    || literal.rest.is_some()
                    || !literal.attrs.is_empty()
                    || !functions.insert(name)
                {
                    return Err(
                        "Cerulean Sphinx functions must return direct typed literals".to_owned(),
                    );
                }
            }
            _ => {
                return Err("Cerulean Sphinx source contains unsupported top-level code".to_owned());
            }
        }
    }
    if imports != expected_imports
        || functions != BTreeSet::from(["definition".to_owned(), "activated_ability".to_owned()])
        || file.items.len() != 4
    {
        return Err("Cerulean Sphinx module structure differs from source contract".to_owned());
    }
    Ok(())
}

fn flatten_use_tree(prefix: &[String], tree: &UseTree, paths: &mut Vec<Vec<String>>) -> bool {
    match tree {
        UseTree::Path(item) => {
            let mut next = prefix.to_vec();
            next.push(item.ident.to_string());
            flatten_use_tree(&next, &item.tree, paths)
        }
        UseTree::Name(item) => {
            let mut path = prefix.to_vec();
            path.push(item.ident.to_string());
            paths.push(path);
            true
        }
        UseTree::Group(items) => items
            .items
            .iter()
            .all(|item| flatten_use_tree(prefix, item, paths)),
        UseTree::Rename(_) | UseTree::Glob(_) => false,
    }
}

fn type_is(output: &syn::ReturnType, expected: &str) -> bool {
    matches!(output, syn::ReturnType::Type(_, ty) if matches!(&**ty, syn::Type::Path(path) if path.qself.is_none() && path_is_type(&path.path, expected)))
}

fn path_is_type(path: &syn::Path, expected: &str) -> bool {
    path.leading_colon.is_none()
        && path.segments.len() == 1
        && path.segments[0].ident == expected
        && matches!(path.segments[0].arguments, syn::PathArguments::None)
}

fn validate_candidate_bindings() -> Result<(), String> {
    let definition = candidate_cerulean_sphinx::definition();
    let ability = candidate_cerulean_sphinx::activated_ability();
    let registered = card_definitions()
        .into_iter()
        .find(|card| card.id == CARD_ID)
        .ok_or("Cerulean Sphinx definition missing")?;
    let binding = rav_activated_ability_bindings()
        .into_iter()
        .find(|value| value.card_definition == CARD_ID && value.ability.id == ABILITY_ID)
        .ok_or("Cerulean Sphinx activation missing")?;
    if definition != registered
        || definition.mana_cost != ManaCost::with_colors(4, [Color::Blue, Color::Blue])
        || definition.colors != BTreeSet::from([Color::Blue])
        || definition.card_types != BTreeSet::from([CardType::Creature])
        || (definition.power, definition.toughness) != (Some(5), Some(5))
        || definition.keywords != [Keyword::Flying]
        || ability != binding
        || ability.ability.mana_cost != ManaCost::with_colors(0, [Color::Blue])
        || ability.ability.effects != [Effect::MoveSourceToOwnersLibraryAndShuffle]
    {
        return Err("Cerulean Sphinx literal is outside the task source contract".to_owned());
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Mutation {
    None,
    NoFlying,
    NoShuffle,
    WrongAbilityCost,
}

#[derive(Clone, Debug)]
struct Captured {
    events: Vec<GameEvent>,
    terminal: String,
    winner: Option<usize>,
    turns: u32,
    accepted: u32,
    rejected: u32,
    sphinx: Option<ObjectId>,
    cast_colors: Vec<Color>,
    ability_source: Option<ObjectId>,
    ability_controller: Option<PlayerId>,
    source_owner: Option<PlayerId>,
    source_controller_before: Option<PlayerId>,
    source_zone_after: Option<Zone>,
    owner_library_len_before: usize,
    owner_library_len_after: usize,
    controller_library_len_before: usize,
    controller_library_len_after: usize,
    ability_cost: Option<ManaCost>,
    ability_colors: Vec<Color>,
    flying: bool,
    shuffle_owner: Option<PlayerId>,
    attacks: u32,
    damage: i32,
    post_activation_attacker: Option<ObjectId>,
    post_activation_attacks: u32,
    post_activation_damage: i32,
    conscription: Option<ObjectId>,
    conscription_colors: Vec<Color>,
    conscription_resolved: bool,
}

fn unique_catalog() -> Result<Vec<CatalogCard>, String> {
    let printings = rav_main_set_catalog();
    let mut names = BTreeSet::new();
    let catalog: Vec<_> = printings
        .iter()
        .copied()
        .filter(|card| names.insert(card.name))
        .collect();
    if catalog.len() != RAV_MAIN_SET_EXPECTED_UNIQUE_NAME_COUNT
        || printings.len() != RAV_MAIN_SET_EXPECTED_PRINTING_COUNT
    {
        return Err("RAV catalog denominator changed".to_owned());
    }
    Ok(catalog)
}

fn find_trace() -> Result<(u64, Captured), String> {
    if let Ok(value) = std::env::var("CARDBENCH_CERULEAN_SPHINX_SEED") {
        let seed = value
            .parse::<u64>()
            .map_err(|_| "CARDBENCH_CERULEAN_SPHINX_SEED must be unsigned".to_owned())?;
        let capture = run_match(seed, Mutation::None)?;
        validate_trace(&capture, seed)?;
        return Ok((seed, capture));
    }
    let mut last = String::new();
    for seed in 0..=MAX_TRACE_SEED {
        match run_match(seed, Mutation::None)
            .and_then(|capture| validate_trace(&capture, seed).map(|_| capture))
        {
            Ok(capture) => return Ok((seed, capture)),
            Err(error) => last = error,
        }
    }
    Err(format!(
        "no bounded seed produced a full Cerulean Sphinx game: {last}"
    ))
}

fn mutation_detected(seed: u64, mutation: Mutation) -> Result<bool, String> {
    Ok(match run_match(seed, mutation) {
        Ok(capture) => validate_trace(&capture, seed).is_err(),
        Err(_) => true,
    })
}

fn same_match(a: &Captured, b: &Captured) -> bool {
    a.events == b.events
        && a.terminal == b.terminal
        && a.winner == b.winner
        && a.turns == b.turns
        && a.sphinx == b.sphinx
        && a.cast_colors == b.cast_colors
        && a.ability_source == b.ability_source
        && a.ability_controller == b.ability_controller
        && a.source_owner == b.source_owner
        && a.source_controller_before == b.source_controller_before
        && a.source_zone_after == b.source_zone_after
        && a.owner_library_len_before == b.owner_library_len_before
        && a.owner_library_len_after == b.owner_library_len_after
        && a.controller_library_len_before == b.controller_library_len_before
        && a.controller_library_len_after == b.controller_library_len_after
        && a.ability_cost == b.ability_cost
        && a.ability_colors == b.ability_colors
        && a.flying == b.flying
        && a.shuffle_owner == b.shuffle_owner
        && a.attacks == b.attacks
        && a.damage == b.damage
        && a.post_activation_attacker == b.post_activation_attacker
        && a.post_activation_attacks == b.post_activation_attacks
        && a.post_activation_damage == b.post_activation_damage
        && a.conscription == b.conscription
        && a.conscription_colors == b.conscription_colors
        && a.conscription_resolved == b.conscription_resolved
}

fn run_match(seed: u64, mutation: Mutation) -> Result<Captured, String> {
    let p0 = DeckList {
        mainboard: vec![
            DeckEntry {
                card: CARD_ID.to_owned(),
                count: 4,
            },
            DeckEntry {
                card: "RAV-ISLAND".to_owned(),
                count: 56,
            },
        ],
        sideboard: vec![],
    };
    let p1 = DeckList {
        mainboard: vec![
            DeckEntry {
                card: CONSCRIPTION_ID.to_owned(),
                count: 4,
            },
            DeckEntry {
                card: "RAV-ISLAND".to_owned(),
                count: 30,
            },
            DeckEntry {
                card: "RAV-MOUNTAIN".to_owned(),
                count: 26,
            },
        ],
        sideboard: vec![],
    };
    if [&p0, &p1].iter().any(|deck| {
        deck.mainboard
            .iter()
            .map(|entry| usize::from(entry.count))
            .sum::<usize>()
            != 60
    }) {
        return Err("Cerulean Sphinx fixtures must be legal shuffled 60-card decks".to_owned());
    }
    let mut definition = candidate_cerulean_sphinx::definition();
    let mut binding = candidate_cerulean_sphinx::activated_ability();
    match mutation {
        Mutation::None => {}
        Mutation::NoFlying => definition.keywords.clear(),
        Mutation::NoShuffle => binding.ability.effects.clear(),
        Mutation::WrongAbilityCost => binding.ability.mana_cost = ManaCost::new(2),
    }
    let mut definitions = card_definitions();
    *definitions
        .iter_mut()
        .find(|known| known.id == CARD_ID)
        .ok_or("Cerulean Sphinx missing from registry")? = definition;
    let mut abilities = rav_activated_ability_bindings();
    *abilities
        .iter_mut()
        .find(|known| known.card_definition == CARD_ID && known.ability.id == ABILITY_ID)
        .ok_or("Cerulean Sphinx activation missing from registry")? = binding;
    let mut game = Game::new_with_all_bindings(
        definitions,
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        abilities,
    )
    .map_err(|error| error.to_string())?;
    let config = DeckMatchConfig {
        shuffle_seed: seed,
        max_turns: 80,
        max_policy_moves: 1600,
        ..DeckMatchConfig::default()
    };
    game.set_shuffle_seed(seed)
        .map_err(|error| error.to_string())?;
    game.load_deck_into_library(PlayerId(0), &p0)
        .map_err(|error| error.to_string())?;
    game.load_deck_into_library(PlayerId(1), &p1)
        .map_err(|error| error.to_string())?;
    game.draw_opening_hand(PlayerId(0), config.opening_hand_size)
        .map_err(|error| error.to_string())?;
    game.draw_opening_hand(PlayerId(1), config.opening_hand_size)
        .map_err(|error| error.to_string())?;
    game.begin_game().map_err(|error| error.to_string())?;
    let mut policies: [Box<dyn CodePolicy>; 2] = [
        Box::new(SphinxPolicy::new(PlayerId(0))),
        Box::new(SphinxPolicy::new(PlayerId(1))),
    ];
    let mut accepted = 0_u32;
    let mut attempted = 0_u32;
    let mut sphinx = None;
    let mut cast_colors = vec![];
    let mut ability_source = None;
    let mut ability_controller = None;
    let mut source_owner = None;
    let mut source_controller_before = None;
    let mut source_zone_after = None;
    let (mut owner_library_len_before, mut owner_library_len_after) = (0, 0);
    let (mut controller_library_len_before, mut controller_library_len_after) = (0, 0);
    let mut ability_cost = None;
    let mut ability_colors = vec![];
    let mut flying = false;
    let mut shuffle_owner = None;
    let mut attacks = 0;
    let mut damage = 0;
    let mut ability_resolved_seen = false;
    let mut post_activation_attacker = None;
    let mut post_activation_attacks = 0;
    let mut post_activation_damage = 0;
    let mut conscription = None;
    let mut conscription_colors = vec![];
    let mut conscription_resolved = false;
    while !game.is_game_over() {
        if game.turn > config.max_turns || accepted >= config.max_policy_moves {
            return Err(format!(
                "Cerulean Sphinx match exceeded bound at turn {} after {accepted} accepted policy moves",
                game.turn
            ));
        }
        let player = game.next_policy_player();
        let view = game
            .view_for_player(player)
            .map_err(|error| error.to_string())?;
        let policy = policies
            .get_mut(player.0)
            .ok_or("Cerulean Sphinx policy seat missing")?;
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
        let activation_source_action = match &action {
            PolicyAction::ActivateAbility { activation } if activation.ability_id == ABILITY_ID => {
                Some(activation.source)
            }
            _ => None,
        };
        let cursor = game.event_log.len();
        attempted += 1;
        game.submit_policy_move(player, policy.id(), action)
            .map_err(|error| {
                format!(
                    "Cerulean Sphinx move rejected at turn {} {:?}: {error}",
                    view.turn, view.step
                )
            })?;
        accepted += 1;
        game.validate_invariants()
            .map_err(|error| format!("Cerulean Sphinx post-move invariant failed: {error}"))?;
        if let Some(source) = activation_source_action {
            ability_source = Some(source);
            if let Ok(object) = game.object(source) {
                ability_controller = Some(player);
                source_owner = Some(object.owner);
                source_controller_before = game.controller_of(source).ok();
                owner_library_len_before = game
                    .player(object.owner)
                    .map(|p| p.library.len())
                    .unwrap_or_default();
                controller_library_len_before = game
                    .player(player)
                    .map(|p| p.library.len())
                    .unwrap_or_default();
                flying = game
                    .characteristics(source)
                    .is_ok_and(|value| value.keywords.contains(&Keyword::Flying));
            }
        }
        for event in &game.event_log[cursor..] {
            match event {
                GameEvent::SpellCast {
                    player: PlayerId(0),
                    card,
                } if sphinx.is_none_or(|known| known == *card)
                    && game
                        .card_definition(*card)
                        .is_ok_and(|known| known.id == CARD_ID) =>
                {
                    sphinx = Some(*card)
                }
                GameEvent::SpellManaPaid {
                    player: PlayerId(0),
                    card,
                    colors,
                } if sphinx.is_none_or(|known| known == *card)
                    && game
                        .card_definition(*card)
                        .is_ok_and(|known| known.id == CARD_ID) =>
                {
                    sphinx = Some(*card);
                    cast_colors = colors.clone();
                }
                GameEvent::SpellCast {
                    player: PlayerId(1),
                    card,
                } if conscription.is_none_or(|known| known == *card)
                    && game
                        .card_definition(*card)
                        .is_ok_and(|known| known.id == CONSCRIPTION_ID) =>
                {
                    conscription = Some(*card)
                }
                GameEvent::SpellManaPaid {
                    player: PlayerId(1),
                    card,
                    colors,
                } if conscription.is_none_or(|known| known == *card)
                    && game
                        .card_definition(*card)
                        .is_ok_and(|known| known.id == CONSCRIPTION_ID) =>
                {
                    conscription = Some(*card);
                    conscription_colors = colors.clone();
                }
                GameEvent::SpellResolved { card } if Some(*card) == conscription => {
                    conscription_resolved = true
                }
                GameEvent::AbilityManaPaid {
                    player: PlayerId(1),
                    source,
                    ability,
                    mana_cost,
                } if Some(*source) == ability_source && *ability == ABILITY_ID => {
                    ability_cost = Some(mana_cost.clone());
                    // The event is an authoritative typed receipt of the blue activation cost.
                    ability_colors = mana_cost.colored.clone();
                }
                GameEvent::AbilityResolved {
                    source, ability, ..
                } if Some(*source) == ability_source && *ability == ABILITY_ID => {
                    ability_resolved_seen = true;
                    source_zone_after = game.zone_of(*source);
                    if let Some(owner) = source_owner {
                        owner_library_len_after = game
                            .player(owner)
                            .map(|p| p.library.len())
                            .unwrap_or_default();
                    }
                    if let Some(controller) = ability_controller {
                        controller_library_len_after = game
                            .player(controller)
                            .map(|p| p.library.len())
                            .unwrap_or_default();
                    }
                }
                GameEvent::LibraryShuffled { player, .. } if Some(*player) == source_owner => {
                    shuffle_owner = Some(*player)
                }
                GameEvent::AttackersDeclared {
                    player: PlayerId(0),
                    attackers,
                } if attackers.iter().any(|attacker| {
                    game.card_definition(*attacker)
                        .is_ok_and(|known| known.id == CARD_ID)
                }) =>
                {
                    attacks += 1;
                    if ability_resolved_seen {
                        let attacking_sphinxes: Vec<_> = attackers
                            .iter()
                            .copied()
                            .filter(|attacker| {
                                game.card_definition(*attacker)
                                    .is_ok_and(|known| known.id == CARD_ID)
                            })
                            .collect();
                        if post_activation_attacker.is_none() {
                            post_activation_attacker = attacking_sphinxes.first().copied();
                        }
                        post_activation_attacks += attacking_sphinxes.len() as u32;
                    }
                }
                GameEvent::DamageDealtToPlayer {
                    source,
                    player: PlayerId(1),
                    amount,
                } if game
                    .card_definition(*source)
                    .is_ok_and(|known| known.id == CARD_ID) =>
                {
                    damage += *amount;
                    if ability_resolved_seen {
                        post_activation_damage += *amount;
                    }
                }
                _ => {}
            }
        }
    }
    game.validate_invariants()
        .map_err(|error| format!("Cerulean Sphinx terminal invariant failed: {error}"))?;
    let winner = game.winner().map(|player| player.0);
    Ok(Captured {
        events: game.event_log.clone(),
        terminal: winner.map_or_else(
            || "Draw".to_owned(),
            |seat| format!("Winner(PlayerId({seat}))"),
        ),
        winner,
        turns: game.turn,
        accepted,
        rejected: attempted.saturating_sub(accepted),
        sphinx,
        cast_colors,
        ability_source,
        ability_controller,
        source_owner,
        source_controller_before,
        source_zone_after,
        owner_library_len_before,
        owner_library_len_after,
        controller_library_len_before,
        controller_library_len_after,
        ability_cost,
        ability_colors,
        flying,
        shuffle_owner,
        attacks,
        damage,
        post_activation_attacker,
        post_activation_attacks,
        post_activation_damage,
        conscription,
        conscription_colors,
        conscription_resolved,
    })
}

struct SphinxPolicy {
    player: PlayerId,
    activation_offered: bool,
    conscription_cast: bool,
}
impl SphinxPolicy {
    fn new(player: PlayerId) -> Self {
        Self {
            player,
            activation_offered: false,
            conscription_cast: false,
        }
    }
}
impl CodePolicy for SphinxPolicy {
    fn id(&self) -> &'static str {
        if self.player == PlayerId(0) {
            "rav.cerulean-sphinx-pilot.v1"
        } else {
            "rav.cerulean-sphinx-opponent.v1"
        }
    }

    fn propose_move(&mut self, view: &GameView) -> PolicyAction {
        if view.player != self.player || view.decision_player != self.player {
            return PolicyAction::PassPriority;
        }
        if view.step == Step::DeclareAttackers
            && view.active_player == self.player
            && !view.attackers_declared
        {
            if self.player == PlayerId(0) {
                return PolicyAction::DeclareAttackers {
                    attackers: view
                        .own_battlefield
                        .iter()
                        .filter(|card| card.definition == Some(CARD_ID) && card.can_attack)
                        .map(|card| card.id)
                        .collect(),
                };
            }
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
        if view.priority != self.player || view.stack_depth != 0 {
            return PolicyAction::PassPriority;
        }
        if self.player == PlayerId(1) && !self.activation_offered {
            if let Some(source) = view
                .own_battlefield
                .iter()
                .find(|card| card.definition == Some(CARD_ID))
            {
                if view.mana_pool.amount(Color::Blue) == 0 {
                    if let Some(land) = view.own_battlefield.iter().find(|card| {
                        card.card_types.contains(&CardType::Land)
                            && !card.tapped
                            && card.mana_colors.contains(&Color::Blue)
                    }) {
                        return PolicyAction::ActivateManaAbility {
                            land: land.id,
                            color: Color::Blue,
                        };
                    }
                } else {
                    self.activation_offered = true;
                    return PolicyAction::ActivateAbility {
                        activation: cardbench_magic_engine::AbilityActivation {
                            source: source.id,
                            ability_id: ABILITY_ID,
                            sacrifice_sources: vec![],
                            additional_tap_creatures: vec![],
                            discard_cards: vec![],
                            targets: vec![],
                        },
                    };
                }
            }
        }
        if !view.active_player.eq(&self.player) || !view.step.is_main() {
            return PolicyAction::PassPriority;
        }
        if view.lands_played == 0 {
            if let Some(land) = view
                .hand
                .iter()
                .find(|card| card.card_types.contains(&CardType::Land))
            {
                return PolicyAction::PlayLand { card: land.id };
            }
        }
        if self.player == PlayerId(0) {
            let on_battlefield = view
                .own_battlefield
                .iter()
                .any(|card| card.definition == Some(CARD_ID));
            if !on_battlefield {
                if let Some(card) = view
                    .hand
                    .iter()
                    .find(|card| card.definition == Some(CARD_ID))
                {
                    let cost = ManaCost::with_colors(4, [Color::Blue, Color::Blue]);
                    if let Some(selection) = payment(view, &cost) {
                        return PolicyAction::CastWithPayment {
                            request: cardbench_magic_engine::CastRequest {
                                card: card.id,
                                targets: vec![],
                                convoke: vec![],
                                payment_mana_abilities: vec![],
                            },
                            chosen_x: None,
                            mana_selection: selection,
                        };
                    }
                    if let Some(action) = make_mana(view, &cost) {
                        return action;
                    }
                }
            }
        } else if !self.conscription_cast {
            if let Some(target) = view
                .opponent_battlefield
                .iter()
                .find(|card| card.definition == Some(CARD_ID))
            {
                let cost = ManaCost::with_colors(5, [Color::Red]);
                if let Some(selection) = payment_preserving_blue(view, &cost) {
                    self.conscription_cast = true;
                    let card = view
                        .hand
                        .iter()
                        .find(|card| card.definition == Some(CONSCRIPTION_ID));
                    if let Some(card) = card {
                        return PolicyAction::CastWithPayment {
                            request: cardbench_magic_engine::CastRequest {
                                card: card.id,
                                targets: vec![Target::Permanent(target.id)],
                                convoke: vec![],
                                payment_mana_abilities: vec![],
                            },
                            chosen_x: None,
                            mana_selection: selection,
                        };
                    }
                } else if let Some(action) = make_mana_preserving_blue(view, &cost) {
                    return action;
                }
            }
        }
        PolicyAction::PassPriority
    }
}

fn payment(view: &GameView, cost: &ManaCost) -> Option<ManaPaymentSelection> {
    let mut available: Vec<_> = Color::MANA_ALL
        .into_iter()
        .map(|color| (color, view.mana_pool.amount(color)))
        .collect();
    for color in &cost.colored {
        let (_, amount) = available.iter_mut().find(|(known, _)| known == color)?;
        if *amount == 0 {
            return None;
        }
        *amount -= 1;
    }
    let mut generic = vec![];
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
fn make_mana(view: &GameView, cost: &ManaCost) -> Option<PolicyAction> {
    let need = cost
        .colored
        .iter()
        .copied()
        .find(|color| view.mana_pool.amount(*color) == 0)
        .or_else(|| {
            (view.mana_pool.total() < cost.generic.saturating_add(cost.colored.len() as u8))
                .then(|| cost.colored.first().copied())
                .flatten()
        });
    let land = view.own_battlefield.iter().find(|card| {
        card.card_types.contains(&CardType::Land)
            && !card.tapped
            && need.is_none_or(|color| card.mana_colors.contains(&color))
    })?;
    let color = need
        .or_else(|| land.mana_colors.iter().copied().next())
        .unwrap_or(Color::Blue);
    Some(PolicyAction::ActivateManaAbility {
        land: land.id,
        color,
    })
}

fn payment_preserving_blue(view: &GameView, cost: &ManaCost) -> Option<ManaPaymentSelection> {
    let mut available: Vec<_> = Color::MANA_ALL
        .into_iter()
        .map(|color| (color, view.mana_pool.amount(color)))
        .collect();
    for color in &cost.colored {
        let (_, amount) = available.iter_mut().find(|(known, _)| known == color)?;
        if *amount == 0 {
            return None;
        }
        *amount -= 1;
    }
    let mut generic = vec![];
    for _ in 0..cost.generic {
        let index = available
            .iter()
            .enumerate()
            .filter(|(_, (color, amount))| *color != Color::Blue && *amount > 0)
            .max_by_key(|(_, (_, amount))| *amount)
            .map(|(index, _)| index)
            .or_else(|| {
                available
                    .iter()
                    .position(|(color, amount)| *color == Color::Blue && *amount > 0)
            })?;
        let (color, amount) = &mut available[index];
        *amount -= 1;
        generic.push(*color);
    }
    let blue_remaining = available
        .iter()
        .find(|(color, _)| *color == Color::Blue)
        .map(|(_, amount)| *amount)
        .unwrap_or_default();
    (blue_remaining > 0).then_some(ManaPaymentSelection {
        generic,
        hybrid: vec![],
    })
}

fn make_mana_preserving_blue(view: &GameView, cost: &ManaCost) -> Option<PolicyAction> {
    if let Some(action) = make_mana(view, cost) {
        return Some(action);
    }
    if let Some(land) = view.own_battlefield.iter().find(|card| {
        card.card_types.contains(&CardType::Land)
            && !card.tapped
            && card.mana_colors.contains(&Color::Blue)
    }) {
        return Some(PolicyAction::ActivateManaAbility {
            land: land.id,
            color: Color::Blue,
        });
    }
    let land = view
        .own_battlefield
        .iter()
        .find(|card| card.card_types.contains(&CardType::Land) && !card.tapped)?;
    let color = land
        .mana_colors
        .iter()
        .copied()
        .next()
        .unwrap_or(Color::Blue);
    Some(PolicyAction::ActivateManaAbility {
        land: land.id,
        color,
    })
}

fn validate_trace(c: &Captured, seed: u64) -> Result<Value, String> {
    if c.terminal != "Winner(PlayerId(0))" || c.winner != Some(0) || c.rejected != 0 {
        return Err("Cerulean Sphinx match did not finish in a clean seat-zero win".to_owned());
    }
    let source = c.sphinx.ok_or("Cerulean Sphinx was not cast")?;
    if c.cast_colors.len() != 6
        || c.cast_colors
            .iter()
            .filter(|color| **color == Color::Blue)
            .count()
            < 2
    {
        return Err(format!(
            "Cerulean Sphinx cast did not spend six mana including both blue requirements: {:?}",
            c.cast_colors
        ));
    }
    let ability_source = c
        .ability_source
        .ok_or("Cerulean Sphinx activation missing")?;
    if source != ability_source
        || c.ability_controller != Some(PlayerId(1))
        || c.source_owner != Some(PlayerId(0))
        || c.source_controller_before != Some(PlayerId(1))
        || c.source_zone_after != Some(Zone::Library)
        || c.shuffle_owner != Some(PlayerId(0))
        || c.owner_library_len_after != c.owner_library_len_before + 1
        || c.controller_library_len_after != c.controller_library_len_before
        || c.ability_cost.as_ref() != Some(&ManaCost::with_colors(0, [Color::Blue]))
        || c.ability_colors != [Color::Blue]
        || !c.flying
        || c.conscription.is_none()
        || c.conscription_colors.len() != 6
        || c.conscription_colors
            .iter()
            .filter(|color| **color == Color::Red)
            .count()
            < 1
        || !c.conscription_resolved
    {
        return Err(format!(
            "Cerulean Sphinx owner-relative activation/control proof differs: owner={:?}, controller={:?}, zone={:?}, owner library {}/{}, controller library {}/{}, activation={:?}, paid={:?}, shuffle={:?}, conscription={:?}/{:?}",
            c.source_owner,
            c.source_controller_before,
            c.source_zone_after,
            c.owner_library_len_before,
            c.owner_library_len_after,
            c.controller_library_len_before,
            c.controller_library_len_after,
            c.ability_cost,
            c.ability_colors,
            c.shuffle_owner,
            c.conscription,
            c.conscription_colors
        ));
    }
    if c.attacks < 3
        || c.damage < 15
        || c.post_activation_attacks == 0
        || c.post_activation_damage == 0
    {
        return Err("Cerulean Sphinx did not finish the full-game flying-combat proof".to_owned());
    }
    let cast_payment = event_index(&c.events, |e| matches!(e, GameEvent::SpellManaPaid { player: PlayerId(0), card, colors } if *card == source && *colors == c.cast_colors)).ok_or("exact Cerulean Sphinx SpellManaPaid receipt missing")?;
    let cast = event_index_after(
        &c.events,
        cast_payment,
        |e| matches!(e, GameEvent::SpellCast { player: PlayerId(0), card } if *card == source),
    )
    .ok_or("Cerulean Sphinx SpellCast receipt missing")?;
    let spell_resolved = event_index_after(
        &c.events,
        cast,
        |e| matches!(e, GameEvent::SpellResolved { card } if *card == source),
    )
    .ok_or("Cerulean Sphinx SpellResolved receipt missing")?;
    let conscription_payment = event_index(&c.events, |e| matches!(e, GameEvent::SpellManaPaid { player: PlayerId(1), card, colors } if Some(*card) == c.conscription && colors == &c.conscription_colors)).ok_or("Flash Conscription exact payment receipt missing")?;
    let conscription_cast = event_index_after(&c.events, conscription_payment, |e| matches!(e, GameEvent::SpellCast { player: PlayerId(1), card } if Some(*card) == c.conscription)).ok_or("Flash Conscription control-change cast receipt missing")?;
    let conscription_resolved = event_index_after(
        &c.events,
        conscription_cast,
        |e| matches!(e, GameEvent::SpellResolved { card } if Some(*card) == c.conscription),
    )
    .ok_or("Flash Conscription control-changing resolution receipt missing")?;
    let ability_paid = event_index_after(&c.events, conscription_resolved, |e| matches!(e, GameEvent::AbilityManaPaid { player: PlayerId(1), source: known, ability, mana_cost } if *known == source && *ability == ABILITY_ID && *mana_cost == ManaCost::with_colors(0, [Color::Blue]))).ok_or("blue activated-ability payment receipt missing")?;
    let activated = event_index_after(&c.events, ability_paid, |e| matches!(e, GameEvent::AbilityActivated { player: PlayerId(1), source: known, ability, .. } if *known == source && *ability == ABILITY_ID)).ok_or("ability activation receipt missing")?;
    let moved = event_index_after(
        &c.events,
        activated,
        |e| matches!(e, GameEvent::CardMoved { card, to: Zone::Library } if *card == source),
    )
    .ok_or("Sphinx owner-library movement receipt missing")?;
    let shuffled = event_index_after(&c.events, moved, |e| {
        matches!(
            e,
            GameEvent::LibraryShuffled {
                player: PlayerId(0),
                ..
            }
        )
    })
    .ok_or("Sphinx owner-library shuffle receipt missing")?;
    let resolved = event_index_after(&c.events, shuffled, |e| matches!(e, GameEvent::AbilityResolved { source: known, ability, .. } if *known == source && *ability == ABILITY_ID)).ok_or("ability resolution receipt missing")?;
    let combat_source = c
        .post_activation_attacker
        .ok_or("post-activation Sphinx attack source missing")?;
    let attack = event_index_after(&c.events, resolved, |e| matches!(e, GameEvent::AttackersDeclared { player: PlayerId(0), attackers } if attackers.contains(&combat_source))).ok_or("post-activation Sphinx combat missing")?;
    let ended = event_index_after(&c.events, attack, |e| {
        matches!(
            e,
            GameEvent::GameEnded {
                winner: Some(PlayerId(0))
            }
        )
    })
    .ok_or("terminal PvP win receipt missing")?;
    let events = project_events(&c.events);
    Ok(
        json!({"trace_id":"rav-cerulean-sphinx-owner-library-shuffle-full-game","shuffle_seed":seed,"terminal":c.terminal,"winner_seat":0,"turns":c.turns,"accepted_moves":c.accepted,"rejected_moves":c.rejected,
        "mechanics":{"sphinx_object":source.0,"mana_cost":"{4}{U}{U}","paid_mana_colors":c.cast_colors.iter().map(|color|format!("{color:?}")).collect::<Vec<_>>(),"power":5,"toughness":5,"flying":c.flying,
            "ability_id":ABILITY_ID,"ability_mana_cost":"{U}","activation_mana_colors":c.ability_colors.iter().map(|color|format!("{color:?}")).collect::<Vec<_>>(),"ability_controller":1,"source_owner":0,"source_controller_before_activation":1,"source_zone_after_resolution":"Library","shuffle_owner":0,
            "owner_library_count_before":c.owner_library_len_before,"owner_library_count_after":c.owner_library_len_after,"controller_library_count_before":c.controller_library_len_before,"controller_library_count_after":c.controller_library_len_after,
            "flash_conscription_object":c.conscription.map(|value|value.0),"flash_conscription_paid_mana_colors":c.conscription_colors.iter().map(|color|format!("{color:?}")).collect::<Vec<_>>(),"flash_conscription_resolved":c.conscription_resolved,
            "attack_count":c.attacks,"damage_to_opponent":c.damage,"post_activation_attacker_object":c.post_activation_attacker.map(|value|value.0),"post_activation_attack_count":c.post_activation_attacks,"post_activation_damage":c.post_activation_damage,"deterministic_replay":true,"terminal_combat_win":true},
        "ordered_event_sequence":[cast_payment,cast,spell_resolved,conscription_payment,conscription_cast,conscription_resolved,ability_paid,activated,moved,shuffled,resolved,attack,ended],"events":events}),
    )
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

fn candidate_kind() -> &'static str {
    if std::env::args().any(|arg| arg == "--candidate") {
        "agent"
    } else {
        "reference"
    }
}

fn card_status(card: &CatalogCard) -> Value {
    let definition_id = match card.semantic_status {
        CardSemanticStatus::ExecutableCompatibilitySlice { definition_id } => Some(definition_id),
        CardSemanticStatus::CatalogOnly { .. } => None,
    };
    let passed = card.collector_number == 39;
    json!({"collector_number":card.collector_number,"name":card.name,"definition_id":definition_id,"engine_status":if definition_id.is_some(){"executable_compatibility_slice"}else{"catalog_only"},"trace_status":if passed{"passed"}else{"not_qualified"},"qualification":if passed{"diagnostic_only"}else{"not_evaluated"},"credited":false})
}
