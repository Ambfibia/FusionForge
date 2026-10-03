use super::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum EquipmentGpuBatchMode {
    Smoke,
    Full,
}

impl EquipmentGpuBatchMode {
    pub fn parse(value: &str) -> std::result::Result<Self, String> {
        match value {
            "smoke" => Ok(Self::Smoke),
            "full" => Ok(Self::Full),
            _ => Err(format!(
                "invalid equipment GPU batch mode {value:?}; expected smoke or full"
            )),
        }
    }
}

pub(super) struct Selection {
    pub(super) index: usize,
    pub(super) reasons: Vec<String>,
}

pub(super) fn status(
    mode: EquipmentGpuBatchMode,
    shard: Option<EquipmentGpuShard>,
    selected_passed: bool,
    full_passed: bool,
    execution_blockers: usize,
) -> &'static str {
    match (
        mode,
        shard.is_some(),
        selected_passed,
        full_passed,
        execution_blockers,
    ) {
        (EquipmentGpuBatchMode::Smoke, false, true, _, 0) => {
            "smoke-standalone-gpu-passed-player-attachment-parity-pending"
        }
        (EquipmentGpuBatchMode::Full, true, true, _, 0) => {
            "full-shard-standalone-gpu-passed-player-attachment-parity-pending"
        }
        (EquipmentGpuBatchMode::Full, false, true, true, 0) => {
            "full-standalone-gpu-passed-player-attachment-parity-pending"
        }
        _ => "complete-with-standalone-gpu-blockers-player-attachment-parity-pending",
    }
}
