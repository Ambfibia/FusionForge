use super::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AvatarGender {
    Male,
    Female,
}

impl AvatarGender {
    pub(super) fn from_protocol(value: i8) -> Result<Self, String> {
        match value {
            1 => Ok(Self::Male),
            2 => Ok(Self::Female),
            other => Err(format!(
                "unsupported protocol-0104 gender {other}; expected 1 (male) or 2 (female)"
            )),
        }
    }

    pub(super) const fn token(self) -> &'static str {
        match self {
            Self::Male => "male",
            Self::Female => "female",
        }
    }

    pub(super) const fn field_prefix(self) -> &'static str {
        match self {
            Self::Male => "M",
            Self::Female => "F",
        }
    }

    pub(super) const fn actor_route(self) -> &'static str {
        match self {
            Self::Male => "actor/m.kfm",
            Self::Female => "actor/w.kfm",
        }
    }

    pub(super) const fn skin_route(self) -> &'static str {
        match self {
            Self::Male => "texture/m_skin.dds",
            Self::Female => "texture/f_skin.dds",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AvatarPartKind {
    Body,
    Face,
    Hair,
    UpperBody,
    LowerBody,
    Foot,
    Hat,
    Glasses,
    Back,
    Hand,
    ExtendedHand,
    Vehicle,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AvatarPartParticipation {
    SharedSkeletonBase,
    CombinedSkinnedMesh,
    SocketAttachment,
    BackTablePolicy,
    CombatSecondary,
    SeparateVehicle,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AvatarPartDisposition {
    Ready,
    Missing,
    Empty,
    SuppressedByHat,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VerifiedNativeCandidate {
    pub project_path: String,
    pub source_path: String,
    pub bytes: u64,
    pub blake3: String,
    pub native_key: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AvatarPartResolution {
    pub part: AvatarPartKind,
    pub semantic_name: String,
    pub participation: AvatarPartParticipation,
    pub disposition: AvatarPartDisposition,
    pub table: Option<AvatarTableSelection>,
    pub model: Option<NativeAvatarAssetResolution>,
    pub textures: Vec<NativeAvatarAssetResolution>,
    pub blockers: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AvatarResolutionReport {
    pub schema: String,
    pub complete: bool,
    pub provenance: AvatarCatalogProvenance,
    pub style: AvatarStyleSelection,
    pub height_factor: f32,
    pub body_shape: f32,
    pub parts: Vec<AvatarPartResolution>,
    pub missing_source_assets: Vec<String>,
}

impl AvatarResolutionReport {
    pub fn to_pretty_json(&self) -> Result<String, serde_json::Error> {
        let mut output = serde_json::to_string_pretty(self)?;
        output.push('\n');
        Ok(output)
    }
}

#[derive(Clone, Copy, Debug)]
pub(super) struct HatPolicy {
    pub(super) face_variant: u8,
    pub(super) hair_variant: Option<u8>,
    pub(super) glasses_visible: bool,
}

impl Default for HatPolicy {
    fn default() -> Self {
        Self {
            face_variant: 1,
            hair_variant: Some(1),
            glasses_visible: true,
        }
    }
}

#[derive(Clone, Debug)]
pub(super) struct ResolvedItemRow {
    pub(super) table_name: String,
    pub(super) row_index: usize,
    pub(super) item_number: i64,
    pub(super) mesh_index: usize,
    pub(super) equip_type: i64,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct TableSet {
    pub(super) schema: String,
    pub(super) tables: Vec<TableDocument>,
}

#[derive(Clone, Debug, Deserialize)]
pub(super) struct TableDocument {
    #[allow(dead_code)]
    pub(super) key: String,
    #[allow(dead_code)]
    pub(super) name: String,
    pub(super) value: Value,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct SemanticPlan {
    pub(super) schema: String,
    pub(super) mode: String,
    pub(super) status: String,
    pub(super) production_assets_mutated: bool,
    pub(super) inputs: Vec<PlanInput>,
    pub(super) entities: Vec<PlanEntity>,
    pub(super) models: Vec<PlanModel>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct PlanInput {
    pub(super) role: String,
    pub(super) path: String,
    #[allow(dead_code)]
    pub(super) schema: String,
    pub(super) bytes: u64,
    pub(super) blake3: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct PlanEntity {
    pub(super) id: String,
    pub(super) table_owner: Option<PlanTableOwner>,
    #[allow(dead_code)]
    pub(super) model_routes: Vec<String>,
    pub(super) model_proposal_ids: Vec<String>,
    pub(super) dependencies: Vec<PlanDependency>,
    pub(super) blocker_ids: Vec<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct PlanTableOwner {
    pub(super) table: String,
    pub(super) row_index: usize,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct PlanDependency {
    pub(super) kind: String,
    #[allow(dead_code)]
    pub(super) source_field: String,
    #[allow(dead_code)]
    pub(super) source_m_name: String,
    pub(super) legacy_route: String,
    pub(super) resolution: String,
    pub(super) ownership: String,
    pub(super) candidates: Vec<PlanCandidate>,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct PlanCandidate {
    #[serde(default)]
    pub(super) native_key: Option<String>,
    #[serde(default)]
    pub(super) project_path: Option<String>,
}
