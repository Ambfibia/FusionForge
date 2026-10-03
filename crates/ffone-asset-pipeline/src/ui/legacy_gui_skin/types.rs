use super::*;

#[derive(Clone, Debug)]
pub struct LegacyGuiSkinConversionOptions {
    pub dump_object_all_json: PathBuf,
    pub output_json: PathBuf,
    pub source_build: String,
}

impl LegacyGuiSkinConversionOptions {
    pub fn new(
        dump_object_all_json: impl Into<PathBuf>,
        output_json: impl Into<PathBuf>,
        source_build: impl Into<String>,
    ) -> Self {
        Self {
            dump_object_all_json: dump_object_all_json.into(),
            output_json: output_json.into(),
            source_build: source_build.into(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LegacyGuiSkinConversionReport {
    pub source_build: String,
    pub skins: usize,
    pub evidence_level: String,
    pub publication_allowed: bool,
    pub styles: usize,
    pub referenced_textures: usize,
    pub referenced_fonts: usize,
    pub unresolved_pointer_count: usize,
    pub diagnostics: usize,
    pub output: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LegacyGuiSkinCandidate {
    pub schema: String,
    pub evidence_level: String,
    pub publication_allowed: bool,
    pub limitations: Vec<String>,
    pub source_build: String,
    pub source_assets: Vec<String>,
    pub unresolved_pointer_count: usize,
    pub diagnostics: Vec<LegacyGuiSkinCandidateDiagnostic>,
    pub skins: Vec<LegacyGuiSkin>,
}

pub type LegacyGuiSkinContract = LegacyGuiSkinCandidate;

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LegacyGuiSkinCandidateDiagnostic {
    pub code: String,
    pub skin: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub style: Option<String>,
    pub field: String,
    pub message: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LegacyGuiSkin {
    pub name: String,
    pub path_id: i64,
    pub source_asset: String,
    /// Unity `GUISkin.m_Font`. Styles with a null `m_Font` inherit this.
    pub font: LegacyGuiObjectPointer,
    pub built_in_styles: BTreeMap<String, LegacyGuiStyle>,
    pub custom_styles: Vec<LegacyGuiStyle>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LegacyGuiStyle {
    pub name: String,
    pub font: LegacyGuiObjectPointer,
    pub states: BTreeMap<String, LegacyGuiStyleState>,
    pub border: LegacyGuiInsets,
    pub margin: LegacyGuiInsets,
    pub padding: LegacyGuiInsets,
    pub overflow: LegacyGuiInsets,
    pub content_offset: LegacyGuiVector2,
    pub image_position: i64,
    pub alignment: i64,
    pub word_wrap: i64,
    pub text_clipping: i64,
    pub fixed_width: f64,
    pub fixed_height: f64,
    pub stretch_width: i64,
    pub stretch_height: i64,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LegacyGuiInsets {
    pub left: i64,
    pub right: i64,
    pub top: i64,
    pub bottom: i64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct LegacyGuiVector2 {
    pub x: f64,
    pub y: f64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct LegacyGuiColor {
    pub r: f64,
    pub g: f64,
    pub b: f64,
    pub a: f64,
}
