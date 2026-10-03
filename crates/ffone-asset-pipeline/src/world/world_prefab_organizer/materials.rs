use super::*;

#[derive(Deserialize)]
pub(super) struct MaterialDocument {
    #[serde(default)]
    pub(super) materials: BTreeMap<String, MaterialName>,
}

#[derive(Deserialize)]
pub(super) struct MaterialName {
    #[serde(default)]
    pub(super) name: String,
}
