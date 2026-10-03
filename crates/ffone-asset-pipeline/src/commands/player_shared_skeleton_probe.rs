//! Temporary/read-only probe for the already extracted CharacterSelection object dump.
//!
//! This binary intentionally never opens a Unity bundle. It narrows the large offline
//! JSON extraction to exact PathIDs while the reusable shared-skeleton publisher is
//! being implemented.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs::File,
    io::BufReader,
    path::PathBuf,
};

use serde::Deserialize;
use serde_json::value::RawValue;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DumpObject {
    name: String,
    path_id: i64,
    #[serde(rename = "type")]
    object_type: String,
    value: Box<RawValue>,
}

pub(super) fn run_command(command_args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let mut args = command_args.iter().map(std::ffi::OsString::from);
    let input = PathBuf::from(args.next().ok_or("missing extracted objects JSON")?);
    let output = PathBuf::from(args.next().ok_or("missing output JSON")?);
    let mut animation_component = None;
    let ids = args
        .filter_map(|value| {
            let value = value.to_string_lossy();
            if let Some(raw) = value.strip_prefix("animation:") {
                animation_component = Some(raw.parse::<i64>().ok()?);
                None
            } else {
                Some(
                    value
                        .parse::<i64>()
                        .map_err(|_| "PathID must be an integer"),
                )
            }
        })
        .collect::<Result<BTreeSet<_>, _>>()?;
    if ids.is_empty() && animation_component.is_none() {
        return Err("at least one PathID is required".into());
    }

    let file = File::open(&input)?;
    let objects: Vec<DumpObject> = serde_json::from_reader(BufReader::new(file))?;
    let by_path_id = objects
        .iter()
        .enumerate()
        .map(|(index, object)| (object.path_id, index))
        .collect::<BTreeMap<_, _>>();
    let mut selected = objects
        .iter()
        .filter(|object| ids.contains(&object.path_id))
        .map(|object| {
            let value: serde_json::Value = serde_json::from_str(object.value.get())?;
            Ok(serde_json::json!({
                "name": object.name,
                "pathId": object.path_id,
                "type": object.object_type,
                "value": value,
            }))
        })
        .collect::<Result<Vec<_>, serde_json::Error>>()?;
    if selected.len() != ids.len() {
        return Err(format!(
            "requested {} PathIDs but found {} exact objects",
            ids.len(),
            selected.len()
        )
        .into());
    }
    if let Some(component_path_id) = animation_component {
        let component = by_path_id
            .get(&component_path_id)
            .and_then(|index| objects.get(*index))
            .ok_or("animation component PathID is absent")?;
        let value: serde_json::Value = serde_json::from_str(component.value.get())?;
        let clips = value
            .get("m_Animations")
            .and_then(serde_json::Value::as_array)
            .ok_or("component has no m_Animations")?;
        let mut clip_index = Vec::with_capacity(clips.len());
        for (order, pointer) in clips.iter().enumerate() {
            let path_id = pointer
                .get("pathId")
                .and_then(serde_json::Value::as_i64)
                .ok_or("invalid clip pointer")?;
            let clip = by_path_id
                .get(&path_id)
                .and_then(|index| objects.get(*index))
                .ok_or("referenced AnimationClip is absent")?;
            let body: serde_json::Value = serde_json::from_str(clip.value.get())?;
            clip_index.push(serde_json::json!({
                "order": order,
                "pathId": path_id,
                "name": clip.name,
                "type": clip.object_type,
                "compressedRotationCurveCount": body.get("m_CompressedRotationCurves").and_then(serde_json::Value::as_array).map_or(0, Vec::len),
                "positionCurveCount": body.get("m_PositionCurves").and_then(serde_json::Value::as_array).map_or(0, Vec::len),
                "rotationCurveCount": body.get("m_RotationCurves").and_then(serde_json::Value::as_array).map_or(0, Vec::len),
                "scaleCurveCount": body.get("m_ScaleCurves").and_then(serde_json::Value::as_array).map_or(0, Vec::len),
                "floatCurveCount": body.get("m_FloatCurves").and_then(serde_json::Value::as_array).map_or(0, Vec::len),
                "eventCount": body.get("m_Events").and_then(serde_json::Value::as_array).map_or(0, Vec::len),
            }));
        }
        selected.push(serde_json::json!({
            "animationComponentPathId": component_path_id,
            "clips": clip_index,
        }));
    }
    let mut bytes = serde_json::to_vec_pretty(&selected)?;
    bytes.push(b'\n');
    std::fs::write(output, bytes)?;
    Ok(())
}
