use super::*;

#[derive(Clone, Copy)]
pub(super) struct SetSpec {
    pub(super) category: &'static str,
    pub(super) set_name: &'static str,
    pub(super) texture_true_name: &'static str,
    pub(super) texture_resource: &'static str,
    pub(super) texture_path_id: i64,
    pub(super) item_number: i64,
    pub(super) aliases: &'static [&'static str],
    pub(super) models: &'static [ModelSpec],
}
