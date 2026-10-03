use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
};

use image::GenericImageView;

use super::unity::UnityValue;

#[cfg(test)]
mod tests;

mod textures;
mod models_imported_mesh;
mod models_apply_mesh_import_with_format_options;
mod animation;
mod types;
mod materials;
mod collision;
mod operations;
mod containers;
mod assets;
mod input;

pub use textures::{TextureImportFormat, ImportedTexture, apply_texture_import};
pub use models_imported_mesh::{
    ImportedMesh, ImportedSubMesh, ImportedModelPreview, gltf_has_skins
};
use models_imported_mesh::{
    imported_gltf_materials, gltf_vec3_to_fusionfall,
    gltf_scale_to_fusionfall, gltf_quat_to_fusionfall
};
#[cfg(test)]
use models_imported_mesh::quantized_vertex_key;
pub use models_apply_mesh_import_with_format_options::{
    apply_mesh_import, apply_mesh_import_with_options,
    apply_mesh_import_uncompressed_with_options
};
use models_apply_mesh_import_with_format_options::{
    find_next_strip_vertex,
    orientation_preserving_third_vertex, gltf_node_paths, parse_obj_vertex
};
pub use animation::{
    ImportedBoneWeight, ImportedAnimationClip, ImportedSkeleton, gltf_animation_names,
    apply_animation_clip_import
};
use animation::{
    gltf_bind_pose_to_fusionfall, normalized_bone_name, fallback_bone_name,
    compressed_bone_index_bits, bone_weight_value
};
pub use types::{
    ImportedMatrix4x4, ImportedVec3Curve, ImportedVec3Key, ImportedQuatCurve, ImportedQuatKey,
    ImportedJoint
};
pub use materials::{ImportedMaterial, ImportedMaterialAlphaMode};
use collision::{
    align_triangle_winding_with_normals, collision_mesh_indices, append_triangle_strips, triangle_strip_neighbor_count,
    same_triangle_vertices
};
use operations::{
    multiply_matrix4, identity_matrix4x4, append_strip,
    estimate_bidirectional_strip_extension, mark_strip_triangles_used,
    undirected_edge_key, compressed_skin_streams, packed_float_vector, packed_int_vector, clear_packed_vector,
    bit_size_for, matrix4x4_value, vector3_array, vector2_array, vec3_curve_value,
    should_keep_scale_curve, quat_curve_value, generated_normals
};
use containers::{packed_float_object, vector3_object};
use assets::parse_index;
use input::parse_f64;
#[cfg(test)]
use models_imported_mesh::dedupe_imported_vertex_streams;
#[cfg(test)]
use collision::build_triangle_strips;
#[cfg(test)]
use operations::{best_strip_join_sequence, strip_to_triangles_u16, quantized_skin_influences};
