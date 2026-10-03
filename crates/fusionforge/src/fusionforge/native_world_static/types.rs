use super::*;

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct JsonTransform {
    pub(super) translation: [f64; 3],
    pub(super) rotation: [f64; 4],
    pub(super) scale: [f64; 3],
}

#[derive(Debug, Clone)]
pub(super) struct TransformRecord {
    pub(super) game_object: ObjectKey,
    pub(super) parent: Option<ObjectKey>,
    pub(super) local: Matrix4,
    pub(super) authored: JsonTransform,
}

#[derive(Debug, Clone)]
pub(super) struct ComponentRecord {
    pub(super) key: ObjectKey,
    pub(super) object_type: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct SourceIdentity {
    pub(super) asset: String,
    pub(super) path_id: i64,
    pub(super) object_type: String,
}

#[derive(Debug, Clone)]
pub(super) struct SceneExtraction {
    pub(super) scene_asset_names: BTreeSet<String>,
    pub(super) game_objects: BTreeMap<String, GameObjectRecord>,
    pub(super) transforms: HashMap<ObjectKey, TransformRecord>,
    pub(super) transform_to_game_object: HashMap<ObjectKey, ObjectKey>,
    pub(super) payloads: Vec<PendingPayload>,
    pub(super) hierarchy_nodes: Vec<JsonValue>,
    pub(super) behaviour_components: Vec<BehaviourComponent>,
    pub(super) selected_meshes: BTreeSet<(usize, i64)>,
    pub(super) selected_material_objects: BTreeSet<(usize, i64)>,
    pub(super) counts: ExportCounts,
}

/// One non-geometry scene component selected for behaviour export.
///
/// Geometry is published as GLB payloads; these components carry the scripted,
/// animated and physical behaviour of the same nodes and are published as
/// exact serialized state joined to the hierarchy by `node`.
#[derive(Debug, Clone)]
pub(super) struct BehaviourComponent {
    pub(super) node: String,
    pub(super) key: ObjectKey,
    pub(super) object_type: String,
    pub(super) world_matrix: Matrix4,
}

pub(super) struct ScratchDirectory {
    pub(super) path: PathBuf,
}

impl ScratchDirectory {
    pub(super) fn fresh(parent: &Path, label: &str) -> Result<Self, String> {
        let path = parent.join(format!(
            ".{label}.{}.{:x}",
            std::process::id(),
            unique_nonce()?
        ));
        if fs::symlink_metadata(&path).is_ok() {
            return Err(format!(
                "scratch path unexpectedly exists: {}",
                path.display()
            ));
        }
        fs::create_dir(&path)
            .map_err(|err| format!("could not create scratch {}: {err}", path.display()))?;
        Ok(Self { path })
    }
}

impl Drop for ScratchDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct BehaviourCounts {
    pub(super) animations: usize,
    pub(super) distinct_animation_clips: usize,
    pub(super) distinct_effect_prefab_closures: usize,
    pub(super) box_colliders: usize,
    pub(super) capsule_colliders: usize,
    pub(super) mono_behaviours: usize,
    pub(super) rigidbodies: usize,
    pub(super) sphere_colliders: usize,
    pub(super) distinct_scripts: usize,
    pub(super) unresolved_scripts: usize,
}

/// Publication layout of one already-installed native terrain tile.
///
/// The scope, runtime identity and native scene location are read from the
/// manifested `ffone.runtime-world.v1` registry instead of being inferred from
/// the bundle name, so `tutorial` and `worldMap` tiles publish into their own
/// existing trees and a tile that is not part of the installed world is
/// rejected before any extraction work is committed.
#[derive(Debug, Clone)]
pub(super) struct TileLayout {
    pub(super) scope: String,
    pub(super) tile_id: String,
    pub(super) source_tile_relative: String,
    pub(super) model_relative_root: String,
    pub(super) static_relative_root: String,
}
