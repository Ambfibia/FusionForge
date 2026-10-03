use super::*;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct SourceEvent {
    pub(super) time: f64,
    pub(super) function_name: String,
    pub(super) string_parameter: String,
    pub(super) float_parameter: f64,
    #[serde(default)]
    pub(super) float_parameter_provenance: Option<AnimationEventFloatParameterProvenance>,
    pub(super) int_parameter: i32,
    pub(super) object_parameter: Option<Value>,
    pub(super) object_parameter_provenance: AnimationEventObjectParameterProvenance,
    pub(super) message_options: i32,
}

pub(super) fn is_exact_larry_unused_end_event(
    animation: &SourceAnimation,
    event: &SourceEvent,
    source_asset_index: u32,
    file_id: i64,
    object_path_id: i64,
) -> bool {
    if animation.asset.as_deref() != Some("CustomAssetBundle-574ca8c2fed89492582a3fbd49965c12")
        || event.function_name != "end"
        || !event.string_parameter.is_empty()
        || event.int_parameter != 0
        || source_asset_index != 5
        || file_id != 1
    {
        return false;
    }
    matches!(
        (
            animation.path_id,
            animation.name.as_str(),
            event.time,
            object_path_id,
            event.message_options,
        ),
        (Some(152), "stand2", 4.730000019073486, 6, 13_156)
            | (Some(153), "stand3", 2.5, 4, 0)
            | (Some(154), "walk", 3.0, 2_816, 1_852_788_223)
    )
}
