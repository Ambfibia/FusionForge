use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::{
    PipelineError, Result,
    policy::{reject_legacy_path, reject_legacy_text},
};

pub const MESH_SCHEMA: &str = "ffone.mesh.v1";

const GLB_MAGIC: &[u8; 4] = b"glTF";
const GLB_VERSION: u32 = 2;
const JSON_CHUNK_TYPE: u32 = 0x4e4f_534a;
const BIN_CHUNK_TYPE: u32 = 0x004e_4942;
const ARRAY_BUFFER: u32 = 34_962;
const ELEMENT_ARRAY_BUFFER: u32 = 34_963;
const FLOAT: u32 = 5_126;
const UNSIGNED_SHORT: u32 = 5_123;
const UNSIGNED_INT: u32 = 5_125;
const TRIANGLES: u32 = 4;

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct NativeMesh {
    pub schema: String,
    pub name: String,
    pub positions: Vec<[f64; 3]>,
    pub normals: Vec<[f64; 3]>,
    pub uvs: Vec<[f64; 2]>,
    pub submeshes: Vec<NativeSubmesh>,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct NativeSubmesh {
    pub indices: Vec<u32>,
}

/// Converts one strict `ffone.mesh.v1` document into a self-contained GLB 2.0 file.
///
/// Native positions, normals, and UVs are retained. Unity's clockwise triangle winding is
/// converted to glTF's counter-clockwise winding by swapping the second and third index of every
/// triangle. Every source submesh is represented by a distinct glTF primitive. There is
/// deliberately no source-engine coordinate conversion in this native-to-native stage.
pub fn mesh_json_to_glb(source_path: &str, bytes: &[u8]) -> Result<Vec<u8>> {
    reject_legacy_path(source_path)?;
    reject_legacy_text(source_path, bytes)?;
    let mesh: NativeMesh = serde_json::from_slice(bytes).map_err(|source| PipelineError::Json {
        path: source_path.to_owned(),
        source,
    })?;
    encode_mesh(source_path, &mesh)
}

fn encode_mesh(path: &str, mesh: &NativeMesh) -> Result<Vec<u8>> {
    validate_mesh(path, mesh)?;

    let positions = vec3_f32(path, "position", &mesh.positions)?;
    let normals = vec3_f32(path, "normal", &mesh.normals)?;
    let uvs = vec2_f32(path, "UV", &mesh.uvs)?;
    let (position_min, position_max) = position_bounds(&positions);

    let mut binary = Vec::new();
    let mut buffer_views = Vec::with_capacity(3 + mesh.submeshes.len());
    let mut accessors = Vec::with_capacity(3 + mesh.submeshes.len());
    let mut attributes = BTreeMap::new();

    let position_accessor = append_f32_vec3(
        path,
        &mut binary,
        &mut buffer_views,
        &mut accessors,
        &positions,
        Some(position_min),
        Some(position_max),
    )?;
    attributes.insert("POSITION", position_accessor);

    if !normals.is_empty() {
        let accessor = append_f32_vec3(
            path,
            &mut binary,
            &mut buffer_views,
            &mut accessors,
            &normals,
            None,
            None,
        )?;
        attributes.insert("NORMAL", accessor);
    }
    if !uvs.is_empty() {
        let accessor = append_f32_vec2(path, &mut binary, &mut buffer_views, &mut accessors, &uvs)?;
        attributes.insert("TEXCOORD_0", accessor);
    }

    let mut primitives = Vec::with_capacity(mesh.submeshes.len());
    for (submesh_index, submesh) in mesh.submeshes.iter().enumerate() {
        let indices = append_indices(
            path,
            &mut binary,
            &mut buffer_views,
            &mut accessors,
            &submesh.indices,
        )?;
        primitives.push(GltfPrimitive {
            attributes: attributes.clone(),
            indices,
            mode: TRIANGLES,
            extras: GltfPrimitiveExtras {
                ffone_submesh: submesh_index,
            },
        });
    }
    align_to_four(&mut binary, 0);

    let binary_len = checked_u32(path, binary.len(), "binary buffer exceeds u32")?;
    let root = GltfRoot {
        asset: GltfAsset {
            generator: "ffone-asset-pipeline".to_owned(),
            version: "2.0".to_owned(),
        },
        scene: 0,
        scenes: vec![GltfScene { nodes: vec![0] }],
        nodes: vec![GltfNode {
            mesh: 0,
            name: mesh.name.clone(),
        }],
        meshes: vec![GltfMesh {
            name: mesh.name.clone(),
            primitives,
        }],
        buffers: vec![GltfBuffer {
            byte_length: binary_len,
        }],
        buffer_views,
        accessors,
    };

    let mut json = serde_json::to_vec(&root).map_err(|source| PipelineError::Json {
        path: path.to_owned(),
        source,
    })?;
    align_to_four(&mut json, b' ');
    let json_len = checked_u32(path, json.len(), "JSON chunk exceeds u32")?;
    let bin_len = checked_u32(path, binary.len(), "BIN chunk exceeds u32")?;
    let total_len = 12_usize
        .checked_add(8)
        .and_then(|value| value.checked_add(json.len()))
        .and_then(|value| value.checked_add(8))
        .and_then(|value| value.checked_add(binary.len()))
        .ok_or_else(|| PipelineError::GlbOverflow {
            path: path.to_owned(),
            what: "total file length overflows usize",
        })?;
    let total_len = checked_u32(path, total_len, "total file length exceeds u32")?;

    let mut glb = Vec::with_capacity(total_len as usize);
    glb.extend_from_slice(GLB_MAGIC);
    push_u32(&mut glb, GLB_VERSION);
    push_u32(&mut glb, total_len);
    push_u32(&mut glb, json_len);
    push_u32(&mut glb, JSON_CHUNK_TYPE);
    glb.extend_from_slice(&json);
    push_u32(&mut glb, bin_len);
    push_u32(&mut glb, BIN_CHUNK_TYPE);
    glb.extend_from_slice(&binary);
    debug_assert_eq!(glb.len(), total_len as usize);
    Ok(glb)
}

fn validate_mesh(path: &str, mesh: &NativeMesh) -> Result<()> {
    if mesh.schema != MESH_SCHEMA {
        return invalid_mesh(
            path,
            format!("schema is {:?}, expected {MESH_SCHEMA:?}", mesh.schema),
        );
    }
    if mesh.name.is_empty() || mesh.name.len() > 1_024 || mesh.name.chars().any(char::is_control) {
        return invalid_mesh(
            path,
            "name must be non-empty, bounded UTF-8 text".to_owned(),
        );
    }
    if mesh.positions.is_empty() {
        return invalid_mesh(path, "positions must not be empty".to_owned());
    }
    if mesh.positions.len() > u32::MAX as usize {
        return Err(PipelineError::GlbOverflow {
            path: path.to_owned(),
            what: "vertex count exceeds u32",
        });
    }
    if !mesh.normals.is_empty() && mesh.normals.len() != mesh.positions.len() {
        return invalid_mesh(
            path,
            format!(
                "normal count {} does not match position count {}",
                mesh.normals.len(),
                mesh.positions.len()
            ),
        );
    }
    if !mesh.uvs.is_empty() && mesh.uvs.len() != mesh.positions.len() {
        return invalid_mesh(
            path,
            format!(
                "UV count {} does not match position count {}",
                mesh.uvs.len(),
                mesh.positions.len()
            ),
        );
    }
    if mesh.submeshes.is_empty() {
        return invalid_mesh(path, "at least one submesh is required".to_owned());
    }
    for (submesh_index, submesh) in mesh.submeshes.iter().enumerate() {
        if submesh.indices.is_empty() {
            return invalid_mesh(path, format!("submesh {submesh_index} has no indices"));
        }
        if submesh.indices.len() % 3 != 0 {
            return invalid_mesh(
                path,
                format!(
                    "submesh {submesh_index} index count {} is not divisible by three",
                    submesh.indices.len()
                ),
            );
        }
        if submesh.indices.len() > u32::MAX as usize {
            return Err(PipelineError::GlbOverflow {
                path: path.to_owned(),
                what: "index count exceeds u32",
            });
        }
        if let Some(index) = submesh
            .indices
            .iter()
            .copied()
            .find(|index| *index as usize >= mesh.positions.len())
        {
            return invalid_mesh(
                path,
                format!(
                    "submesh {submesh_index} index {index} is outside vertex count {}",
                    mesh.positions.len()
                ),
            );
        }
    }
    Ok(())
}

fn vec3_f32(path: &str, label: &str, values: &[[f64; 3]]) -> Result<Vec<[f32; 3]>> {
    values
        .iter()
        .enumerate()
        .map(|(index, value)| {
            let converted = [value[0] as f32, value[1] as f32, value[2] as f32];
            if value.iter().all(|component| component.is_finite())
                && converted.iter().all(|component| component.is_finite())
            {
                Ok(converted)
            } else {
                invalid_mesh(path, format!("{label} {index} contains a non-finite value"))
            }
        })
        .collect()
}

fn vec2_f32(path: &str, label: &str, values: &[[f64; 2]]) -> Result<Vec<[f32; 2]>> {
    values
        .iter()
        .enumerate()
        .map(|(index, value)| {
            let converted = [value[0] as f32, value[1] as f32];
            if value.iter().all(|component| component.is_finite())
                && converted.iter().all(|component| component.is_finite())
            {
                Ok(converted)
            } else {
                invalid_mesh(path, format!("{label} {index} contains a non-finite value"))
            }
        })
        .collect()
}

fn position_bounds(positions: &[[f32; 3]]) -> ([f32; 3], [f32; 3]) {
    let mut minimum = positions[0];
    let mut maximum = positions[0];
    for position in &positions[1..] {
        for component in 0..3 {
            minimum[component] = minimum[component].min(position[component]);
            maximum[component] = maximum[component].max(position[component]);
        }
    }
    (minimum, maximum)
}

fn append_f32_vec3(
    path: &str,
    binary: &mut Vec<u8>,
    views: &mut Vec<GltfBufferView>,
    accessors: &mut Vec<GltfAccessor>,
    values: &[[f32; 3]],
    minimum: Option<[f32; 3]>,
    maximum: Option<[f32; 3]>,
) -> Result<u32> {
    let offset = checked_u32(path, binary.len(), "buffer-view offset exceeds u32")?;
    for value in values {
        for component in value {
            binary.extend_from_slice(&component.to_le_bytes());
        }
    }
    let byte_length = values
        .len()
        .checked_mul(12)
        .ok_or_else(|| PipelineError::GlbOverflow {
            path: path.to_owned(),
            what: "VEC3 byte length overflows usize",
        })?;
    let view_index = checked_u32(path, views.len(), "buffer-view count exceeds u32")?;
    views.push(GltfBufferView {
        buffer: 0,
        byte_offset: offset,
        byte_length: checked_u32(path, byte_length, "VEC3 byte length exceeds u32")?,
        target: ARRAY_BUFFER,
    });
    let accessor_index = checked_u32(path, accessors.len(), "accessor count exceeds u32")?;
    accessors.push(GltfAccessor {
        buffer_view: view_index,
        component_type: FLOAT,
        count: checked_u32(path, values.len(), "VEC3 count exceeds u32")?,
        value_type: "VEC3",
        minimum,
        maximum,
    });
    Ok(accessor_index)
}

fn append_f32_vec2(
    path: &str,
    binary: &mut Vec<u8>,
    views: &mut Vec<GltfBufferView>,
    accessors: &mut Vec<GltfAccessor>,
    values: &[[f32; 2]],
) -> Result<u32> {
    let offset = checked_u32(path, binary.len(), "buffer-view offset exceeds u32")?;
    for value in values {
        for component in value {
            binary.extend_from_slice(&component.to_le_bytes());
        }
    }
    let byte_length = values
        .len()
        .checked_mul(8)
        .ok_or_else(|| PipelineError::GlbOverflow {
            path: path.to_owned(),
            what: "VEC2 byte length overflows usize",
        })?;
    let view_index = checked_u32(path, views.len(), "buffer-view count exceeds u32")?;
    views.push(GltfBufferView {
        buffer: 0,
        byte_offset: offset,
        byte_length: checked_u32(path, byte_length, "VEC2 byte length exceeds u32")?,
        target: ARRAY_BUFFER,
    });
    let accessor_index = checked_u32(path, accessors.len(), "accessor count exceeds u32")?;
    accessors.push(GltfAccessor {
        buffer_view: view_index,
        component_type: FLOAT,
        count: checked_u32(path, values.len(), "VEC2 count exceeds u32")?,
        value_type: "VEC2",
        minimum: None,
        maximum: None,
    });
    Ok(accessor_index)
}

fn append_indices(
    path: &str,
    binary: &mut Vec<u8>,
    views: &mut Vec<GltfBufferView>,
    accessors: &mut Vec<GltfAccessor>,
    indices: &[u32],
) -> Result<u32> {
    align_to_four(binary, 0);
    let offset = checked_u32(path, binary.len(), "index buffer-view offset exceeds u32")?;
    let use_u16 = indices.iter().all(|index| *index <= u16::MAX as u32);
    if use_u16 {
        for triangle in indices.chunks_exact(3) {
            for index in [triangle[0], triangle[2], triangle[1]] {
                binary.extend_from_slice(&(index as u16).to_le_bytes());
            }
        }
    } else {
        for triangle in indices.chunks_exact(3) {
            for index in [triangle[0], triangle[2], triangle[1]] {
                binary.extend_from_slice(&index.to_le_bytes());
            }
        }
    }
    let element_width = if use_u16 { 2 } else { 4 };
    let byte_length =
        indices
            .len()
            .checked_mul(element_width)
            .ok_or_else(|| PipelineError::GlbOverflow {
                path: path.to_owned(),
                what: "index byte length overflows usize",
            })?;
    let view_index = checked_u32(path, views.len(), "buffer-view count exceeds u32")?;
    views.push(GltfBufferView {
        buffer: 0,
        byte_offset: offset,
        byte_length: checked_u32(path, byte_length, "index byte length exceeds u32")?,
        target: ELEMENT_ARRAY_BUFFER,
    });
    let accessor_index = checked_u32(path, accessors.len(), "accessor count exceeds u32")?;
    accessors.push(GltfAccessor {
        buffer_view: view_index,
        component_type: if use_u16 {
            UNSIGNED_SHORT
        } else {
            UNSIGNED_INT
        },
        count: checked_u32(path, indices.len(), "index count exceeds u32")?,
        value_type: "SCALAR",
        minimum: None,
        maximum: None,
    });
    align_to_four(binary, 0);
    Ok(accessor_index)
}

fn checked_u32(path: &str, value: usize, what: &'static str) -> Result<u32> {
    u32::try_from(value).map_err(|_| PipelineError::GlbOverflow {
        path: path.to_owned(),
        what,
    })
}

fn invalid_mesh<T>(path: &str, reason: String) -> Result<T> {
    Err(PipelineError::InvalidMesh {
        path: path.to_owned(),
        reason,
    })
}

fn align_to_four(bytes: &mut Vec<u8>, padding: u8) {
    while !bytes.len().is_multiple_of(4) {
        bytes.push(padding);
    }
}

fn push_u32(bytes: &mut Vec<u8>, value: u32) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct GltfRoot {
    asset: GltfAsset,
    scene: u32,
    scenes: Vec<GltfScene>,
    nodes: Vec<GltfNode>,
    meshes: Vec<GltfMesh>,
    buffers: Vec<GltfBuffer>,
    buffer_views: Vec<GltfBufferView>,
    accessors: Vec<GltfAccessor>,
}

#[derive(Serialize)]
struct GltfAsset {
    generator: String,
    version: String,
}

#[derive(Serialize)]
struct GltfScene {
    nodes: Vec<u32>,
}

#[derive(Serialize)]
struct GltfNode {
    mesh: u32,
    name: String,
}

#[derive(Serialize)]
struct GltfMesh {
    name: String,
    primitives: Vec<GltfPrimitive>,
}

#[derive(Serialize)]
struct GltfPrimitive {
    attributes: BTreeMap<&'static str, u32>,
    indices: u32,
    mode: u32,
    extras: GltfPrimitiveExtras,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct GltfPrimitiveExtras {
    ffone_submesh: usize,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct GltfBuffer {
    byte_length: u32,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct GltfBufferView {
    buffer: u32,
    byte_offset: u32,
    byte_length: u32,
    target: u32,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct GltfAccessor {
    buffer_view: u32,
    component_type: u32,
    count: u32,
    #[serde(rename = "type")]
    value_type: &'static str,
    #[serde(rename = "min", skip_serializing_if = "Option::is_none")]
    minimum: Option<[f32; 3]>,
    #[serde(rename = "max", skip_serializing_if = "Option::is_none")]
    maximum: Option<[f32; 3]>,
}
