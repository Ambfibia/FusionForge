//! Deterministic, read-only inventory of logical-model container routes.
//!
//! This module deliberately reads only the cached `bundle-index.json`. It does
//! not open, extract, or interpret any Unity payload. Exact container spelling
//! remains available in every occurrence; normalization is used only to group
//! slash/case variants for collision reporting.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
};

use serde::{Deserialize, Serialize};

pub const LOGICAL_MODEL_CATALOG_SCHEMA: &str = "ffclient.logical-model-catalog.v1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LogicalModelCatalog {
    pub schema: &'static str,
    pub source_index_path: String,
    pub normalization: LogicalModelRouteNormalization,
    pub owner_identity_fields: Vec<&'static str>,
    pub counts: LogicalModelCatalogCounts,
    pub occurrences: Vec<LogicalModelOccurrence>,
    pub route_groups: Vec<LogicalModelRouteGroup>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LogicalModelRouteNormalization {
    pub separators: &'static str,
    pub case: &'static str,
    pub all_other_characters: &'static str,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LogicalModelCatalogCounts {
    pub bundle_count: usize,
    pub asset_count: usize,
    pub bundle_with_logical_models_count: usize,
    pub asset_with_logical_models_count: usize,
    pub distinct_owner_count: usize,
    pub occurrence_count: usize,
    pub kfm_occurrence_count: usize,
    pub nif_occurrence_count: usize,
    pub exact_route_count: usize,
    pub normalized_route_count: usize,
    pub unique_kfm_route_count: usize,
    pub unique_nif_route_count: usize,
    pub duplicate_route_group_count: usize,
    pub case_or_separator_collision_group_count: usize,
    pub sole_owner_group_count: usize,
    pub ambiguous_owner_group_count: usize,
    pub preferred_owner_group_count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum LogicalModelKind {
    Kfm,
    Nif,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LogicalModelOwner {
    pub bundle_path: String,
    pub bundle_name: String,
    pub asset_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LogicalModelOccurrence {
    pub exact_route: String,
    pub normalized_route: String,
    pub kind: LogicalModelKind,
    pub owner: LogicalModelOwner,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LogicalModelRouteGroup {
    pub normalized_route: String,
    pub kind: LogicalModelKind,
    pub occurrence_count: usize,
    pub exact_routes: Vec<String>,
    pub owners: Vec<LogicalModelOwner>,
    pub duplicate_route: bool,
    pub case_or_separator_collision: bool,
    pub ambiguous_owner: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preferred_owner: Option<LogicalModelOwner>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preferred_owner_basis: Option<&'static str>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BundleIndexInput {
    #[serde(default)]
    bundles: Vec<BundleInput>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BundleInput {
    path: String,
    name: String,
    #[serde(default)]
    assets: Vec<AssetInput>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AssetInput {
    name: String,
    #[serde(default)]
    container_paths: Vec<String>,
}

/// Catalogs a cached bundle index without opening any referenced bundle.
pub fn catalog_logical_models_from_path(
    bundle_index_path: impl AsRef<Path>,
) -> Result<LogicalModelCatalog, String> {
    let bundle_index_path = bundle_index_path.as_ref();
    let source = fs::read_to_string(bundle_index_path).map_err(|err| {
        format!(
            "could not read bundle index {}: {err}",
            bundle_index_path.display()
        )
    })?;
    catalog_logical_models_from_json(&source, bundle_index_path.to_string_lossy())
}

/// Catalogs a bundle-index JSON document supplied by the caller.
pub fn catalog_logical_models_from_json(
    source: &str,
    source_index_path: impl Into<String>,
) -> Result<LogicalModelCatalog, String> {
    let index = serde_json::from_str::<BundleIndexInput>(source)
        .map_err(|err| format!("invalid bundle-index JSON: {err}"))?;
    Ok(build_catalog(index, source_index_path.into()))
}

fn build_catalog(index: BundleIndexInput, source_index_path: String) -> LogicalModelCatalog {
    let bundle_count = index.bundles.len();
    let asset_count = index.bundles.iter().map(|bundle| bundle.assets.len()).sum();
    let mut occurrences = Vec::new();

    for bundle in index.bundles {
        for asset in bundle.assets {
            let owner = LogicalModelOwner {
                bundle_path: bundle.path.clone(),
                bundle_name: bundle.name.clone(),
                asset_name: asset.name,
            };
            for exact_route in asset.container_paths {
                let normalized_route = normalize_logical_model_route(&exact_route);
                let Some(kind) = logical_model_kind(&normalized_route) else {
                    continue;
                };
                occurrences.push(LogicalModelOccurrence {
                    exact_route,
                    normalized_route,
                    kind,
                    owner: owner.clone(),
                });
            }
        }
    }

    occurrences.sort_by(|left, right| {
        left.normalized_route
            .cmp(&right.normalized_route)
            .then_with(|| left.exact_route.cmp(&right.exact_route))
            .then_with(|| left.owner.cmp(&right.owner))
    });

    let mut grouped_occurrences = BTreeMap::<String, Vec<usize>>::new();
    for (occurrence_index, occurrence) in occurrences.iter().enumerate() {
        grouped_occurrences
            .entry(occurrence.normalized_route.clone())
            .or_default()
            .push(occurrence_index);
    }

    let mut route_groups = Vec::with_capacity(grouped_occurrences.len());
    for (normalized_route, occurrence_indices) in grouped_occurrences {
        let kind = occurrences[occurrence_indices[0]].kind;
        let exact_routes = occurrence_indices
            .iter()
            .map(|index| occurrences[*index].exact_route.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        let owners = occurrence_indices
            .iter()
            .map(|index| occurrences[*index].owner.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        let sole_owner = (owners.len() == 1).then(|| owners[0].clone());

        route_groups.push(LogicalModelRouteGroup {
            normalized_route,
            kind,
            occurrence_count: occurrence_indices.len(),
            duplicate_route: occurrence_indices.len() > 1,
            case_or_separator_collision: exact_routes.len() > 1,
            ambiguous_owner: owners.len() > 1,
            preferred_owner_basis: sole_owner
                .as_ref()
                .map(|_| "soleDistinctBundlePathBundleNameAndAssetNameOwner"),
            preferred_owner: sole_owner,
            exact_routes,
            owners,
        });
    }

    let exact_route_count = occurrences
        .iter()
        .map(|occurrence| occurrence.exact_route.as_str())
        .collect::<BTreeSet<_>>()
        .len();
    let bundles_with_logical_models = occurrences
        .iter()
        .map(|occurrence| {
            (
                occurrence.owner.bundle_path.as_str(),
                occurrence.owner.bundle_name.as_str(),
            )
        })
        .collect::<BTreeSet<_>>()
        .len();
    let distinct_owner_count = occurrences
        .iter()
        .map(|occurrence| &occurrence.owner)
        .collect::<BTreeSet<_>>()
        .len();
    let kfm_occurrence_count = occurrences
        .iter()
        .filter(|occurrence| occurrence.kind == LogicalModelKind::Kfm)
        .count();
    let nif_occurrence_count = occurrences.len() - kfm_occurrence_count;
    let unique_kfm_route_count = route_groups
        .iter()
        .filter(|group| group.kind == LogicalModelKind::Kfm)
        .count();
    let unique_nif_route_count = route_groups.len() - unique_kfm_route_count;
    let duplicate_route_group_count = route_groups
        .iter()
        .filter(|group| group.duplicate_route)
        .count();
    let case_or_separator_collision_group_count = route_groups
        .iter()
        .filter(|group| group.case_or_separator_collision)
        .count();
    let ambiguous_owner_group_count = route_groups
        .iter()
        .filter(|group| group.ambiguous_owner)
        .count();
    let sole_owner_group_count = route_groups.len() - ambiguous_owner_group_count;

    LogicalModelCatalog {
        schema: LOGICAL_MODEL_CATALOG_SCHEMA,
        source_index_path,
        normalization: LogicalModelRouteNormalization {
            separators: "backslash-to-forward-slash",
            case: "ASCII-lowercase",
            all_other_characters: "preserved",
        },
        owner_identity_fields: vec!["bundlePath", "bundleName", "assetName"],
        counts: LogicalModelCatalogCounts {
            bundle_count,
            asset_count,
            bundle_with_logical_models_count: bundles_with_logical_models,
            asset_with_logical_models_count: distinct_owner_count,
            distinct_owner_count,
            occurrence_count: occurrences.len(),
            kfm_occurrence_count,
            nif_occurrence_count,
            exact_route_count,
            normalized_route_count: route_groups.len(),
            unique_kfm_route_count,
            unique_nif_route_count,
            duplicate_route_group_count,
            case_or_separator_collision_group_count,
            sole_owner_group_count,
            ambiguous_owner_group_count,
            preferred_owner_group_count: sole_owner_group_count,
        },
        occurrences,
        route_groups,
    }
}

pub fn normalize_logical_model_route(exact_route: &str) -> String {
    exact_route.replace('\\', "/").to_ascii_lowercase()
}

fn logical_model_kind(normalized_route: &str) -> Option<LogicalModelKind> {
    if normalized_route.ends_with(".kfm") {
        Some(LogicalModelKind::Kfm)
    } else if normalized_route.ends_with(".nif") {
        Some(LogicalModelKind::Nif)
    } else {
        None
    }
}

#[cfg(test)]
mod tests;
