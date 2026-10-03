use super::super::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(in super::super) struct TerrainTextureLayer {
    pub(in super::super) id: String,
    pub(in super::super) name: String,
    pub(in super::super) color: String,
    pub(in super::super) unity_material_path_id: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(in super::super) struct TerrainDocument {
    pub(in super::super) width: u32,
    pub(in super::super) height: u32,
    pub(in super::super) world_size: f32,
    pub(in super::super) max_height: f32,
    pub(in super::super) heights: Vec<f32>,
    pub(in super::super) texture_resolution: u32,
    pub(in super::super) texture_map: Vec<u8>,
    pub(in super::super) texture_layers: Vec<TerrainTextureLayer>,
    pub(in super::super) preview_texture_data_url: Option<String>,
    pub(in super::super) origin: Option<Vec3>,
    pub(in super::super) axis_x: Option<Vec3>,
    pub(in super::super) axis_z: Option<Vec3>,
    pub(in super::super) height_axis: Option<Vec3>,
    pub(in super::super) flip_x: Option<bool>,
    pub(in super::super) reverse_winding: Option<bool>,
}
