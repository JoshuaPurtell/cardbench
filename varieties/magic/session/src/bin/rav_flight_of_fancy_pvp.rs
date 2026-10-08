//! Full-game diagnostic authority for the RAV Flight of Fancy task.

use cardbench_magic_engine::{
    CardType, Color, DeckEntry, DeckList, Effect, Game, GameEvent, GameView, Keyword, ManaCost,
    ManaPaymentSelection, ObjectId, PlayerId, PolicyAction, Step, Target, TargetRequirement,
    TriggerCondition, Zone,
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
const CARD_ID: &str = "RAV-FLIGHT-OF-FANCY";
const CARD_NAME: &str = "Flight of Fancy";
const WATCHWOLF_ID: &str = "RAV-WATCHWOLF";
const TRIGGER_ID: &str = "etb-draw-two";
const SOURCE_PATH: &str =
    "varieties/magic/sets/ravnica_city_of_guilds/src/cards/flight_of_fancy.rs";
const MAX_TRACE_SEED: u64 = 64;

#[path = "../../../sets/ravnica_city_of_guilds/src/cards/flight_of_fancy.rs"]
mod candidate_flight_of_fancy;

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
            eprintln!("Flight of Fancy PvP authority failed: {error}");
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
        return Err("Flight of Fancy shuffled full-game replay was not deterministic".to_owned());
    }
    let mutation_sensitivity = json!({
        "flying_omitted_mutation_detected_in_game": mutation_detected(seed, Mutation::NoFlying),
        "etb_trigger_removed_mutation_detected_in_game": mutation_detected(seed, Mutation::NoTrigger),
        "draw_count_changed_mutation_detected_in_game": mutation_detected(seed, Mutation::DrawOne),
        "sealed_mutation_authority": false
    });
    for key in [
        "flying_omitted_mutation_detected_in_game",
        "etb_trigger_removed_mutation_detected_in_game",
        "draw_count_changed_mutation_detected_in_game",
    ] {
        if mutation_sensitivity[key] != true {
            return Err(format!(
                "Flight of Fancy mutation escaped gameplay oracle: {key}"
            ));
        }
    }
    let breakdown: Vec<_> = catalog.iter().map(card_status).collect();
    Ok(json!({
        "schema_version": "cardbench.magic.card-pvp.check.v1",
        "task_id": TASK_ID,
        "expansion": "RAV",
        "card": {"collector_number": 49, "definition_id": CARD_ID, "name": CARD_NAME},
        "candidate": {"kind": candidate_kind(), "card_definition": CARD_ID},
        "candidate_kind": candidate_kind(),
        "viewer_scope": format!("{:?}", ViewerScope::Judge),
        "rules_authority": {
            "comprehensive_rules": "2026-08-07",
            "checked_sections": ["303.4 Auras", "601.2 Casting spells", "603.2-603.3 Triggered abilities", "608.2 Resolving spells and abilities", "613 Continuous effects", "510 Combat damage"],
            "source_contract": "CardBench-authored semantic summary: the blue RAV Aura enchants a creature, grants flying while attached, and has a mandatory enter-the-battlefield ability that draws two cards.",
            "authority_cutoff": "2026-09-24"
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
        .map_err(|error| format!("could not read Flight of Fancy source: {error}"))?;
    let file = syn::parse_file(&source)
        .map_err(|_| "Flight of Fancy source is not valid Rust".to_owned())?;
    if !file
        .attrs
        .iter()
        .all(|attribute| attribute.path().is_ident("doc"))
    {
        return Err("Flight of Fancy source has an unsupported module attribute".to_owned());
    }
    let expected_imports: BTreeSet<Vec<String>> = [
        vec!["std", "collections", "BTreeSet"],
        vec!["cardbench_magic_engine", "CardDefinition"],
        vec!["cardbench_magic_engine", "CardType"],
        vec!["cardbench_magic_engine", "Color"],
        vec!["cardbench_magic_engine", "ContinuousChange"],
        vec!["cardbench_magic_engine", "Effect"],
        vec!["cardbench_magic_engine", "Keyword"],
        vec!["cardbench_magic_engine", "ManaCost"],
        vec!["cardbench_magic_engine", "TargetRequirement"],
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
                    return Err("Flight of Fancy source has unsupported imports".to_owned());
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
                    _ => return Err("Flight of Fancy has an unsupported function".to_owned()),
                };
                let Stmt::Expr(Expr::Struct(literal), None) = &item.block.stmts[0] else {
                    return Err(
                        "Flight of Fancy functions must return direct typed literals".to_owned(),
                    );
                };
                if !type_is(&item.sig.output, output)
                    || !path_is_type(&literal.path, output)
                    || literal.rest.is_some()
                    || !literal.attrs.is_empty()
                {
                    return Err(
                        "Flight of Fancy functions must return direct typed literals".to_owned(),
                    );
                }
                if !functions.insert(item.sig.ident.to_string()) {
                    return Err("Flight of Fancy has duplicate functions".to_owned());
                }
            }
            _ => {
                return Err(
                    "Flight of Fancy source contains an unsupported top-level item".to_owned(),
                );
            }
        }
    }
    if imports != expected_imports
        || functions != BTreeSet::from(["definition".to_owned(), "triggered_ability".to_owned()])
        || file.items.len() != 4
    {
        return Err("Flight of Fancy module structure differs from source contract".to_owned());
    }
    Ok(())
}

fn validate_candidate_bindings() -> Result<(), String> {
    let definition = candidate_flight_of_fancy::definition();
    let registered = card_definitions()
        .into_iter()
        .find(|card| card.id == CARD_ID)
        .ok_or("Flight of Fancy definition is missing from RAV registry")?;
    if definition != registered
        || definition.name != CARD_NAME
        || definition.set_code != "RAV"
        || definition.mana_cost != ManaCost::with_colors(3, [Color::Blue])
        || definition.colors != BTreeSet::from([Color::Blue])
        || !definition.mana_colors.is_empty()
        || definition.card_types != BTreeSet::from([CardType::Enchantment])
        || definition.is_basic_land
        || definition.power.is_some()
        || definition.toughness.is_some()
        || !definition.keywords.is_empty()
        || definition.effects
            != [Effect::AttachSourceToTarget {
                target: TargetRequirement::Creature,
                changes: vec![cardbench_magic_engine::ContinuousChange::AddKeyword(
                    Keyword::Flying,
                )],
            }]
    {
        return Err("Flight of Fancy literal is outside the task source contract".to_owned());
    }
    let binding = candidate_flight_of_fancy::triggered_ability();
    let registered = rav_triggered_ability_bindings()
        .into_iter()
        .find(|item| item.card_definition == CARD_ID)
        .ok_or("Flight of Fancy ETB draw trigger is missing from RAV registry")?;
    if binding != registered
        || binding.ability.id != TRIGGER_ID
        || binding.ability.condition != TriggerCondition::EntersBattlefield
        || binding.ability.mana_cost != ManaCost::new(0)
        || binding.ability.optional
        || !binding.ability.targets.is_empty()
        || binding.ability.effects != [Effect::DrawController, Effect::DrawController]
    {
        return Err(
            "Flight of Fancy trigger literal is outside the task source contract".to_owned(),
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
        return Err("RAV catalog denominator differs from source contract".to_owned());
    }
    Ok(catalog)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Mutation {
    None,
    NoFlying,
    NoTrigger,
    DrawOne,
}

#[derive(Clone, Debug, PartialEq)]
struct CapturedMatch {
    events: Vec<GameEvent>,
    winner: Option<usize>,
    turns: u32,
    accepted: u32,
    rejected: u32,
    aura: Option<ObjectId>,
    target: Option<ObjectId>,
    payment: Vec<Color>,
    drawn: Vec<(ObjectId, String)>,
    target_controlled: bool,
    target_flying: bool,
    target_attacked: bool,
    target_combat_damage: i32,
}

fn find_trace_seed() -> Result<u64, String> {
    let mut last_error = "no trace attempt".to_owned();
    for seed in 0..MAX_TRACE_SEED {
        match run_match(seed, Mutation::None).and_then(|capture| validate_trace(&capture)) {
            Ok(()) => return Ok(seed),
            Err(error) => last_error = format!("seed {seed}: {error}"),
        }
    }
    Err(format!(
        "no bounded shuffled duel completed Flight of Fancy trace; {last_error}"
    ))
}

fn run_match(seed: u64, mutation: Mutation) -> Result<CapturedMatch, String> {
    let mut definitions = card_definitions();
    let definition = definitions
        .iter_mut()
        .find(|card| card.id == CARD_ID)
        .ok_or("Flight of Fancy definition missing from RAV registry")?;
    *definition = candidate_flight_of_fancy::definition();
    if mutation == Mutation::NoFlying {
        definition.effects = vec![Effect::AttachSourceToTarget {
            target: TargetRequirement::Creature,
            changes: vec![],
        }];
    }
    let mut triggers = rav_triggered_ability_bindings();
    let trigger_index = triggers
        .iter()
        .position(|binding| binding.card_definition == CARD_ID)
        .ok_or("Flight of Fancy trigger missing from RAV registry")?;
    triggers[trigger_index] = candidate_flight_of_fancy::triggered_ability();
    match mutation {
        Mutation::NoTrigger => {
            triggers.remove(trigger_index);
        }
        Mutation::DrawOne => {
            triggers[trigger_index].ability.effects = vec![Effect::DrawController];
        }
        Mutation::None | Mutation::NoFlying => {}
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
                count: 18,
            },
            DeckEntry {
                card: "RAV-PLAINS".into(),
                count: 18,
            },
            DeckEntry {
                card: "RAV-ISLAND".into(),
                count: 16,
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
        Box::new(FlightPolicy::new(PlayerId(0))),
        Box::new(FlightPolicy::new(PlayerId(1))),
    ];
    let mut accepted = 0_u32;
    let mut attempted = 0_u32;
    while !game.is_game_over() {
        if game.turn > config.max_turns || accepted >= config.max_policy_moves {
            return Err(format!(
                "Flight of Fancy match exceeded bounds at turn {}",
                game.turn
            ));
        }
        let player = game.next_policy_player();
        let view = game
            .view_for_player(player)
            .map_err(|error| error.to_string())?;
        let policy = policies
            .get_mut(player.0)
            .ok_or("Flight of Fancy policy seat missing")?;
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
        let summary = format!("{action:?}");
        game.submit_policy_move(player, policy.id(), action)
            .map_err(|error| {
                format!(
                    "policy move rejected at turn {} {:?}: {error}; action={summary}; mana={:?}",
                    view.turn, view.step, view.mana_pool
                )
            })?;
        accepted += 1;
        game.validate_invariants()
            .map_err(|error| error.to_string())?;
    }
    let events = game.event_log.clone();
    let aura = events.iter().find_map(|event| match event {
        GameEvent::SpellCast {
            player: PlayerId(0),
            card,
        } if game
            .card_definition(*card)
            .is_ok_and(|def| def.id == CARD_ID) =>
        {
            Some(*card)
        }
        _ => None,
    });
    let target = aura.and_then(|aura| {
        events.iter().find_map(|event| match event {
            GameEvent::AuraAttached {
                aura: attached_aura,
                target,
            } if *attached_aura == aura => Some(*target),
            _ => None,
        })
    });
    let payment = aura
        .and_then(|aura| {
            events.iter().find_map(|event| match event {
                GameEvent::SpellManaPaid {
                    player: PlayerId(0),
                    card,
                    colors,
                } if *card == aura => Some(colors.clone()),
                _ => None,
            })
        })
        .unwrap_or_default();
    let drawn = aura.and_then(|aura| {
        let start = events.iter().position(|event| matches!(event, GameEvent::TriggeredAbilityStacked { source, ability: TRIGGER_ID, .. } if *source == aura))?;
        let end = events.iter().position(|event| matches!(event, GameEvent::AbilityResolved { source, ability: TRIGGER_ID, .. } if *source == aura))?;
        Some(events[start + 1..end].iter().filter_map(|event| match event { GameEvent::CardMoved { card, to: Zone::Hand } => game.card_definition(*card).ok().map(|def| (*card, def.id.to_owned())), _ => None }).collect::<Vec<_>>())
    }).unwrap_or_default();
    let (target_controlled, target_flying) = target.map_or((false, false), |target| {
        (
            game.controller_of(target)
                .is_ok_and(|controller| controller == PlayerId(0)),
            game.characteristics(target)
                .is_ok_and(|chars| chars.keywords.contains(&Keyword::Flying)),
        )
    });
    let target_attacked = target.is_some_and(|target| events.iter().any(|event| matches!(event, GameEvent::AttackersDeclared { player: PlayerId(0), attackers } if attackers.contains(&target))));
    let target_combat_damage = target.map_or(0, |target| {
        events
            .iter()
            .filter_map(|event| match event {
                GameEvent::DamageDealtToPlayer {
                    source,
                    player: PlayerId(1),
                    amount,
                } if *source == target => Some(*amount),
                _ => None,
            })
            .sum()
    });
    Ok(CapturedMatch {
        events,
        winner: game.winner().map(|player| player.0),
        turns: game.turn,
        accepted,
        rejected: attempted.saturating_sub(accepted),
        aura,
        target,
        payment,
        drawn,
        target_controlled,
        target_flying,
        target_attacked,
        target_combat_damage,
    })
}

struct FlightPolicy {
    player: PlayerId,
}
impl FlightPolicy {
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
    fn activate_any(&self, view: &GameView) -> Option<PolicyAction> {
        [
            Color::Green,
            Color::White,
            Color::Blue,
            Color::Red,
            Color::Black,
        ]
        .into_iter()
        .find_map(|color| self.activate_color(view, color))
    }
    fn cast(&self, card: ObjectId, targets: Vec<Target>, generic: Vec<Color>) -> PolicyAction {
        PolicyAction::CastWithPayment {
            request: cardbench_magic_engine::CastRequest {
                card,
                targets,
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
    fn choose_land(&self, view: &GameView) -> Option<ObjectId> {
        let counts = [Color::Green, Color::White, Color::Blue].map(|color| {
            view.own_battlefield
                .iter()
                .filter(|land| {
                    land.card_types.contains(&CardType::Land) && land.mana_colors.contains(&color)
                })
                .count()
        });
        let preferred = if counts[0] == 0 {
            "RAV-FOREST"
        } else if counts[1] == 0 {
            "RAV-PLAINS"
        } else if counts[2] == 0 {
            "RAV-ISLAND"
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
            .map(|card| card.id)
    }
    fn generic_payment(&self, view: &GameView) -> Option<Vec<Color>> {
        if view.mana_pool.amount(Color::Blue) == 0 || view.mana_pool.total() < 4 {
            return None;
        }
        let mut remaining: Vec<_> = Color::MANA_ALL
            .into_iter()
            .map(|color| (color, view.mana_pool.amount(color)))
            .collect();
        remaining
            .iter_mut()
            .find(|(color, _)| *color == Color::Blue)?
            .1 -= 1;
        let mut generic = Vec::new();
        for color in [
            Color::Green,
            Color::White,
            Color::Red,
            Color::Black,
            Color::Blue,
        ] {
            let (_, amount) = remaining
                .iter_mut()
                .find(|(available, _)| *available == color)?;
            while *amount > 0 && generic.len() < 3 {
                generic.push(color);
                *amount -= 1;
            }
        }
        (generic.len() == 3).then_some(generic)
    }
}
impl CodePolicy for FlightPolicy {
    fn id(&self) -> &'static str {
        "rav.flight-of-fancy-aura-flying-draw-two.v1"
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
        if view.lands_played == 0
            && let Some(land) = self.choose_land(view)
        {
            return PolicyAction::PlayLand { card: land };
        }
        if self.player != PlayerId(0) {
            return PolicyAction::PassPriority;
        }
        let target = view
            .own_battlefield
            .iter()
            .find(|card| card.definition == Some(WATCHWOLF_ID))
            .map(|card| card.id);
        let aura_on_battlefield = view
            .own_battlefield
            .iter()
            .any(|card| card.definition == Some(CARD_ID));
        if !aura_on_battlefield
            && let (Some(target), Some(aura)) = (
                target,
                view.hand
                    .iter()
                    .find(|card| card.definition == Some(CARD_ID)),
            )
        {
            if view.mana_pool.amount(Color::Blue) == 0 {
                if let Some(action) = self.activate_color(view, Color::Blue) {
                    return action;
                }
            }
            if view.mana_pool.total() < 4 {
                if let Some(action) = self.activate_any(view) {
                    return action;
                }
            }
            if let Some(generic) = self.generic_payment(view) {
                return self.cast(aura.id, vec![Target::Permanent(target)], generic);
            }
        }
        if let Some(watchwolf) = view
            .hand
            .iter()
            .find(|card| card.definition == Some(WATCHWOLF_ID))
        {
            if view.mana_pool.amount(Color::Green) == 0 {
                if let Some(action) = self.activate_color(view, Color::Green) {
                    return action;
                }
            }
            if view.mana_pool.amount(Color::White) == 0 {
                if let Some(action) = self.activate_color(view, Color::White) {
                    return action;
                }
            }
            if view.mana_pool.amount(Color::Green) > 0 && view.mana_pool.amount(Color::White) > 0 {
                return self.cast(watchwolf.id, vec![], vec![]);
            }
        }
        PolicyAction::PassPriority
    }
}

fn validate_trace(capture: &CapturedMatch) -> Result<(), String> {
    if capture.winner != Some(0) || capture.rejected != 0 {
        return Err("Flight of Fancy duel did not finish as a clean P0 win".to_owned());
    }
    let aura = capture.aura.ok_or("Flight of Fancy was not cast")?;
    let target = capture
        .target
        .ok_or("Flight of Fancy did not attach to its target")?;
    if aura == target
        || capture.payment.len() != 4
        || capture
            .payment
            .iter()
            .filter(|color| **color == Color::Blue)
            .count()
            != 1
    {
        return Err("Flight of Fancy did not pay exact {3}{U}".to_owned());
    }
    if !capture.target_controlled || !capture.target_flying {
        return Err(
            "Flight of Fancy did not grant flying to its controller's attached creature".to_owned(),
        );
    }
    let stacked: Vec<_> = capture.events.iter().filter(|event| matches!(event, GameEvent::TriggeredAbilityStacked { controller: PlayerId(0), source, ability: TRIGGER_ID, .. } if *source == aura)).collect();
    let resolved: Vec<_> = capture.events.iter().filter(|event| matches!(event, GameEvent::AbilityResolved { source, ability: TRIGGER_ID, .. } if *source == aura)).collect();
    if stacked.len() != 1 || resolved.len() != 1 || capture.drawn.len() != 2 {
        return Err(format!(
            "Flight of Fancy ETB must stack/resolve once and draw two cards; stacked={}, resolved={}, drawn={}",
            stacked.len(),
            resolved.len(),
            capture.drawn.len()
        ));
    }
    if !capture.target_attacked || capture.target_combat_damage <= 0 {
        return Err("the enchanted creature did not attack and deal combat damage".to_owned());
    }
    if !capture.events.iter().any(|event| {
        matches!(
            event,
            GameEvent::GameEnded {
                winner: Some(PlayerId(0))
            }
        )
    }) {
        return Err("the trace lacks terminal P0 GameEnded receipt".to_owned());
    }
    let paid = event_index(&capture.events, |event| matches!(event, GameEvent::SpellManaPaid { player: PlayerId(0), card, colors } if *card == aura && colors.len() == 4 && colors.iter().filter(|color| **color == Color::Blue).count() == 1)).ok_or("exact Flight mana receipt missing")?;
    let cast = event_index(&capture.events, |event| matches!(event, GameEvent::SpellCast { player: PlayerId(0), card } if *card == aura)).ok_or("Flight cast receipt missing")?;
    let resolved_spell = event_index(
        &capture.events,
        |event| matches!(event, GameEvent::SpellResolved { card } if *card == aura),
    )
    .ok_or("Flight resolution receipt missing")?;
    let enters = event_index(&capture.events, |event| matches!(event, GameEvent::CardMoved { card, to: Zone::Battlefield } if *card == aura)).ok_or("Flight battlefield entry receipt missing")?;
    let attached = event_index(&capture.events, |event| matches!(event, GameEvent::AuraAttached { aura: attached_aura, target: attached_target } if *attached_aura == aura && *attached_target == target)).ok_or("exact AuraAttached receipt missing")?;
    let trigger = event_index(&capture.events, |event| matches!(event, GameEvent::TriggeredAbilityStacked { controller: PlayerId(0), source, ability: TRIGGER_ID, .. } if *source == aura)).ok_or("Flight ETB trigger receipt missing")?;
    let trigger_end = event_index(&capture.events, |event| matches!(event, GameEvent::AbilityResolved { source, ability: TRIGGER_ID, .. } if *source == aura)).ok_or("Flight ETB resolution receipt missing")?;
    let draw_events: Vec<_> = capture
        .events
        .iter()
        .enumerate()
        .filter_map(|(index, event)| {
            (index > trigger
                && index < trigger_end
                && matches!(event, GameEvent::CardMoved { to: Zone::Hand, .. }))
            .then_some(index)
        })
        .collect();
    let attack = event_index(&capture.events, |event| matches!(event, GameEvent::AttackersDeclared { player: PlayerId(0), attackers } if attackers.contains(&target))).ok_or("enchanted creature attack receipt missing")?;
    let damage = event_index_after(&capture.events, attack, |event| matches!(event, GameEvent::DamageDealtToPlayer { source, player: PlayerId(1), amount } if *source == target && *amount > 0)).ok_or("enchanted creature combat damage receipt missing")?;
    let end = event_index(&capture.events, |event| {
        matches!(
            event,
            GameEvent::GameEnded {
                winner: Some(PlayerId(0))
            }
        )
    })
    .ok_or("terminal P0 win receipt missing")?;
    if draw_events.len() != 2
        || !(paid < cast
            && cast < resolved_spell
            && resolved_spell < enters
            && enters < attached
            && attached < trigger
            && trigger < draw_events[0]
            && draw_events[0] < draw_events[1]
            && draw_events[1] < trigger_end
            && trigger_end < attack
            && attack < damage
            && damage < end)
    {
        return Err(
            "Flight payment, attachment, ETB draw, combat, and terminal event order differs"
                .to_owned(),
        );
    }
    Ok(())
}

fn mutation_detected(seed: u64, mutation: Mutation) -> bool {
    run_match(seed, mutation)
        .and_then(|capture| validate_trace(&capture))
        .is_err()
}
fn same_match(left: &CapturedMatch, right: &CapturedMatch) -> bool {
    left == right
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

fn trace_json(capture: &CapturedMatch) -> Value {
    let aura = capture.aura;
    let target = capture.target;
    let draw_objects: BTreeSet<_> = capture.drawn.iter().map(|(id, _)| *id).collect();
    let projected = project_events(&capture.events);
    let events: Vec<Value> = capture.events.iter().enumerate().filter_map(|(index, event)| {
        let include = match event {
            GameEvent::SpellManaPaid { card, .. } | GameEvent::SpellCast { card, .. } | GameEvent::SpellResolved { card } => aura == Some(*card),
            GameEvent::CardMoved { card, to: Zone::Battlefield } => aura == Some(*card),
            GameEvent::CardMoved { card, to: Zone::Hand } => draw_objects.contains(card),
            GameEvent::AuraAttached { aura: event_aura, target: event_target } => aura == Some(*event_aura) && target == Some(*event_target),
            GameEvent::TriggeredAbilityStacked { source, ability, .. } | GameEvent::AbilityResolved { source, ability, .. } => aura == Some(*source) && *ability == TRIGGER_ID,
            GameEvent::AttackersDeclared { player: PlayerId(0), attackers } => target.is_some_and(|target| attackers.contains(&target)),
            GameEvent::DamageDealtToPlayer { source, player: PlayerId(1), .. } => target == Some(*source),
            GameEvent::GameEnded { winner: Some(PlayerId(0)) } => true,
            _ => false,
        };
        if !include { return None; }
        let event: &ProjectedEvent = projected.get(index)?;
        Some(json!({"sequence": event.sequence, "turn": event.turn, "kind": event.kind, "facts": event.facts, "canonical": event.canonical}))
    }).collect();
    json!({
        "trace_id": "rav-flight-of-fancy-attaches-flying-and-draws-two",
        "terminal": capture.winner.map_or_else(|| "Draw".to_owned(), |seat| format!("Winner(PlayerId({seat}))")),
        "winner": capture.winner,
        "turns": capture.turns,
        "accepted_policy_moves": capture.accepted,
        "rejected_policy_moves": capture.rejected,
        "mechanics": {
            "shuffled_60_card_decks": true,
            "mana_cost": "{3}{U}",
            "paid_mana_colors": capture.payment.iter().map(|color| format!("{color:?}")).collect::<Vec<_>>(),
            "aura_object": aura.map(|id| id.0),
            "target_creature_object": target.map(|id| id.0),
            "aura_attached_to_target": aura.is_some() && target.is_some(),
            "target_controlled_by_controller": capture.target_controlled,
            "target_has_flying": capture.target_flying,
            "trigger_stack_count": capture.events.iter().filter(|event| matches!(event, GameEvent::TriggeredAbilityStacked { source, ability: TRIGGER_ID, .. } if Some(*source) == aura)).count(),
            "trigger_resolve_count": capture.events.iter().filter(|event| matches!(event, GameEvent::AbilityResolved { source, ability: TRIGGER_ID, .. } if Some(*source) == aura)).count(),
            "drawn_card_objects": capture.drawn.iter().map(|(id, _)| id.0).collect::<Vec<_>>(),
            "drawn_card_definitions": capture.drawn.iter().map(|(_, definition)| definition.clone()).collect::<Vec<_>>(),
            "draw_count": capture.drawn.len(),
            "deterministic_replay": true,
            "terminal_combat_win": capture.winner == Some(0) && capture.target_attacked && capture.target_combat_damage > 0
        },
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
    let passed = card.collector_number == 49;
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
