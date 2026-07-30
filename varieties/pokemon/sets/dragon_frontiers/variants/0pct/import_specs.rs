//! Dragon Frontiers import-spec benchmark stub.

use serde_json::Value;

use crate::{PowerBodySpec, TrainerSpec};

pub const DF_POWERS: &[PowerBodySpec] = &[];
pub const DF_TRAINERS: &[TrainerSpec] = &[];

pub fn power_effect_ast(_number: &str, _power_name: &str, _kind: &str) -> Option<Value> {
    None
}

pub fn trainer_effect_ast(_number: &str, _card_name: &str, _trainer_kind: &str) -> Option<Value> {
    None
}

pub fn attack_effect_ast(_number: &str, _attack_name: &str) -> Option<Value> {
    None
}
