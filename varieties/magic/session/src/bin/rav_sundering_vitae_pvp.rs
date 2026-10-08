//! Full-game diagnostic authority for RAV Sundering Vitae.

use std::collections::BTreeSet;
use std::process::ExitCode;

use cardbench_magic_engine::{
    CardDefinition, CardType, CastRequest, Color, ConvokeContribution, ConvokePayment, DeckEntry,
    DeckList, Effect, Game, GameEvent, GameView, Keyword, ManaCost, ManaPaymentSelection, ObjectId,
    PlayerId, PolicyAction, Step, Target, Zone,
};
use cardbench_magic_policies::{CodePolicy, DeckMatchConfig};
use cardbench_magic_protocol::ViewerScope;
use cardbench_magic_rav::{
    CardSemanticStatus, CatalogCard, RAV_MAIN_SET_EXPECTED_PRINTING_COUNT,
    RAV_MAIN_SET_EXPECTED_UNIQUE_NAME_COUNT, card_definitions, rav_basic_land_type_bindings,
    rav_main_set_catalog, rav_mana_ability_bindings,
};
use cardbench_magic_session::project_events;
use serde_json::{Value, json};

const TASK_ID: &str = "cardbench/magic/card";
const CARD_ID: &str = "RAV-SUNDERING-VITAE";
const CARD_NAME: &str = "Sundering Vitae";
const TARGET_ID: &str = "RAV-BOROS-SIGNET";
const CREATURE_ID: &str = "RAV-WATCHWOLF";
const SOURCE_PATH: &str =
    "varieties/magic/sets/ravnica_city_of_guilds/src/cards/sundering_vitae.rs";
const MAX_SEED: u64 = 32;
const MAX_TURNS: u32 = 50;

#[path = "../../../sets/ravnica_city_of_guilds/src/cards/sundering_vitae.rs"]
mod candidate_sundering_vitae;

fn main() -> ExitCode {
    match run_authority() {
        Ok(report) => {
            if let Some(root) = output_root()
                && let Err(error) = write_outputs(&root, &report)
            {
                eprintln!("could not write Sundering Vitae result: {error}");
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
            eprintln!("Sundering Vitae PvP authority failed: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run_authority() -> Result<Value, String> {
    validate_source()?;
    validate_candidate()?;
    let printings = rav_main_set_catalog();
    let mut names = BTreeSet::new();
    let catalog: Vec<CatalogCard> = printings
        .iter()
        .copied()
        .filter(|card| names.insert(card.name))
        .collect();
    if catalog.len() != RAV_MAIN_SET_EXPECTED_UNIQUE_NAME_COUNT
        || printings.len() != RAV_MAIN_SET_EXPECTED_PRINTING_COUNT
        || catalog.len() != 291
        || printings.len() != 306
    {
        return Err("RAV catalog denominator differs from the source contract".to_owned());
    }
    let (seed, captured) = find_trace()?;
    let trace = validate_trace(&captured, seed)?;
    let replay = run_match(seed, Mutation::None)?;
    if captured != replay {
        return Err("Sundering Vitae full-game replay differed".to_owned());
    }
    let mutations = json!({
        "convoke_removed_mutation_detected": mutation_detected(seed, Mutation::NoConvoke),
        "cost_mutation_detected": mutation_detected(seed, Mutation::WrongCost),
        "destruction_removed_mutation_detected": mutation_detected(seed, Mutation::NoDestroy),
        "sealed_mutation_authority": false
    });
    for name in [
        "convoke_removed_mutation_detected",
        "cost_mutation_detected",
        "destruction_removed_mutation_detected",
    ] {
        if mutations[name] != true {
            return Err(format!("Sundering Vitae semantic mutant escaped: {name}"));
        }
    }
    let breakdown: Vec<_> = catalog.iter().map(card_status).collect();
    Ok(json!({
        "schema_version":"cardbench.magic.card-pvp.check.v1",
        "task_id":TASK_ID,"expansion":"RAV",
        "card":{"collector_number":185,"printings":1,"definition_id":CARD_ID,"name":CARD_NAME},
        "candidate":{"kind":candidate_kind(),"card_definition":CARD_ID},"candidate_kind":candidate_kind(),
        "viewer_scope":format!("{:?}",ViewerScope::Judge),
        "rules_authority":{"comprehensive_rules":"2026-08-07","checked_sections":[
            "601.2 Casting spells and paying costs","608.2 Resolving spells","701.7 Destroying permanents","702.50 Convoke","508.1 Declaring attackers","510 Combat damage"
        ],"source_contract":"RAV #185 Sundering Vitae is a green {2}{G} Instant with Convoke that destroys target artifact or enchantment.","authority_cutoff":"2026-09-24","no_provider_trial":true},
        "passed":true,"pilot_card_passed":true,"deterministic_replay":true,"cards_passed":1,
        "cards_credited":0,"cards_total":catalog.len(),"printings_total":printings.len(),
        "coverage_complete":false,"full_reward":false,"harbor_reward":0.0,"pilot_reward":0.0,
        "qualification_status":"diagnostic_only; independent review and candidate trial pending",
        "mutation_sensitivity":mutations,"traces":[trace],"per_card_breakdown":breakdown
    }))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Mutation {
    None,
    NoConvoke,
    WrongCost,
    NoDestroy,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Captured {
    events: Vec<GameEvent>,
    winner: Option<usize>,
    turns: u32,
    accepted: u32,
    rejected: u32,
    spell: Option<ObjectId>,
    target: Option<ObjectId>,
    convoker: Option<ObjectId>,
    target_zone: Option<Zone>,
    spell_zone: Option<Zone>,
    convoker_tapped: bool,
}

fn mutated_definition(mutation: Mutation) -> CardDefinition {
    let mut definition = candidate_sundering_vitae::definition();
    match mutation {
        Mutation::None => {}
        Mutation::NoConvoke => definition
            .keywords
            .retain(|keyword| *keyword != Keyword::Convoke),
        Mutation::WrongCost => definition.mana_cost = ManaCost::with_colors(3, [Color::Green]),
        Mutation::NoDestroy => definition.effects.clear(),
    }
    definition
}

fn validate_candidate() -> Result<(), String> {
    let definition = candidate_sundering_vitae::definition();
    let registered = card_definitions()
        .into_iter()
        .find(|item| item.id == CARD_ID)
        .ok_or("Sundering Vitae missing from RAV registry")?;
    if definition != registered
        || definition.id != CARD_ID
        || definition.name != CARD_NAME
        || definition.set_code != "RAV"
        || definition.mana_cost != ManaCost::with_colors(2, [Color::Green])
        || definition.colors != BTreeSet::from([Color::Green])
        || !definition.mana_colors.is_empty()
        || definition.card_types != BTreeSet::from([CardType::Instant])
        || definition.is_basic_land
        || definition.power.is_some()
        || definition.toughness.is_some()
        || definition.keywords != [Keyword::Convoke]
        || definition.effects != [Effect::DestroyTargetArtifactOrEnchantment]
    {
        return Err("Sundering Vitae definition differs from exact RAV #185 identity/cost/Convoke/destruction contract".to_owned());
    }
    Ok(())
}

fn validate_source() -> Result<(), String> {
    let source = std::fs::read_to_string(SOURCE_PATH)
        .map_err(|e| format!("could not read candidate source: {e}"))?;
    let file = syn::parse_file(&source)
        .map_err(|_| "Sundering Vitae source is not valid Rust".to_owned())?;
    if file.items.len() != 3 || !file.attrs.iter().all(|attr| attr.path().is_ident("doc")) {
        return Err(
            "Sundering Vitae module must contain direct imports and one literal definition"
                .to_owned(),
        );
    }
    let mut found = false;
    for item in file.items {
        if let syn::Item::Fn(function) = item {
            if function.sig.ident != "definition"
                || found
                || function.block.stmts.len() != 1
                || !matches!(function.vis, syn::Visibility::Restricted(ref v) if v.path.is_ident("crate"))
                || !matches!(function.block.stmts.first(), Some(syn::Stmt::Expr(syn::Expr::Struct(s), None)) if s.path.segments.last().is_some_and(|seg| seg.ident == "CardDefinition"))
            {
                return Err(
                    "Sundering Vitae must expose only its crate-private literal definition"
                        .to_owned(),
                );
            }
            found = true;
        }
    }
    if !found {
        return Err("Sundering Vitae candidate definition is missing".to_owned());
    }
    Ok(())
}

fn new_game(definition: CardDefinition) -> Result<Game, String> {
    let mut definitions = card_definitions();
    let entry = definitions
        .iter_mut()
        .find(|card| card.id == CARD_ID)
        .ok_or("registry entry missing")?;
    *entry = definition;
    Game::new_with_mana_abilities_and_basic_land_types(
        definitions,
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
    )
    .map_err(|e| e.to_string())
}

fn find_trace() -> Result<(u64, Captured), String> {
    if let Ok(value) = std::env::var("CARDBENCH_SUNDERING_VITAE_SEED") {
        let seed = value
            .parse()
            .map_err(|_| "CARDBENCH_SUNDERING_VITAE_SEED must be an unsigned integer".to_owned())?;
        let capture = run_match(seed, Mutation::None)?;
        validate_trace(&capture, seed)?;
        return Ok((seed, capture));
    }
    let mut last = String::new();
    for seed in 0..=MAX_SEED {
        if let Ok(capture) = run_match(seed, Mutation::None) {
            match validate_trace(&capture, seed) {
                Ok(_) => return Ok((seed, capture)),
                Err(error) => last = error,
            }
        }
    }
    Err(format!(
        "no terminal Sundering Vitae trace in seeds 0..={MAX_SEED}: {last}"
    ))
}

fn run_match(seed: u64, mutation: Mutation) -> Result<Captured, String> {
    let mut game = new_game(mutated_definition(mutation))?;
    let pilot = DeckList {
        mainboard: vec![
            DeckEntry {
                card: CARD_ID.into(),
                count: 4,
            },
            DeckEntry {
                card: CREATURE_ID.into(),
                count: 4,
            },
            DeckEntry {
                card: "RAV-FOREST".into(),
                count: 26,
            },
            DeckEntry {
                card: "RAV-PLAINS".into(),
                count: 26,
            },
        ],
        sideboard: vec![],
    };
    let opponent = DeckList {
        mainboard: vec![
            DeckEntry {
                card: TARGET_ID.into(),
                count: 4,
            },
            DeckEntry {
                card: "RAV-MOUNTAIN".into(),
                count: 56,
            },
        ],
        sideboard: vec![],
    };
    if pilot
        .mainboard
        .iter()
        .map(|entry| usize::from(entry.count))
        .sum::<usize>()
        != 60
        || opponent
            .mainboard
            .iter()
            .map(|entry| usize::from(entry.count))
            .sum::<usize>()
            != 60
    {
        return Err("Sundering Vitae decks must each contain exactly 60 cards".to_owned());
    }
    let config = DeckMatchConfig {
        shuffle_seed: seed,
        ..DeckMatchConfig::default()
    };
    game.set_shuffle_seed(seed).map_err(|e| e.to_string())?;
    game.load_deck_into_library(PlayerId(0), &pilot)
        .map_err(|e| e.to_string())?;
    game.load_deck_into_library(PlayerId(1), &opponent)
        .map_err(|e| e.to_string())?;
    for player in [PlayerId(0), PlayerId(1)] {
        game.draw_opening_hand(player, config.opening_hand_size)
            .map_err(|e| e.to_string())?;
    }
    game.begin_game().map_err(|e| e.to_string())?;
    let mut policies: [Box<dyn CodePolicy>; 2] = [
        Box::new(Pilot {
            player: PlayerId(0),
            cast: false,
        }),
        Box::new(Opponent {
            player: PlayerId(1),
            signet_cast: false,
        }),
    ];
    let mut accepted = 0_u32;
    let mut attempted = 0_u32;
    let mut convoker_tapped_at_payment = None;
    while !game.is_game_over() {
        if game.turn > MAX_TURNS || accepted >= config.max_policy_moves {
            return Err("Sundering Vitae match exceeded action/turn bound".into());
        }
        let player = game.next_policy_player();
        let view = game.view_for_player(player).map_err(|e| e.to_string())?;
        let policy = policies
            .get_mut(player.0)
            .ok_or("Sundering Vitae policy seat missing")?;
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
        attempted += 1;
        game.submit_policy_move(player, policy.id(), action)
            .map_err(|e| {
                format!(
                    "policy move rejected at turn {} {:?}: {e}",
                    view.turn, view.step
                )
            })?;
        accepted += 1;
        game.validate_invariants().map_err(|e| e.to_string())?;
        if convoker_tapped_at_payment.is_none()
            && let Some(creature) = game.event_log.iter().find_map(|event| match event {
                GameEvent::ConvokeUsed {
                    player: PlayerId(0),
                    creature,
                    contribution: Some(Color::Green),
                } => Some(*creature),
                _ => None,
            })
        {
            convoker_tapped_at_payment = Some((
                creature,
                game.object(creature).map_err(|e| e.to_string())?.tapped,
            ));
        }
    }
    let events = game.event_log.clone();
    let spell = events.iter().find_map(|event| match event {
        GameEvent::SpellCast {
            player: PlayerId(0),
            card,
        } if game.card_definition(*card).is_ok_and(|d| d.id == CARD_ID) => Some(*card),
        _ => None,
    });
    let target = spell.and_then(|spell| {
        events.iter().find_map(|event| match event {
            GameEvent::CardDestroyed { source, card } if *source == spell => Some(*card),
            _ => None,
        })
    });
    let convoker = convoker_tapped_at_payment.map(|(creature, _)| creature);
    let target_zone_at_resolution = target
        .filter(|target| {
            events.iter().any(|event| {
                matches!(
                    event,
                    GameEvent::CardMoved { card, to: Zone::Graveyard } if card == target
                )
            })
        })
        .map(|_| Zone::Graveyard);
    Ok(Captured {
        events,
        winner: game.winner().map(|p| p.0),
        turns: game.turn,
        accepted,
        rejected: attempted.saturating_sub(accepted),
        spell,
        target,
        convoker,
        target_zone: target_zone_at_resolution,
        spell_zone: spell.and_then(|id| game.zone_of(id)),
        convoker_tapped: convoker_tapped_at_payment.is_some_and(|(_, tapped)| tapped),
    })
}

struct Pilot {
    player: PlayerId,
    cast: bool,
}
struct Opponent {
    player: PlayerId,
    signet_cast: bool,
}

fn common_combat(view: &GameView, player: PlayerId) -> Option<PolicyAction> {
    if view.player != player {
        return None;
    }
    if view.step == Step::DeclareAttackers
        && view.active_player == player
        && !view.attackers_declared
    {
        return Some(PolicyAction::DeclareAttackers {
            attackers: view
                .own_battlefield
                .iter()
                .filter(|c| c.can_attack)
                .map(|c| c.id)
                .collect(),
        });
    }
    if view.step == Step::DeclareBlockers && view.active_player != player && !view.blockers_declared
    {
        return Some(PolicyAction::DeclareBlockers {
            assignments: vec![],
        });
    }
    None
}

impl CodePolicy for Pilot {
    fn id(&self) -> &'static str {
        "rav.sundering-vitae-convoke-artifact-destruction.v1"
    }
    fn propose_move(&mut self, view: &GameView) -> PolicyAction {
        if let Some(action) = common_combat(view, self.player) {
            return action;
        }
        if view.player != self.player
            || view.decision_player != self.player
            || view.priority != self.player
            || view.stack_depth != 0
        {
            return PolicyAction::PassPriority;
        }
        let main = view.active_player == self.player
            && matches!(view.step, Step::PrecombatMain | Step::PostcombatMain);
        if !main {
            return PolicyAction::PassPriority;
        }
        if view.lands_played == 0 {
            let forests = view
                .own_battlefield
                .iter()
                .filter(|c| c.mana_colors.contains(&Color::Green))
                .count();
            let plains = view
                .own_battlefield
                .iter()
                .filter(|c| c.mana_colors.contains(&Color::White))
                .count();
            let preferred = if forests == 0 {
                "RAV-FOREST"
            } else if plains < 2 {
                "RAV-PLAINS"
            } else if forests <= plains {
                "RAV-FOREST"
            } else {
                "RAV-PLAINS"
            };
            if let Some(land) = view.hand.iter().find(|c| c.definition == Some(preferred)) {
                return PolicyAction::PlayLand { card: land.id };
            }
            if let Some(land) = view
                .hand
                .iter()
                .find(|c| c.definition == Some("RAV-FOREST") || c.definition == Some("RAV-PLAINS"))
            {
                return PolicyAction::PlayLand { card: land.id };
            }
        }
        if !self.cast
            && let Some(wolf) = view.hand.iter().find(|c| c.definition == Some(CREATURE_ID))
        {
            if view.mana_pool.amount(Color::Green) >= 1 && view.mana_pool.amount(Color::White) >= 1
            {
                return PolicyAction::Cast(CastRequest {
                    card: wolf.id,
                    targets: vec![],
                    convoke: vec![],
                    payment_mana_abilities: vec![],
                });
            }
            if let Some(land) = view.own_battlefield.iter().find(|c| {
                !c.tapped
                    && c.definition == Some("RAV-FOREST")
                    && view.mana_pool.amount(Color::Green) == 0
            }) {
                return PolicyAction::ActivateManaAbility {
                    land: land.id,
                    color: Color::Green,
                };
            }
            if let Some(land) = view.own_battlefield.iter().find(|c| {
                !c.tapped
                    && c.definition == Some("RAV-PLAINS")
                    && view.mana_pool.amount(Color::White) == 0
            }) {
                return PolicyAction::ActivateManaAbility {
                    land: land.id,
                    color: Color::White,
                };
            }
        }
        if !self.cast {
            if let (Some(card), Some(convoker)) = (
                view.hand.iter().find(|c| c.definition == Some(CARD_ID)),
                view.own_battlefield
                    .iter()
                    .find(|c| c.definition == Some(CREATURE_ID) && !c.tapped),
            ) {
                if view
                    .opponent_battlefield
                    .iter()
                    .any(|c| c.definition == Some(TARGET_ID))
                {
                    if view.mana_pool.amount(Color::White) >= 2 {
                        self.cast = true;
                        let target = view
                            .opponent_battlefield
                            .iter()
                            .find(|c| c.definition == Some(TARGET_ID))
                            .map(|c| c.id)
                            .expect("target found");
                        return PolicyAction::CastWithPayment {
                            request: CastRequest {
                                card: card.id,
                                targets: vec![Target::Permanent(target)],
                                convoke: vec![ConvokePayment {
                                    creature: convoker.id,
                                    contribution: ConvokeContribution::Color(Color::Green),
                                }],
                                payment_mana_abilities: vec![],
                            },
                            chosen_x: None,
                            mana_selection: ManaPaymentSelection {
                                generic: vec![Color::White; 2],
                                hybrid: vec![],
                            },
                        };
                    }
                    if let Some(land) = view
                        .own_battlefield
                        .iter()
                        .find(|c| !c.tapped && c.definition == Some("RAV-PLAINS"))
                    {
                        return PolicyAction::ActivateManaAbility {
                            land: land.id,
                            color: Color::White,
                        };
                    }
                }
            }
        }
        if let Some(land) = view.own_battlefield.iter().find(|c| {
            !c.tapped
                && ((view.mana_pool.amount(Color::Green) == 0
                    && c.definition == Some("RAV-FOREST"))
                    || (view.mana_pool.amount(Color::White) == 0
                        && c.definition == Some("RAV-PLAINS")))
        }) {
            let color = if land.definition == Some("RAV-FOREST") {
                Color::Green
            } else {
                Color::White
            };
            return PolicyAction::ActivateManaAbility {
                land: land.id,
                color,
            };
        }
        PolicyAction::PassPriority
    }
}

impl CodePolicy for Opponent {
    fn id(&self) -> &'static str {
        "rav.signet-opponent-pass.v1"
    }
    fn propose_move(&mut self, view: &GameView) -> PolicyAction {
        if let Some(action) = common_combat(view, self.player) {
            return action;
        }
        if view.player != self.player
            || view.decision_player != self.player
            || view.priority != self.player
            || view.stack_depth != 0
            || view.active_player != self.player
            || !matches!(view.step, Step::PrecombatMain | Step::PostcombatMain)
        {
            return PolicyAction::PassPriority;
        }
        if view.lands_played == 0
            && let Some(land) = view
                .hand
                .iter()
                .find(|c| c.definition == Some("RAV-MOUNTAIN"))
        {
            return PolicyAction::PlayLand { card: land.id };
        }
        if !self.signet_cast
            && let Some(signet) = view.hand.iter().find(|c| c.definition == Some(TARGET_ID))
        {
            if view.mana_pool.amount(Color::Red) >= 2 {
                self.signet_cast = true;
                return PolicyAction::Cast(CastRequest {
                    card: signet.id,
                    targets: vec![],
                    convoke: vec![],
                    payment_mana_abilities: vec![],
                });
            }
            if let Some(land) = view
                .own_battlefield
                .iter()
                .find(|c| !c.tapped && c.definition == Some("RAV-MOUNTAIN"))
            {
                return PolicyAction::ActivateManaAbility {
                    land: land.id,
                    color: Color::Red,
                };
            }
        }
        PolicyAction::PassPriority
    }
}

fn validate_trace(captured: &Captured, seed: u64) -> Result<Value, String> {
    if captured.winner != Some(0) {
        return Err("Sundering Vitae match did not reach a seat-zero terminal win".into());
    }
    if captured.rejected != 0 {
        return Err("Sundering Vitae policy had rejected moves".into());
    }
    let spell = captured.spell.ok_or("Sundering Vitae was not cast")?;
    let target = captured
        .target
        .ok_or("Sundering Vitae did not destroy a target")?;
    let convoker = captured
        .convoker
        .ok_or("Sundering Vitae did not use colored Convoke")?;
    if captured.target_zone != Some(Zone::Graveyard)
        || captured.spell_zone != Some(Zone::Graveyard)
        || !captured.convoker_tapped
    {
        return Err(format!(
            "Sundering Vitae target, spell, or Convoke creature state is wrong (target={:?} zone={:?}, spell={:?}, convoker_tapped={}; target events={:?})",
            target,
            captured.target_zone,
            captured.spell_zone,
            captured.convoker_tapped,
            captured
                .events
                .iter()
                .filter(|event| match event {
                    GameEvent::CardDestroyed { card, .. } | GameEvent::CardMoved { card, .. } =>
                        *card == target,
                    _ => false,
                })
                .collect::<Vec<_>>()
        ));
    }
    let convoke = captured.events.iter().position(|event| matches!(event, GameEvent::ConvokeUsed { player: PlayerId(0), creature, contribution: Some(Color::Green) } if *creature == convoker)).ok_or("green Convoke receipt missing")?;
    let payment = captured.events.iter().position(|event| matches!(event, GameEvent::SpellManaPaid { player: PlayerId(0), card, colors } if *card == spell && colors == &[Color::White, Color::White])).ok_or("two white mana did not pay the generic portion")?;
    let cast = captured.events.iter().position(|event| matches!(event, GameEvent::SpellCast { player: PlayerId(0), card } if *card == spell)).ok_or("SpellCast receipt missing")?;
    let destroy = captured.events.iter().position(|event| matches!(event, GameEvent::CardDestroyed { source, card } if *source == spell && *card == target)).ok_or("exact artifact target was not destroyed by Sundering Vitae")?;
    let target_move = captured.events.iter().position(|event| matches!(event, GameEvent::CardMoved { card, to: Zone::Graveyard } if *card == target)).ok_or("destroyed artifact did not move to graveyard")?;
    let resolve = captured
        .events
        .iter()
        .position(|event| matches!(event, GameEvent::SpellResolved { card } if *card == spell))
        .ok_or("Sundering Vitae resolution receipt missing")?;
    let spell_move = captured.events.iter().position(|event| matches!(event, GameEvent::CardMoved { card, to: Zone::Graveyard } if *card == spell)).ok_or("resolved spell did not move to graveyard")?;
    let ended = captured
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
        .ok_or("terminal seat-zero win receipt missing")?;
    if !(convoke < payment
        && payment < cast
        && cast < destroy
        && destroy < target_move
        && target_move < resolve
        && resolve < spell_move
        && spell_move < ended)
    {
        return Err("Sundering Vitae payment/destruction/resolution order is invalid".into());
    }
    if !captured.events.iter().any(|event| matches!(event, GameEvent::AttackersDeclared { player: PlayerId(0), attackers } if !attackers.is_empty())) { return Err("Sundering Vitae game did not continue through combat".into()); }
    let keep = [
        convoke,
        payment,
        cast,
        destroy,
        target_move,
        resolve,
        spell_move,
        ended,
    ]
    .into_iter()
    .collect::<BTreeSet<_>>();
    let projected: Vec<_> = project_events(&captured.events)
        .into_iter()
        .enumerate()
        .filter_map(|(index, event)| keep.contains(&index).then_some(event))
        .collect();
    Ok(
        json!({"trace_id":"rav-sundering-vitae-convoke-destroys-artifact-full-game","shuffle_seed":seed,"decks":["rav_sundering_vitae_watchwolf_60","rav_boros_signet_mountain_60"],"policies":["rav.sundering-vitae-convoke-artifact-destruction.v1","rav.signet-opponent-pass.v1"],"terminal":"Winner(PlayerId(0))","winner_seat":0,"turns":captured.turns,"accepted_moves":captured.accepted,"rejected_moves":captured.rejected,"passed":true,"sundering_vitae":{"object":spell.0,"collector_number":185,"mana_cost":"{2}{G}","convoke_creature":convoker.0,"convoke_contribution":"green","direct_mana_paid":["white","white"],"target_object":target.0,"target_definition":TARGET_ID,"target_owner_seat":1,"target_zone_after_resolution":"Graveyard","spell_zone_after_resolution":"Graveyard","convoke_creature_tapped":captured.convoker_tapped,"terminal_combat_win":true},"ordered_event_indices":[convoke,payment,cast,destroy,target_move,resolve,spell_move,ended],"events":projected}),
    )
}

fn mutation_detected(seed: u64, mutation: Mutation) -> bool {
    run_match(seed, mutation)
        .and_then(|capture| validate_trace(&capture, seed).map(|_| ()))
        .is_err()
}
fn card_status(card: &CatalogCard) -> Value {
    let id = match card.semantic_status {
        CardSemanticStatus::ExecutableCompatibilitySlice { definition_id } => Some(definition_id),
        CardSemanticStatus::CatalogOnly { .. } => None,
    };
    let passed = id == Some(CARD_ID);
    json!({"collector_number":card.collector_number,"name":card.name,"definition_id":id,"engine_status":match card.semantic_status { CardSemanticStatus::ExecutableCompatibilitySlice { .. } => "executable_compatibility_slice", CardSemanticStatus::CatalogOnly { .. } => "catalog_only" },"trace_status":if passed {"passed"} else {"not_qualified"},"credited":false,"qualification":if passed {"pilot_trace_passed; independent reviewer approval pending"} else {"not_evaluated"}})
}
fn candidate_kind() -> &'static str {
    if std::env::args().any(|arg| arg == "--candidate") {
        "candidate"
    } else {
        "reference"
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
