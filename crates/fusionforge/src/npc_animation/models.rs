use super::*;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SampledMeshPositions {
    pub id: String,
    pub positions: Vec<[f32; 3]>,
    pub skinned: bool,
}

#[derive(Debug, Clone)]
pub(super) struct PreviewMesh {
    pub(super) id: String,
    pub(super) positions: Vec<Vec3>,
    pub(super) skin: Option<MeshSkin>,
    pub(super) skin_warning: Option<String>,
}

#[derive(Debug, Clone)]
pub(super) struct MeshSkin {
    pub(super) palette: Vec<usize>,
    pub(super) bone_indices: Vec<[usize; 4]>,
    pub(super) weights: Vec<[f64; 4]>,
    pub(super) inverse_bind_matrices: Vec<Matrix4>,
    /// Converts animation-root coordinates back into the renderer's local
    /// mesh space. Unity bind poses are `boneWorldToLocal * rendererLocalToWorld`,
    /// so `boneGlobal * bindPose` still contains the renderer transform.
    pub(super) skeleton_to_mesh: Matrix4,
}

pub(super) fn parse_mesh_skin(
    value: &JsonValue,
    vertex_count: usize,
    joints: &[Joint],
    lookup: &JointLookup,
    rest_globals: &[Matrix4],
) -> Result<MeshSkin, String> {
    let joint_paths = value
        .get("jointPaths")
        .or_else(|| value.get("bones"))
        .and_then(JsonValue::as_array)
        .map(|values| {
            values
                .iter()
                .filter_map(JsonValue::as_str)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let palette = if joint_paths.is_empty() {
        (0..joints.len()).collect::<Vec<_>>()
    } else {
        joint_paths
            .iter()
            .map(|path| {
                lookup
                    .resolve(path)
                    .ok_or_else(|| format!("skin joint '{path}' is absent from the skeleton"))
            })
            .collect::<Result<Vec<_>, _>>()?
    };
    if palette.is_empty() {
        return Err("skin has no resolvable joint palette".to_string());
    }

    let bone_indices = value
        .get("boneIndices")
        .or_else(|| value.get("joints"))
        .and_then(|stream| parse_usize4_stream(stream, vertex_count))
        .ok_or_else(|| {
            "skin bone-index stream does not contain four indices per vertex".to_string()
        })?;
    let weights = value
        .get("weights")
        .and_then(|stream| parse_f64x4_stream(stream, vertex_count))
        .ok_or_else(|| "skin weight stream does not contain four weights per vertex".to_string())?;
    if bone_indices
        .iter()
        .flatten()
        .any(|index| *index >= palette.len())
    {
        return Err("skin bone index exceeds its joint palette".to_string());
    }

    let explicit_bind = value
        .get("inverseBindMatrices")
        .or_else(|| value.get("bindPoses"))
        .and_then(|matrices| parse_matrix_stream(matrices, palette.len()));
    let inverse_bind_matrices = explicit_bind.unwrap_or_else(|| {
        palette
            .iter()
            .map(|joint_index| {
                joints[*joint_index]
                    .inverse_bind
                    .or_else(|| invert_matrix(rest_globals[*joint_index]))
                    .unwrap_or_else(identity_matrix)
            })
            .collect()
    });
    let skeleton_to_mesh = palette
        .iter()
        .zip(&inverse_bind_matrices)
        .find_map(|(joint_index, inverse_bind)| {
            invert_matrix(mat_mul(rest_globals[*joint_index], *inverse_bind))
        })
        .unwrap_or_else(identity_matrix);

    Ok(MeshSkin {
        palette,
        bone_indices,
        weights,
        inverse_bind_matrices,
        skeleton_to_mesh,
    })
}
