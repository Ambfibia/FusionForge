use super::*;

pub(super) const WORLD_EFFECT_PREFAB_CLOSURE_SCHEMA: &str = "ffone.world-effect-prefab-closure.v1";

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ExportEffectPrefabClosure {
    pub(super) schema: String,
    pub(super) id: String,
    pub(super) root: JsonValue,
    pub(super) objects: Vec<JsonValue>,
}
