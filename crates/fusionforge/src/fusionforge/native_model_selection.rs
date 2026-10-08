//! Exact render selection within a compound model, before native conversion.
//! Keep the original hierarchy, bind matrices and animation data unchanged.
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeSet;

#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct Selection {
    pub mesh: String,
    pub logical_name: String,
    pub idle_source: String,
    #[serde(default)]
    pub call_from_idle: bool,
}

pub(super) fn select(source: &mut Value, selection: &Selection) -> Result<(), String> {
    // Work on a copy so an invalid/ambiguous request cannot partially alter input.
    let mut selected = source.clone();
    let meshes = selected["meshes"].as_array_mut().ok_or("missing meshes")?;
    let matches = meshes
        .iter()
        .filter(|m| m["name"].as_str() == Some(&selection.mesh))
        .count();
    if matches != 1 {
        return Err(format!(
            "exact mesh {:?} resolved {matches} matches",
            selection.mesh
        ));
    }
    meshes.retain(|m| m["name"].as_str() == Some(&selection.mesh));
    let mesh_id = meshes[0]["id"]
        .as_str()
        .ok_or("mesh has no identity")?
        .to_owned();
    let bindings = selected["rendererMaterialBindings"]
        .as_array_mut()
        .ok_or("missing renderer bindings")?;
    bindings.retain(|binding| binding["mesh"]["id"].as_str() == Some(&mesh_id));
    if bindings.len() != 1 {
        return Err("selected mesh must have one exact renderer".into());
    }
    let material_ids: BTreeSet<String> = bindings
        .iter()
        .flat_map(|binding| binding["materialSlots"].as_array().into_iter().flatten())
        .filter_map(|slot| slot["materialId"].as_str().map(str::to_owned))
        .collect();
    let materials = selected["materials"]
        .as_object_mut()
        .ok_or("missing materials")?;
    materials.retain(|id, _| material_ids.contains(id));
    if materials.len() != material_ids.len() {
        return Err("selected renderer has a missing material".into());
    }
    let texture_ids: BTreeSet<String> = materials
        .values()
        .flat_map(|material| {
            material["savedProperties"]["textureEnvs"]
                .as_array()
                .into_iter()
                .flatten()
        })
        .filter_map(|slot| slot["textureId"].as_str().map(str::to_owned))
        .collect();
    let textures = selected["textures"]
        .as_object_mut()
        .ok_or("missing textures")?;
    textures.retain(|id, _| texture_ids.contains(id));
    if textures.len() != texture_ids.len() {
        return Err("selected material has a missing texture".into());
    }

    let old_name = selected["logicalName"]
        .as_str()
        .ok_or("missing logical name")?
        .to_owned();
    if selection.logical_name.is_empty() || selection.logical_name.contains(['/', '\\']) {
        return Err("selection requires a safe semantic logical name".into());
    }
    // Only absolute hierarchy paths change. Relative joint/clip targets stay intact.
    for key in ["roots", "nodes"] {
        for node in selected["modelHierarchy"][key]
            .as_array_mut()
            .ok_or("missing hierarchy")?
        {
            if node["path"].as_str() == Some(&old_name) {
                node["name"] = Value::String(selection.logical_name.clone());
            }
            for field in ["path", "parent"] {
                rename_path(&mut node[field], &old_name, &selection.logical_name);
            }
        }
    }
    for mesh in selected["meshes"].as_array_mut().unwrap() {
        for binding in mesh["sourceBindings"]
            .as_array_mut()
            .ok_or("missing mesh bindings")?
        {
            if binding["rootTransformName"].as_str() != Some(&old_name) {
                return Err("selected renderer is not bound to the proven root".into());
            }
            binding["rootTransformName"] = Value::String(selection.logical_name.clone());
            rename_path(
                &mut binding["transformPath"],
                &old_name,
                &selection.logical_name,
            );
        }
    }
    selected["logicalName"] = Value::String(selection.logical_name.clone());
    if let Some(proof) = selected.get_mut("exactMeshSelectionProof") {
        proof["selectedMeshes"] = Value::from(1);
    }
    let animations = selected["animations"]
        .as_array_mut()
        .ok_or("missing animations")?;
    if animations
        .iter()
        .filter(|a| a["name"].as_str() == Some(&selection.idle_source))
        .count()
        != 1
    {
        return Err("idle source must resolve one existing clip".into());
    }
    if selection.idle_source != "stand1" && animations.iter().any(|a| a["name"] == "stand1") {
        return Err("stand1 already exists; refusing an animation collision".into());
    }
    for animation in animations.iter_mut() {
        if animation["name"].as_str() == Some(&selection.idle_source) {
            animation["name"] = Value::String("stand1".into());
        }
    }
    if selection.call_from_idle {
        if animations.iter().any(|a| a["name"] == "call") {
            return Err("call already exists; refusing an animation collision".into());
        }
        let mut call = animations
            .iter()
            .find(|a| a["name"] == "stand1")
            .unwrap()
            .clone();
        if call["events"]
            .as_array()
            .is_some_and(|events| !events.is_empty())
        {
            return Err("idle events cannot be reused for summon".into());
        }
        call["name"] = Value::String("call".into());
        call["loop"] = Value::Bool(false);
        animations.push(call);
    }
    *source = selected;
    Ok(())
}

fn rename_path(value: &mut Value, old: &str, new: &str) {
    if let Some(path) = value.as_str() {
        if path == old {
            *value = Value::String(new.into());
        } else if let Some(suffix) = path.strip_prefix(&format!("{old}/")) {
            *value = Value::String(format!("{new}/{suffix}"));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn fixture() -> Value {
        json!({"logicalName":"group", "modelHierarchy":{"roots":[{"name":"group","path":"group"}],
            "nodes":[{"name":"group","path":"group","parent":null},{"name":"rig","path":"group/rig","parent":"group","translation":[1,2,3]}]},
            "skeleton":{"joints":[{"path":"rig/bone","translation":[3,2,1]}]},
            "meshes":[{"id":"a","name":"chosen","positions":[1,2,3],"skin":{"jointPaths":["rig/bone"],"weights":[0.2,0.8]},"sourceBindings":[{"rootTransformName":"group","transformPath":"group/rig"}]},{"id":"b","name":"other"}],
            "rendererMaterialBindings":[{"mesh":{"id":"a"},"materialSlots":[{"materialId":"ma"}]},{"mesh":{"id":"b"},"materialSlots":[{"materialId":"mb"}]}],
            "materials":{"ma":{"savedProperties":{"textureEnvs":[{"textureId":"ta"}]}},"mb":{"savedProperties":{"textureEnvs":[{"textureId":"tb"}]}}},
            "textures":{"ta":{"pixels":[1,2,3]},"tb":{"pixels":[4,5,6]}},
            "animations":[{"name":"nif-default","duration":2.5,"loop":true,"animationData":{"rotations":[{"path":"rig/bone","keys":[1,3,2]}]}}]})
    }

    fn selection() -> Selection {
        Selection {
            mesh: "chosen".into(),
            logical_name: "nano_chosen".into(),
            idle_source: "nif-default".into(),
            call_from_idle: true,
        }
    }

    #[test]
    fn isolates_renderer_without_changing_skin_pose_or_curves() {
        let before = fixture();
        let mut after = before.clone();
        select(&mut after, &selection()).unwrap();
        assert_eq!(after["meshes"].as_array().unwrap().len(), 1);
        for key in ["positions", "skin"] {
            assert_eq!(after["meshes"][0][key], before["meshes"][0][key]);
        }
        assert_eq!(after["skeleton"], before["skeleton"]);
        assert_eq!(
            after["animations"][0]["animationData"],
            before["animations"][0]["animationData"]
        );
        assert_eq!(
            after["modelHierarchy"]["nodes"][1]["translation"],
            json!([1, 2, 3])
        );
        assert_eq!(after["modelHierarchy"]["nodes"][1]["parent"], "nano_chosen");
        assert_eq!(after["animations"][0]["name"], "stand1");
        assert_eq!(after["animations"][1]["name"], "call");
        assert_eq!(after["animations"][1]["loop"], false);
        assert_eq!(
            after["animations"][1]["animationData"],
            before["animations"][0]["animationData"]
        );
        assert_eq!(after["materials"].as_object().unwrap().len(), 1);
        assert_eq!(after["textures"].as_object().unwrap().len(), 1);
    }

    #[test]
    fn ambiguity_and_missing_idle_leave_source_intact() {
        for ambiguous in [false, true] {
            let mut source = fixture();
            if ambiguous {
                let duplicate = source["meshes"][0].clone();
                source["meshes"].as_array_mut().unwrap().push(duplicate);
            } else {
                source["animations"][0]["name"] = json!("other");
            }
            let before = source.clone();
            assert!(select(&mut source, &selection()).is_err());
            assert_eq!(source, before);
        }
    }
}
