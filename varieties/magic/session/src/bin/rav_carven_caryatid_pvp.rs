//! Full-game diagnostic authority for the RAV Carven Caryatid task.

use cardbench_magic_engine::{
    CardType, Color, DeckEntry, DeckList, Effect, Game, GameEvent, GameView, Keyword, ManaCost,
    ManaPaymentSelection, ObjectId, PlayerId, PolicyAction, Step, TriggerCondition,
    TriggeredAbility, TriggeredAbilityBinding, Zone,
};
use cardbench_magic_policies::{CodePolicy, DeckMatchConfig};
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
use syn::{Expr, Item, Stmt, UseTree, Visibility};

const TASK_ID: &str = "cardbench/magic/card";
const CARD_ID: &str = "RAV-CARVEN-CARYATID";
const WATCHWOLF_ID: &str = "RAV-WATCHWOLF";
const TRIGGER_ID: &str = "etb-draw-controller";
const SOURCE_PATH: &str =
    "varieties/magic/sets/ravnica_city_of_guilds/src/cards/carven_caryatid.rs";
const MAX_TRACE_SEED: u64 = 64;

#[path = "../../../sets/ravnica_city_of_guilds/src/cards/carven_caryatid.rs"]
mod candidate_carven_caryatid;

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
            eprintln!("Carven Caryatid PvP authority failed: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run_authority() -> Result<Value, String> {
    validate_candidate_source()?;
    validate_candidate_bindings()?;
    let catalog = unique_catalog()?;
    let seed = find_trace_seed()?;
    let captured = run_match(seed, Mutation::None)?;
    validate_trace(&captured)?;
    let replay = run_match(seed, Mutation::None)?;
    if !same_match(&captured, &replay) {
        return Err("Carven Caryatid shuffled full-game replay was not deterministic".to_owned());
    }
    let mutation_sensitivity = json!({
        "etb_draw_trigger_removed_mutation_detected_in_game": mutation_detected(seed, Mutation::NoTrigger),
        "defender_removed_mutation_detected_by_attack_legality": mutation_detected(seed, Mutation::NoDefender),
        "toughness_mutation_detected_in_game": mutation_detected(seed, Mutation::WrongToughness),
        "sealed_mutation_authority": false
    });
    if mutation_sensitivity["etb_draw_trigger_removed_mutation_detected_in_game"] != true
        || mutation_sensitivity["defender_removed_mutation_detected_by_attack_legality"] != true
        || mutation_sensitivity["toughness_mutation_detected_in_game"] != true
    {
        return Err(format!(
            "a Carven Caryatid semantic mutation escaped the gameplay oracle: {mutation_sensitivity}"
        ));
    }
    let breakdown: Vec<_> = catalog.iter().map(card_status).collect();
    Ok(json!({
        "schema_version": "cardbench.magic.card-pvp.check.v1",
        "task_id": TASK_ID,
        "expansion": "RAV",
        "card": {"collector_number": 155, "definition_id": CARD_ID, "name": "Carven Caryatid"},
        "candidate": {"kind": candidate_kind(), "card_definition": CARD_ID},
        "candidate_kind": candidate_kind(),
        "viewer_scope": format!("{:?}", ViewerScope::Judge),
        "rules_authority": {
            "comprehensive_rules": "2026-08-07",
            "checked_sections": [
                "117: timing and priority",
                "302.1 and 601.2: creature spells and casting",
                "603.2 and 603.3: triggered abilities",
                "608.2: resolving spells and abilities",
                "702.3: Defender",
                "120.2 and 121: damage and card drawing"
            ],
            "source_contract": "RAV Carven Caryatid is a green 2/5 Creature for {1}{G}{G} with Defender; its mandatory ETB trigger draws a card for its controller.",
            "authority_cutoff": "2026-09-23"
        },
        "passed": true,
        "pilot_card_passed": true,
        "cards_passed": 1,
        "cards_credited": 0,
        "cards_total": catalog.len(),
        "printings_total": rav_main_set_catalog().len(),
        "coverage_complete": false,
        "full_reward": false,
        "harbor_reward": 0.0,
        "pilot_reward": 0.0,
        "qualification_status": "unreviewed_diagnostic",
        "mutation_sensitivity": mutation_sensitivity,
        "traces": [trace_json(&captured)],
        "per_card_breakdown": breakdown
    }))
}

fn validate_candidate_source() -> Result<(), String> {
    let source = std::fs::read_to_string(SOURCE_PATH)
        .map_err(|error| format!("could not read Carven Caryatid source: {error}"))?;
    let file = syn::parse_file(&source)
        .map_err(|_| "Carven Caryatid source is not valid Rust".to_owned())?;
    if !file
        .attrs
        .iter()
        .all(|attribute| attribute.path().is_ident("doc"))
    {
        return Err("Carven Caryatid source has an unsupported module attribute".to_owned());
    }
    let expected_imports: BTreeSet<Vec<String>> = [
        vec!["std", "collections", "BTreeSet"],
        vec!["cardbench_magic_engine", "CardDefinition"],
        vec!["cardbench_magic_engine", "CardType"],
        vec!["cardbench_magic_engine", "Color"],
        vec!["cardbench_magic_engine", "Effect"],
        vec!["cardbench_magic_engine", "Keyword"],
        vec!["cardbench_magic_engine", "ManaCost"],
        vec!["cardbench_magic_engine", "TriggerCondition"],
        vec!["cardbench_magic_engine", "TriggeredAbility"],
        vec!["cardbench_magic_engine", "TriggeredAbilityBinding"],
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
                    return Err("Carven Caryatid source has unsupported imports".to_owned());
                }
                imports.extend(paths);
            }
            Item::Fn(item)
                if matches!(&item.vis, Visibility::Restricted(vis) if vis.path.is_ident("crate"))
                    && item.sig.inputs.is_empty()
                    && item.sig.generics.params.is_empty()
                    && item.sig.generics.where_clause.is_none()
                    && item.sig.constness.is_none()
                    && item.sig.asyncness.is_none()
                    && item.sig.unsafety.is_none()
                    && item.sig.abi.is_none()
                    && item.sig.variadic.is_none()
                    && item.block.stmts.len() == 1 =>
            {
                let output = match item.sig.ident.to_string().as_str() {
                    "definition" => "CardDefinition",
                    "triggered_ability" => "TriggeredAbilityBinding",
                    _ => return Err("Carven Caryatid has an unsupported function".to_owned()),
                };
                let Stmt::Expr(Expr::Struct(literal), None) = &item.block.stmts[0] else {
                    return Err(
                        "Carven Caryatid functions must return direct typed literals".to_owned(),
                    );
                };
                if !type_is(&item.sig.output, output)
                    || !path_is_type(&literal.path, output)
                    || literal.rest.is_some()
                    || !literal.attrs.is_empty()
                {
                    return Err(
                        "Carven Caryatid functions must return direct typed literals".to_owned(),
                    );
                }
                if !functions.insert(item.sig.ident.to_string()) {
                    return Err("Carven Caryatid has duplicate functions".to_owned());
                }
            }
            _ => {
                return Err(
                    "Carven Caryatid source contains an unsupported top-level item".to_owned(),
                );
            }
        }
    }
    if imports != expected_imports
        || functions != BTreeSet::from(["definition".to_owned(), "triggered_ability".to_owned()])
        || file.items.len() != 4
    {
        return Err("Carven Caryatid module structure differs from the source contract".to_owned());
    }
    Ok(())
}

fn expected_trigger() -> TriggeredAbilityBinding {
    TriggeredAbilityBinding {
        card_definition: CARD_ID,
        ability: TriggeredAbility {
            id: TRIGGER_ID,
            condition: TriggerCondition::EntersBattlefield,
            mana_cost: ManaCost::new(0),
            optional: false,
            targets: vec![],
            effects: vec![Effect::DrawController],
        },
    }
}

fn validate_candidate_bindings() -> Result<(), String> {
    let definition = candidate_carven_caryatid::definition();
    let registered = card_definitions()
        .into_iter()
        .find(|card| card.id == CARD_ID)
        .ok_or("Carven Caryatid definition is missing from the RAV registry")?;
    if definition != registered
        || definition.name != "Carven Caryatid"
        || definition.set_code != "RAV"
        || definition.mana_cost != ManaCost::with_colors(1, [Color::Green, Color::Green])
        || definition.colors != BTreeSet::from([Color::Green])
        || !definition.mana_colors.is_empty()
        || definition.card_types != BTreeSet::from([CardType::Creature])
        || definition.is_basic_land
        || definition.power != Some(2)
        || definition.toughness != Some(5)
        || definition.keywords != [Keyword::Defender]
        || !definition.effects.is_empty()
    {
        return Err("Carven Caryatid literal is outside the task source contract".to_owned());
    }
    let binding = candidate_carven_caryatid::triggered_ability();
    let registered = rav_triggered_ability_bindings()
        .into_iter()
        .find(|item| item.card_definition == CARD_ID)
        .ok_or("Carven Caryatid ETB draw trigger is missing from the RAV registry")?;
    if binding != registered || binding != expected_trigger() {
        return Err(
            "Carven Caryatid trigger literal is outside the task source contract".to_owned(),
        );
    }
    Ok(())
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
        return Err("RAV catalog denominator differs from the source contract".to_owned());
    }
    Ok(catalog)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Mutation {
    None,
    NoTrigger,
    NoDefender,
    WrongToughness,
}

struct CapturedMatch {
    events: Vec<GameEvent>,
    terminal: String,
    winner: Option<usize>,
    turns: u32,
    accepted_moves: u32,
    rejected_moves: u32,
    candidate: Option<ObjectId>,
    drawn_card: Option<ObjectId>,
    drawn_definition: Option<String>,
    power: Option<i32>,
    toughness: Option<i32>,
    live_defender: bool,
    probe_rejection: Option<String>,
    probe_unchanged: bool,
    candidate_mana: Vec<Color>,
    attacker: Option<ObjectId>,
    combat_damage: i32,
}

fn find_trace_seed() -> Result<u64, String> {
    let mut last_error = "no trace attempt".to_owned();
    for seed in 0..MAX_TRACE_SEED {
        match run_match(seed, Mutation::None) {
            Ok(capture) => match validate_trace(&capture) {
                Ok(()) => return Ok(seed),
                Err(error) => last_error = format!("seed {seed}: {error}"),
            },
            Err(error) => last_error = format!("seed {seed}: {error}"),
        }
    }
    Err(format!(
        "no bounded shuffled deck completed Carven Caryatid's defender and ETB draw trace; {last_error}"
    ))
}

fn run_match(seed: u64, mutation: Mutation) -> Result<CapturedMatch, String> {
    let mut definitions = card_definitions();
    let definition = definitions
        .iter_mut()
        .find(|card| card.id == CARD_ID)
        .ok_or("Carven Caryatid definition missing from registry")?;
    *definition = candidate_carven_caryatid::definition();
    match mutation {
        Mutation::None => {}
        Mutation::NoDefender => definition
            .keywords
            .retain(|keyword| *keyword != Keyword::Defender),
        Mutation::WrongToughness => definition.toughness = Some(4),
        Mutation::NoTrigger => {}
    }
    let mut triggers = rav_triggered_ability_bindings();
    let index = triggers
        .iter()
        .position(|binding| binding.card_definition == CARD_ID)
        .ok_or("Carven Caryatid trigger missing from registry")?;
    triggers[index] = candidate_carven_caryatid::triggered_ability();
    if mutation == Mutation::NoTrigger {
        triggers.remove(index);
    }
    triggers.extend(attachment_triggers());
    let mut game = Game::new_with_all_bindings_triggers_static_continuous_effects_and_land_entries(
        definitions,
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        rav_activated_ability_bindings(),
        triggers,
        rav_static_continuous_effect_bindings(),
        rav_land_entry_bindings(),
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
    game.validate_invariants()
        .map_err(|error| error.to_string())?;

    let deck0 = DeckList {
        mainboard: vec![
            DeckEntry {
                card: CARD_ID.into(),
                count: 4,
            },
            DeckEntry {
                card: WATCHWOLF_ID.into(),
                count: 4,
            },
            DeckEntry {
                card: "RAV-FOREST".into(),
                count: 28,
            },
            DeckEntry {
                card: "RAV-PLAINS".into(),
                count: 24,
            },
        ],
        sideboard: vec![],
    };
    let deck1 = DeckList {
        mainboard: vec![DeckEntry {
            card: "RAV-FOREST".into(),
            count: 60,
        }],
        sideboard: vec![],
    };
    let config = DeckMatchConfig {
        shuffle_seed: seed,
        max_turns: 80,
        max_policy_moves: 8_000,
        ..DeckMatchConfig::default()
    };
    game.set_shuffle_seed(seed)
        .map_err(|error| error.to_string())?;
    game.load_deck_into_library(PlayerId(0), &deck0)
        .map_err(|error| error.to_string())?;
    game.load_deck_into_library(PlayerId(1), &deck1)
        .map_err(|error| error.to_string())?;
    game.draw_opening_hand(PlayerId(0), config.opening_hand_size)
        .map_err(|error| error.to_string())?;
    game.draw_opening_hand(PlayerId(1), config.opening_hand_size)
        .map_err(|error| error.to_string())?;
    game.begin_game().map_err(|error| error.to_string())?;
    let mut policies: [Box<dyn CodePolicy>; 2] = [
        Box::new(CaryatidPolicy::new(PlayerId(0))),
        Box::new(CaryatidPolicy::new(PlayerId(1))),
    ];
    let mut accepted = 0_u32;
    let mut attempted = 0_u32;
    let mut candidate = None;
    let mut live = None;
    let mut probe = None;
    let mut probe_unchanged = false;
    while !game.is_game_over() {
        if game.turn > config.max_turns || accepted >= config.max_policy_moves {
            return Err(format!(
                "Carven Caryatid match exceeded bounds at turn {}",
                game.turn
            ));
        }
        let player = game.next_policy_player();
        let view = game
            .view_for_player(player)
            .map_err(|error| error.to_string())?;
        if player == PlayerId(0)
            && view.step == Step::DeclareAttackers
            && view.active_player == player
            && !view.attackers_declared
        {
            if let Some(defender) = view
                .own_battlefield
                .iter()
                .find(|card| card.definition == Some(CARD_ID))
            {
                if probe.is_none() {
                    let before_events = game.event_log.len();
                    let before_view = view.clone();
                    let attempted_attack = PolicyAction::DeclareAttackers {
                        attackers: vec![defender.id],
                    };
                    match game.submit_policy_move(
                        player,
                        "rav.carven-caryatid-defender-probe.v1",
                        attempted_attack,
                    ) {
                        Ok(()) => {
                            return Err("Defender Caryatid attack probe was accepted".to_owned());
                        }
                        Err(error) => {
                            let message = error.to_string();
                            if !message.contains("illegal attacker") {
                                return Err(format!(
                                    "unexpected Defender probe rejection: {message}"
                                ));
                            }
                            let unchanged = game.event_log.len() == before_events
                                && game
                                    .view_for_player(player)
                                    .map_err(|error| error.to_string())?
                                    == before_view;
                            if !unchanged {
                                return Err(
                                    "rejected Defender legality probe changed the live match"
                                        .to_owned(),
                                );
                            }
                            probe = Some(message);
                            probe_unchanged = unchanged;
                        }
                    }
                    continue;
                }
            }
        }
        let policy = policies
            .get_mut(player.0)
            .ok_or("Carven Caryatid policy seat missing")?;
        let action = if let Some(action) = policy.propose_pending_decision(&view) {
            action
        } else if view.optional_triggered_ability_choice.is_some() {
            policy.propose_optional_triggered_ability(&view)
        } else if view.draw_replacement_pending {
            policy.propose_draw_replacement(&view)
        } else {
            policy.propose_move(&view)
        };
        attempted += 1;
        let action_summary = format!("{action:?}");
        game.submit_policy_move(player, policy.id(), action)
            .map_err(|error| {
                format!(
                    "policy move rejected at turn {} {:?}: {error}; action={action_summary}; mana={:?}",
                    view.turn, view.step, view.mana_pool
                )
            })?;
        accepted += 1;
        game.validate_invariants()
            .map_err(|error| error.to_string())?;
        if candidate.is_none() {
            candidate = game.event_log.iter().find_map(|event| match event {
                GameEvent::SpellCast {
                    player: PlayerId(0),
                    card,
                } if game
                    .card_definition(*card)
                    .is_ok_and(|definition| definition.id == CARD_ID) =>
                {
                    Some(*card)
                }
                _ => None,
            });
        }
        if let Some(id) = candidate
            && live.is_none()
            && game.zone_of(id) == Some(Zone::Battlefield)
        {
            let characteristics = game
                .characteristics(id)
                .map_err(|error| error.to_string())?;
            live = Some((
                characteristics.power.unwrap_or_default(),
                characteristics.toughness.unwrap_or_default(),
                characteristics.keywords.contains(&Keyword::Defender),
            ));
        }
    }
    let drawn_card = candidate.and_then(|candidate| {
        let stacked = game.event_log.iter().position(|event| matches!(event, GameEvent::TriggeredAbilityStacked { source, ability: TRIGGER_ID, .. } if *source == candidate))?;
        let resolved = game.event_log.iter().position(|event| matches!(event, GameEvent::AbilityResolved { source, ability: TRIGGER_ID, .. } if *source == candidate))?;
        game.event_log[stacked + 1..resolved].iter().find_map(|event| match event { GameEvent::CardMoved { card, to: Zone::Hand } => Some(*card), _ => None })
    });
    let drawn_definition = drawn_card
        .map(|card| {
            game.card_definition(card)
                .map(|definition| definition.id.to_owned())
        })
        .transpose()
        .map_err(|error| error.to_string())?;
    let candidate_mana = candidate
        .and_then(|candidate| {
            game.event_log.iter().find_map(|event| match event {
                GameEvent::SpellManaPaid {
                    player: PlayerId(0),
                    card,
                    colors,
                } if *card == candidate => Some(colors.clone()),
                _ => None,
            })
        })
        .unwrap_or_default();
    let attacker = game.event_log.iter().find_map(|event| match event {
        GameEvent::AttackersDeclared {
            player: PlayerId(0),
            attackers,
        } => attackers
            .iter()
            .find(|card| {
                game.card_definition(**card)
                    .is_ok_and(|definition| definition.id == WATCHWOLF_ID)
            })
            .copied(),
        _ => None,
    });
    let combat_damage = game
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
    let winner = game.winner().map(|player| player.0);
    let (power, toughness, live_defender) = live
        .map_or((None, None, false), |(power, toughness, defender)| {
            (Some(power), Some(toughness), defender)
        });
    Ok(CapturedMatch {
        events: game.event_log.clone(),
        terminal: winner.map_or_else(
            || "Draw".to_owned(),
            |seat| format!("Winner(PlayerId({seat}))"),
        ),
        winner,
        turns: game.turn,
        accepted_moves: accepted,
        rejected_moves: attempted.saturating_sub(accepted),
        candidate,
        drawn_card,
        drawn_definition,
        power,
        toughness,
        live_defender,
        probe_rejection: probe,
        probe_unchanged,
        candidate_mana,
        attacker,
        combat_damage,
    })
}

struct CaryatidPolicy {
    player: PlayerId,
}
impl CaryatidPolicy {
    fn new(player: PlayerId) -> Self {
        Self { player }
    }
    fn activate_color(&self, view: &GameView, color: Color) -> Option<PolicyAction> {
        view.own_battlefield
            .iter()
            .find(|land| {
                !land.tapped
                    && land.card_types.contains(&CardType::Land)
                    && land.mana_colors.contains(&color)
            })
            .map(|land| PolicyAction::ActivateManaAbility {
                land: land.id,
                color,
            })
    }
    fn make_any(&self, view: &GameView) -> Option<PolicyAction> {
        self.activate_color(view, Color::White)
            .or_else(|| self.activate_color(view, Color::Green))
    }
    fn pay_colors(&self, card: ObjectId, generic: Vec<Color>) -> PolicyAction {
        PolicyAction::CastWithPayment {
            request: cardbench_magic_engine::CastRequest {
                card,
                targets: vec![],
                convoke: vec![],
                payment_mana_abilities: vec![],
            },
            chosen_x: None,
            mana_selection: ManaPaymentSelection {
                generic,
                hybrid: vec![],
            },
        }
    }
}
impl CodePolicy for CaryatidPolicy {
    fn id(&self) -> &'static str {
        "rav.carven-caryatid-defender-etb-draw.v1"
    }
    fn propose_move(&mut self, view: &GameView) -> PolicyAction {
        if view.player != self.player {
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
                    .filter(|card| card.definition == Some(WATCHWOLF_ID) && card.can_attack)
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
        if view.decision_player != self.player
            || view.priority != self.player
            || view.active_player != self.player
            || view.step != Step::PrecombatMain
            || view.stack_depth != 0
        {
            return PolicyAction::PassPriority;
        }
        if view.lands_played == 0 {
            let forests = view
                .own_battlefield
                .iter()
                .filter(|land| {
                    land.card_types.contains(&CardType::Land)
                        && land.mana_colors.contains(&Color::Green)
                })
                .count();
            let preferred = if forests < 2 {
                "RAV-FOREST"
            } else {
                "RAV-PLAINS"
            };
            if let Some(land) = view
                .hand
                .iter()
                .find(|card| card.definition == Some(preferred))
                .or_else(|| {
                    view.hand
                        .iter()
                        .find(|card| card.card_types.contains(&CardType::Land))
                })
            {
                return PolicyAction::PlayLand { card: land.id };
            }
        }
        if self.player == PlayerId(0) {
            let candidate_on_battlefield = view
                .own_battlefield
                .iter()
                .any(|card| card.definition == Some(CARD_ID));
            if !candidate_on_battlefield {
                if let Some(card) = view
                    .hand
                    .iter()
                    .find(|card| card.definition == Some(CARD_ID))
                {
                    let green = view.mana_pool.amount(Color::Green);
                    if green < 2 {
                        if let Some(action) = self.activate_color(view, Color::Green) {
                            return action;
                        }
                    }
                    if view.mana_pool.total() < 3 {
                        if let Some(action) = self.make_any(view) {
                            return action;
                        }
                    }
                    if green >= 2 && view.mana_pool.total() >= 3 {
                        let generic = if view.mana_pool.amount(Color::White) > 0 {
                            Color::White
                        } else {
                            Color::Green
                        };
                        return self.pay_colors(card.id, vec![generic]);
                    }
                }
            } else if let Some(card) = view
                .hand
                .iter()
                .find(|card| card.definition == Some(WATCHWOLF_ID))
            {
                let green = view.mana_pool.amount(Color::Green);
                let white = view.mana_pool.amount(Color::White);
                if green == 0 {
                    if let Some(action) = self.activate_color(view, Color::Green) {
                        return action;
                    }
                }
                if white == 0 {
                    if let Some(action) = self.activate_color(view, Color::White) {
                        return action;
                    }
                }
                if green > 0 && white > 0 {
                    return self.pay_colors(card.id, vec![]);
                }
            }
        }
        PolicyAction::PassPriority
    }
}

fn validate_trace(capture: &CapturedMatch) -> Result<(), String> {
    if capture.terminal != "Winner(PlayerId(0))" || capture.rejected_moves != 0 {
        return Err("Carven Caryatid duel did not finish as a clean P0 win".to_owned());
    }
    let candidate = capture.candidate.ok_or("Carven Caryatid was not cast")?;
    if capture.power != Some(2) || capture.toughness != Some(5) || !capture.live_defender {
        return Err("Carven Caryatid live 2/5 Defender characteristics differ".to_owned());
    }
    let trigger_stacked = capture.events.iter().any(|event| matches!(event, GameEvent::TriggeredAbilityStacked { controller: PlayerId(0), source, ability: TRIGGER_ID, .. } if *source == candidate));
    let trigger_resolved = capture.events.iter().any(|event| matches!(event, GameEvent::AbilityResolved { source, ability: TRIGGER_ID, .. } if *source == candidate));
    let draw_moved = capture.drawn_card.is_some_and(|card| capture.events.iter().any(|event| matches!(event, GameEvent::CardMoved { card: moved, to: Zone::Hand } if *moved == card)));
    if !trigger_stacked || !trigger_resolved || !draw_moved || capture.drawn_definition.is_none() {
        return Err(
            "Carven Caryatid ETB trigger did not move the drawn card into its controller's hand"
                .to_owned(),
        );
    }
    if capture.candidate_mana.len() != 3
        || capture
            .candidate_mana
            .iter()
            .filter(|color| **color == Color::Green)
            .count()
            != 2
    {
        return Err("Carven Caryatid did not pay exact {1}{G}{G} mana".to_owned());
    }
    if !capture.probe_unchanged
        || !capture
            .probe_rejection
            .as_deref()
            .is_some_and(|message| message.contains("illegal attacker"))
    {
        return Err("Defender attack probe did not reject atomically".to_owned());
    }
    if capture.events.iter().any(|event| matches!(event, GameEvent::AttackersDeclared { attackers, .. } if attackers.contains(&candidate))) { return Err("Carven Caryatid was declared as an attacker despite Defender".to_owned()); }
    if capture.attacker.is_none() || capture.combat_damage < 20 {
        return Err(format!(
            "the full game did not exercise legal Watchwolf combat through victory: winner={:?}, attacker={:?}, damage={}, turns={}",
            capture.winner, capture.attacker, capture.combat_damage, capture.turns
        ));
    }
    Ok(())
}

fn mutation_detected(seed: u64, mutation: Mutation) -> bool {
    run_match(seed, mutation)
        .and_then(|capture| validate_trace(&capture))
        .is_err()
}
fn same_match(a: &CapturedMatch, b: &CapturedMatch) -> bool {
    a.events == b.events
        && a.terminal == b.terminal
        && a.winner == b.winner
        && a.turns == b.turns
        && a.accepted_moves == b.accepted_moves
        && a.rejected_moves == b.rejected_moves
        && a.candidate == b.candidate
        && a.drawn_card == b.drawn_card
        && a.drawn_definition == b.drawn_definition
        && a.power == b.power
        && a.toughness == b.toughness
        && a.live_defender == b.live_defender
        && a.probe_rejection == b.probe_rejection
        && a.probe_unchanged == b.probe_unchanged
        && a.candidate_mana == b.candidate_mana
        && a.attacker == b.attacker
        && a.combat_damage == b.combat_damage
}

fn trace_json(capture: &CapturedMatch) -> Value {
    let candidate = capture.candidate;
    let draw_index = candidate.and_then(|candidate| capture.events.iter().position(|event| matches!(event, GameEvent::TriggeredAbilityStacked { source, ability: TRIGGER_ID, .. } if *source == candidate)));
    let trigger_resolved_index = candidate.and_then(|candidate| capture.events.iter().position(|event| matches!(event, GameEvent::AbilityResolved { source, ability: TRIGGER_ID, .. } if *source == candidate)));
    let draw_event_index = draw_index
        .zip(trigger_resolved_index)
        .and_then(|(start, end)| {
            capture.events[start + 1..end]
                .iter()
                .position(|event| matches!(event, GameEvent::CardMoved { to: Zone::Hand, .. }))
                .map(|offset| start + 1 + offset)
        });
    let watchwolf_cast = capture.attacker;
    let first_attack = capture.events.iter().position(|event| matches!(event, GameEvent::AttackersDeclared { player: PlayerId(0), attackers } if watchwolf_cast.is_some_and(|attacker| attackers.contains(&attacker))));
    let damage_index = first_attack.and_then(|start| capture.events.iter().enumerate().skip(start + 1).find_map(|(index, event)| matches!(event, GameEvent::DamageDealtToPlayer { source, player: PlayerId(1), .. } if watchwolf_cast == Some(*source)).then_some(index)));
    let mut indices = Vec::new();
    if let Some(candidate) = candidate {
        indices.extend(
            capture
                .events
                .iter()
                .enumerate()
                .filter_map(|(i, event)| match event {
                    GameEvent::SpellManaPaid {
                        player: PlayerId(0),
                        card,
                        ..
                    } if *card == candidate => Some(i),
                    GameEvent::SpellCast {
                        player: PlayerId(0),
                        card,
                    }
                    | GameEvent::SpellResolved { card }
                        if *card == candidate =>
                    {
                        Some(i)
                    }
                    GameEvent::CardMoved {
                        card,
                        to: Zone::Battlefield,
                    } if *card == candidate => Some(i),
                    GameEvent::TriggeredAbilityStacked {
                        source,
                        ability: TRIGGER_ID,
                        ..
                    }
                    | GameEvent::AbilityResolved {
                        source,
                        ability: TRIGGER_ID,
                        ..
                    } if *source == candidate => Some(i),
                    GameEvent::GameEnded { .. } => Some(i),
                    _ => None,
                }),
        );
    }
    if let Some(index) = draw_event_index {
        indices.push(index);
    }
    if let Some(index) = first_attack {
        indices.push(index);
    }
    if let Some(index) = damage_index {
        indices.push(index);
    }
    indices.sort_unstable();
    indices.dedup();
    let projected = project_events(&capture.events);
    let events: Vec<Value> = indices.into_iter().filter_map(|index| {
        let event: &ProjectedEvent = projected.get(index)?;
        Some(json!({"sequence": event.sequence, "turn": event.turn, "kind": event.kind, "facts": event.facts, "canonical": event.canonical}))
    }).collect();
    json!({
        "trace_id": "rav-carven-caryatid-etb-draw-defender-combat",
        "terminal": capture.terminal,
        "winner": capture.winner,
        "turns": capture.turns,
        "accepted_policy_moves": capture.accepted_moves,
        "rejected_policy_moves": capture.rejected_moves,
        "mechanics": {
            "shuffled_60_card_decks": true,
            "mana_cost": "{1}{G}{G}",
            "paid_mana_colors": capture.candidate_mana.iter().map(|color| format!("{color:?}")).collect::<Vec<_>>(),
            "live_power_toughness": [capture.power, capture.toughness],
            "live_defender": capture.live_defender,
            "triggered_draw_definition": capture.drawn_definition,
            "draw_moved_to_hand": capture.drawn_card.is_some(),
            "defender_attack_was_rejected": capture.probe_rejection.is_some(),
            "rejected_probe_left_state_unchanged": capture.probe_unchanged,
            "caryatid_attacked": capture.events.iter().any(|event| matches!(event, GameEvent::AttackersDeclared { attackers, .. } if candidate.is_some_and(|card| attackers.contains(&card)))),
            "watchwolf_combat_damage": capture.combat_damage
        },
        "legality_probe": {"attacker_live_defender": capture.live_defender, "rejected_by_engine": capture.probe_rejection.is_some(), "match_state_unchanged": capture.probe_unchanged, "rejection": capture.probe_rejection},
        "events": events
    })
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
    let passed = card.collector_number == 155;
    json!({"collector_number": card.collector_number, "name": card.name, "definition_id": definition_id, "engine_status": if definition_id.is_some() { "executable_compatibility_slice" } else { "catalog_only" }, "trace_status": if passed { "passed" } else { "not_qualified" }, "qualification": if passed { "unreviewed_diagnostic" } else { "not_evaluated" }, "credited": false})
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
