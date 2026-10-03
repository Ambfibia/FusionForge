use super::*;

mod animation;
mod materials;
mod operations;
mod localization;
mod codec;
mod containers;
mod collision;
mod models;
mod state;
mod commands;

use animation::constant_test_clip;
use operations::{
    plain_test_curve, simon_empty_binding_regression_fixture, exact_constant_sibling_fixture
};
use localization::duplicate_translation_regression_fixture;
use codec::{decode_constant_sibling_fixture, decode_compressed_skin_values};
