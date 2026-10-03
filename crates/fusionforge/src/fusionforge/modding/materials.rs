use super::*;

/// The subset of a GLTF PBR material that both the native preview and the
/// legacy Unity 2.x NPC writer can reproduce faithfully.
#[derive(Debug, Clone)]
pub struct ImportedMaterial {
    pub index: usize,
    pub name: String,
    pub base_color: [f64; 4],
    pub emissive: [f64; 3],
    pub metallic: f64,
    pub roughness: f64,
    pub alpha_mode: ImportedMaterialAlphaMode,
    pub alpha_cutoff: Option<f64>,
    pub double_sided: bool,
    pub base_color_texture: Option<ImportedTexture>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportedMaterialAlphaMode {
    Opaque,
    Mask,
    Blend,
}

impl ImportedMaterialAlphaMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Opaque => "opaque",
            Self::Mask => "mask",
            Self::Blend => "blend",
        }
    }
}

impl ImportedMaterial {
    /// Loads the material bound to the first rendered primitive. The clean
    /// legacy writer currently emits one renderer material, so selecting the
    /// actually-used primitive is more reliable than taking the first entry
    /// in the GLTF material table (which may be unused).
    pub fn primary_from_model_path(path: &Path) -> Result<Option<Self>, String> {
        match path
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase()
            .as_str()
        {
            "glb" | "gltf" => {}
            _ => return Ok(None),
        }
        let (document, _, images) =
            gltf::import(path).map_err(|err| format!("{}: {err}", path.display()))?;
        let materials = imported_gltf_materials(&document, &images)?;
        let primary_index = document
            .meshes()
            .flat_map(|mesh| mesh.primitives())
            .find_map(|primitive| primitive.material().index());
        Ok(primary_index
            .and_then(|index| materials.iter().find(|material| material.index == index))
            .cloned())
    }
}
