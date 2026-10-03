use super::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RootTrsAction {
    ReplaceWithSpawnWorldValue,
    #[serde(rename = "replace_with_npc_row_m_f_scale")]
    ReplaceWithNpcRowMFScale,
    ReplaceWithIdentity,
    PreserveAuthored,
    ControlledByRuntimeAttachment,
}
