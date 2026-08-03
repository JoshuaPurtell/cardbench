//! Reference Rust policies for deterministic Magic engine development matches.

#![forbid(unsafe_code)]

mod boros_char_control;
mod boros_convoke_burn;
mod boros_radiance_assault;
mod boros_tempo;
mod boros_token_rally;
mod deck_match;
mod development_match;
mod dimir_transmute_attrition;
mod dimir_transmute_convoke;
mod dimir_transmute_helix;
mod golgari_attrition;
mod golgari_dredge_grind;
mod golgari_wurm_press;
mod radiance_convoke_assault;
mod selesnya_convoke;
mod selesnya_radiance_tokens;
mod selesnya_siege;

pub use boros_char_control::BorosCharControlPolicy;
pub use boros_convoke_burn::BorosConvokeBurnPolicy;
pub use boros_radiance_assault::BorosRadianceAssaultPolicy;
pub use boros_tempo::BorosTempoPolicy;
pub use boros_token_rally::BorosTokenRallyPolicy;
pub use deck_match::{
    DeckMatchConfig, DeckMatchResult, DeckMatchSweepResult, DeckMatchTermination, EngineFinding,
    EngineFindingKind, EngineTournamentFailure, EngineTournamentResult, RAV_DECK_MATCH_ID,
    RAV_REFERENCE_DECK_MATRIX_ID, run_rav_deck_matchup, run_rav_engine_tournament,
    run_rav_full_deck_match, run_rav_full_deck_sweep, run_rav_reference_deck_matrix,
};
pub use development_match::{PolicyMatchResult, run_rav_reference_match};
pub use dimir_transmute_attrition::DimirTransmuteAttritionPolicy;
pub use dimir_transmute_convoke::DimirTransmuteConvokePolicy;
pub use dimir_transmute_helix::DimirTransmuteHelixPolicy;
pub use golgari_attrition::GolgariAttritionPolicy;
pub use golgari_dredge_grind::GolgariDredgeGrindPolicy;
pub use golgari_wurm_press::GolgariWurmPressPolicy;
pub use radiance_convoke_assault::RadianceConvokeAssaultPolicy;
pub use selesnya_convoke::SelesnyaConvokePolicy;
pub use selesnya_radiance_tokens::SelesnyaRadianceTokensPolicy;
pub use selesnya_siege::SelesnyaSiegePolicy;

use cardbench_magic_engine::{GameView, PolicyAction};

/// Submission ABI for `cardbench/magic/code_policy` development runs.
pub trait CodePolicy {
    fn id(&self) -> &'static str;
    fn propose_move(&mut self, view: &GameView) -> PolicyAction;

    /// Chooses a draw replacement when the engine exposes that mandatory
    /// decision. Policies that do not use replacement effects take the normal
    /// draw by default.
    fn propose_draw_replacement(&mut self, _view: &GameView) -> PolicyAction {
        PolicyAction::Draw { dredge: None }
    }

    /// Completes a mandatory, controller-private library choice that was
    /// opened in the middle of spell resolution. The conservative default
    /// selects no cards, so a policy cannot accidentally pay life for hidden
    /// cards it has not been implemented to evaluate.
    fn propose_private_library_choice(&mut self, view: &GameView) -> PolicyAction {
        let choice = view
            .private_library_choice
            .as_ref()
            .expect("private-library choice proposal requires a visible choice");
        PolicyAction::ChoosePrivateLibraryCards {
            decision: choice.decision,
            spell: choice.spell,
            selected: Vec::new(),
        }
    }

    /// Completes a mandatory private choice opened by a targeted activated
    /// ability that inspects an opponent's library. With no card-evaluation
    /// policy yet, the deterministic development default exiles the current
    /// top candidate (or submits `None` when the target library is empty).
    fn propose_private_opponent_library_choice(&mut self, view: &GameView) -> PolicyAction {
        let choice = view
            .private_opponent_library_choice
            .as_ref()
            .expect("private opponent-library choice proposal requires a visible choice");
        PolicyAction::ChoosePrivateOpponentLibraryCardToExile {
            source: choice.source,
            ability: choice.ability,
            selected: choice.cards.first().map(|card| card.id),
        }
    }

    /// Completes a controller-private typed library search suspended during a
    /// spell or ability resolution. The conservative default selects the
    /// first legal candidate, or explicitly finds nothing when no candidate
    /// exists. Policies that value particular cards can override this view.
    fn propose_library_search_choice(&mut self, view: &GameView) -> PolicyAction {
        let choice = view
            .library_search_choice
            .as_ref()
            .expect("library-search choice proposal requires a visible choice");
        PolicyAction::ChooseLibrarySearchCard {
            source: choice.source,
            selected: choice.cards.first().map(|card| card.id),
        }
    }
}
