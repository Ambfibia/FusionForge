use super::*;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RuntimeCharacterModel {
    pub id: String,
    pub logical_name: String,
    /// Exact legacy/XDT model stems that resolve to this differently named
    /// Unity root. The GLB and `logical_name` always retain the true root name.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub legacy_aliases: Vec<String>,
    pub category: RuntimeCharacterCategory,
    pub glb: String,
    pub glb_blake3: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub collision: Option<String>,
    pub animations: Vec<String>,
}

pub(super) fn route_from_model_field(row: &Value, field: &str, prefix: &str) -> Option<String> {
    let value = string(row, field)?.trim().trim_matches('"').trim();
    if value.is_empty() || value.eq_ignore_ascii_case("null") || value.eq_ignore_ascii_case("none")
    {
        return None;
    }
    let mut route = normalize_route(value);
    if route.ends_with(".nif") {
        route.truncate(route.len().saturating_sub(3));
        route.push_str("kfm");
    } else if !route.ends_with(".kfm") {
        route.push_str(".kfm");
    }
    let prefix = normalize_route(prefix);
    if route != prefix && !route.starts_with(&format!("{prefix}/")) {
        route = format!("{prefix}/{route}");
    }
    Some(route)
}
