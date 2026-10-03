use super::*;

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct RuntimeModel {
    pub(super) id: String,
    pub(super) logical_name: String,
    #[serde(default)]
    pub(super) legacy_aliases: Vec<String>,
    pub(super) category: String,
    pub(super) glb: String,
    pub(super) glb_blake3: String,
    pub(super) animations: Vec<String>,
}

#[derive(Clone, Debug)]
pub(super) struct ModelTask {
    pub(super) key: String,
    pub(super) model: RuntimeModel,
    pub(super) scale: f64,
    pub(super) animation: Option<String>,
    pub(super) main_texture: Option<String>,
    pub(super) sub_texture: Option<String>,
    pub(super) main_sampler: Option<NativeSampler>,
    pub(super) sub_sampler: Option<NativeSampler>,
    pub(super) screenshot: PathBuf,
    pub(super) report: PathBuf,
    pub(super) log: PathBuf,
    pub(super) relative_screenshot: String,
    pub(super) relative_report: String,
}

pub(super) fn build_missing_model_report(records: &[EntityRecord]) -> Vec<Value> {
    let mut grouped = BTreeMap::<(String, String), Vec<&EntityRecord>>::new();
    for record in records.iter().filter(|record| {
        !matches!(
            record.status.as_str(),
            "rendered"
                | "runtime_available"
                | "render_failed"
                | "not_rendered"
                | "not_rendered_limit"
                | "hidden_location_marker"
                | "primary_interaction_placeholder"
                | "texture_override_unresolved"
        )
    }) {
        grouped
            .entry((record.status.clone(), record.model_stem.clone()))
            .or_default()
            .push(record);
    }
    grouped
        .into_iter()
        .map(|((status, model_stem), rows)| {
            let first = rows[0];
            json!({
                "status": status,
                "modelStem": model_stem,
                "legacyRoute": first.legacy_route,
                "rowCount": rows.len(),
                "npcNumbers": rows.iter().map(|row| row.npc_number).collect::<Vec<_>>(),
                "roles": rows.iter().map(|row| row.role.as_str()).collect::<BTreeSet<_>>(),
                "blockers": first.blockers,
            })
        })
        .collect()
}

pub(super) fn model_name(value: Option<&Value>) -> Option<String> {
    let value = value?.as_str()?.trim();
    (!value.is_empty() && !value.eq_ignore_ascii_case("null")).then(|| value.to_owned())
}
