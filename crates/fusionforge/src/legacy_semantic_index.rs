//! Reverse semantic ownership hints derived from the live legacy TableData bundle.
//!
//! Paths in old FusionFall bundles are not sufficient to distinguish NPC, Nano,
//! wearable-item and player resources.  The already-patched `TableData.resourceFile`
//! is the authoritative source for those relationships.  This module deliberately
//! has no dependency on the repacker so it can be built and tested independently.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

use serde::Serialize;

use crate::fusionforge::{object_name, Asset, UnityValue};

const TABLE_DATA_BUNDLE: &str = "TableData.resourceFile";
const ALL_HNPC_PATH_ID: i64 = 6;
const XDTDATAS_PATH_ID: i64 = 7;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum SemanticFamily {
    Npc,
    Nano,
    Items,
    Hnpc,
    Player,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum OwnerHint {
    None,
    One(String),
    Shared,
}

impl OwnerHint {
    fn merge(&mut self, owner: Option<&str>) {
        let Some(owner) = owner.filter(|value| !value.is_empty()) else {
            return;
        };
        match self {
            Self::None => *self = Self::One(owner.to_string()),
            Self::One(current) if current == owner => {}
            Self::One(_) => *self = Self::Shared,
            Self::Shared => {}
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SemanticHint {
    /// Every semantic family that reaches this public AssetLoader route.
    pub families: BTreeSet<SemanticFamily>,
    /// Ownership is intentionally tracked per family.  A route shared by one NPC
    /// and one Nano must retain both families without turning either owner into
    /// `Shared`; two different NPC owners do turn the NPC owner into `Shared`.
    pub owners: BTreeMap<SemanticFamily, OwnerHint>,
}

impl SemanticHint {
    pub fn owner(&self, family: SemanticFamily) -> OwnerHint {
        self.owners.get(&family).cloned().unwrap_or(OwnerHint::None)
    }

    pub fn is_cross_family(&self) -> bool {
        self.families.len() > 1
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SemanticIndexCounts {
    pub routes: usize,
    pub routes_by_family: BTreeMap<SemanticFamily, usize>,
    pub cross_family_routes: usize,
    pub shared_owner_routes: usize,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SemanticIndex {
    hints: BTreeMap<String, SemanticHint>,
}

impl SemanticIndex {
    /// Build from the already-patched runtime output, never from the pristine source
    /// build.  This is important when custom TableData rows were added by the pipeline.
    pub fn build(out_dir: &Path) -> Result<Self, String> {
        let bundle = out_dir.join(TABLE_DATA_BUNDLE);
        if !bundle.is_file() {
            return Err(format!("{} was not found", bundle.display()));
        }

        let temp = crate::native_build_temp_dir("legacy_semantic_index")?;
        crate::extract_bundle_native_to_dir(&bundle, temp.path())?;
        let mut files = Vec::new();
        collect_files(temp.path(), &mut files)?;
        files.sort();

        let mut xdtdatas = None;
        let mut all_hnpc = None;
        for path in files {
            let Ok(asset) = Asset::from_path(&path) else {
                continue;
            };
            collect_table_roots(&asset, &mut xdtdatas, &mut all_hnpc)?;
            if xdtdatas.is_some() && all_hnpc.is_some() {
                break;
            }
        }

        let xdtdatas = xdtdatas.ok_or_else(|| {
            format!(
                "{} does not contain xdtdatas (expected pathId {})",
                bundle.display(),
                XDTDATAS_PATH_ID
            )
        })?;
        let all_hnpc = all_hnpc.ok_or_else(|| {
            format!(
                "{} does not contain all_hnpc (expected pathId {})",
                bundle.display(),
                ALL_HNPC_PATH_ID
            )
        })?;
        Self::from_table_values(&xdtdatas, &all_hnpc)
    }

    /// Build from parsed Unity values.  Kept as a first-class API both for focused
    /// validation and for callers that already parsed TableData for another stage.
    pub fn from_table_values(xdtdatas: &UnityValue, all_hnpc: &UnityValue) -> Result<Self, String> {
        if xdtdatas.as_object().is_none() {
            return Err("xdtdatas root is not a Unity object".to_string());
        }
        if all_hnpc.as_object().is_none() {
            return Err("all_hnpc root is not a Unity object".to_string());
        }

        let mut index = Self::default();
        index.add_npc_table(xdtdatas);
        index.add_nano_table(xdtdatas);
        let item_owned_routes = index.add_item_tables(xdtdatas);
        index.add_hnpc_table(all_hnpc, &item_owned_routes);
        index.add_player_base();
        Ok(index)
    }

    /// Lookups accept either an already-normalized route or the original mixed-case,
    /// backslash form used by some legacy table rows.
    pub fn hint(&self, normalized_path: &str) -> Option<&SemanticHint> {
        self.hints.get(&normalize_path(normalized_path))
    }

    pub fn len(&self) -> usize {
        self.hints.len()
    }

    pub fn is_empty(&self) -> bool {
        self.hints.is_empty()
    }

    pub fn routes(&self) -> impl Iterator<Item = (&str, &SemanticHint)> {
        self.hints.iter().map(|(path, hint)| (path.as_str(), hint))
    }

    pub fn counts(&self) -> SemanticIndexCounts {
        let mut routes_by_family = BTreeMap::new();
        let mut cross_family_routes = 0;
        let mut shared_owner_routes = 0;
        for hint in self.hints.values() {
            if hint.is_cross_family() {
                cross_family_routes += 1;
            }
            if hint
                .owners
                .values()
                .any(|owner| matches!(owner, OwnerHint::Shared))
            {
                shared_owner_routes += 1;
            }
            for family in &hint.families {
                *routes_by_family.entry(*family).or_insert(0) += 1;
            }
        }
        SemanticIndexCounts {
            routes: self.hints.len(),
            routes_by_family,
            cross_family_routes,
            shared_owner_routes,
        }
    }

    /// Normalize, deduplicate and return routes for which TableData supplies no
    /// semantic evidence.  The repacker can use this for its explicit audit report.
    pub fn unclassified<I, S>(&self, routes: I) -> Vec<String>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        routes
            .into_iter()
            .map(|path| normalize_path(path.as_ref()))
            .filter(|path| !path.is_empty() && !self.hints.contains_key(path))
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect()
    }

    fn add(&mut self, path: String, family: SemanticFamily, owner: Option<&str>) {
        if path.is_empty() {
            return;
        }
        let hint = self.hints.entry(path).or_insert_with(|| SemanticHint {
            families: BTreeSet::new(),
            owners: BTreeMap::new(),
        });
        hint.families.insert(family);
        hint.owners
            .entry(family)
            .or_insert(OwnerHint::None)
            .merge(owner);
    }

    fn add_field_route(
        &mut self,
        row: &UnityValue,
        field: &str,
        prefix: &str,
        extension: &str,
        family: SemanticFamily,
        owner: &str,
    ) -> Option<String> {
        let path = route_from_field(row, field, prefix, extension)?;
        self.add(path.clone(), family, Some(owner));
        Some(path)
    }

    fn add_npc_table(&mut self, xdtdatas: &UnityValue) {
        let Some(table) = xdtdatas.get("m_pNpcTable") else {
            return;
        };
        let rows = value_array(table.get("m_pNpcData"));
        let meshes = value_array(table.get("m_pNpcMeshData"));
        let mesh_indices = rows
            .iter()
            .filter(|row| int_field(row, "m_iHNpc") == Some(0))
            .filter_map(|row| positive_index(row, "m_iMesh"))
            .collect::<BTreeSet<_>>();

        for mesh_index in mesh_indices {
            let Some(mesh) = meshes.get(mesh_index) else {
                continue;
            };
            let model = string_field(mesh, "m_pstrMMeshModelString")
                .and_then(asset_stem)
                .unwrap_or_else(|| mesh_index.to_string());
            let owner = format!("npcmesh:{model}");
            self.add_field_route(
                mesh,
                "m_pstrMMeshModelString",
                "mob",
                "kfm",
                SemanticFamily::Npc,
                &owner,
            );
            for field in ["m_pstrMTextureString", "m_pstrMTextureString2"] {
                self.add_field_route(mesh, field, "texture", "dds", SemanticFamily::Npc, &owner);
            }
            // These misleadingly named F-fields are the NPC hurt SFX lookups.
            for field in [
                "m_pstrFMeshModelString",
                "m_pstrFTextureString",
                "m_pstrFTextureString2",
            ] {
                self.add_field_route(mesh, field, "sound", "wav", SemanticFamily::Npc, &owner);
            }
        }
    }

    fn add_nano_table(&mut self, xdtdatas: &UnityValue) {
        let Some(table) = xdtdatas.get("m_pNanoTable") else {
            return;
        };
        let rows = value_array(table.get("m_pNanoData"));
        let meshes = value_array(table.get("m_pNanoMeshData"));
        let mesh_indices = rows
            .iter()
            .filter_map(|row| positive_index(row, "m_iMesh"))
            .collect::<BTreeSet<_>>();

        for mesh_index in mesh_indices {
            let Some(mesh) = meshes.get(mesh_index) else {
                continue;
            };
            let model = string_field(mesh, "m_pstrMMeshModelString")
                .and_then(asset_stem)
                .unwrap_or_else(|| mesh_index.to_string());
            let owner = format!("nano:{model}");
            self.add_field_route(
                mesh,
                "m_pstrMMeshModelString",
                "nano",
                "kfm",
                SemanticFamily::Nano,
                &owner,
            );
            for field in ["m_pstrMTextureString", "m_pstrMTextureString2"] {
                self.add_field_route(mesh, field, "texture", "dds", SemanticFamily::Nano, &owner);
            }
        }
    }

    fn add_item_tables(&mut self, xdtdatas: &UnityValue) -> BTreeSet<String> {
        let mut item_owned_routes = BTreeSet::new();
        let Some(tables) = xdtdatas.as_object() else {
            return item_owned_routes;
        };

        for (table_name, table) in tables {
            let item_rows = value_array(table.get("m_pItemData"));
            let meshes = value_array(table.get("m_pItemMeshData"));
            if item_rows.is_empty() || meshes.is_empty() {
                continue;
            }
            let sounds = value_array(table.get("m_pItemSoundData"));
            let special_face_head = table_name.eq_ignore_ascii_case("m_pFaceItemTable")
                || table_name.eq_ignore_ascii_case("m_pHeadItemTable");
            let normalized_table = table_name.to_ascii_lowercase();

            for item in item_rows {
                let mesh_index = nonnegative_index(item, "m_iMesh").unwrap_or(0);
                let owner = format!("item:{normalized_table}:{mesh_index}");
                if mesh_index > 0 {
                    if let Some(mesh) = meshes.get(mesh_index) {
                        for field in ["m_pstrFMeshModelString", "m_pstrMMeshModelString"] {
                            if let Some(path) = self.add_field_route(
                                mesh,
                                field,
                                "wear",
                                "nif",
                                SemanticFamily::Items,
                                &owner,
                            ) {
                                item_owned_routes.insert(path);
                            }
                            if special_face_head {
                                self.add_face_head_model_aliases(
                                    mesh,
                                    field,
                                    &owner,
                                    &mut item_owned_routes,
                                );
                            }
                        }
                        for field in [
                            "m_pstrFTextureString",
                            "m_pstrFTextureString2",
                            "m_pstrMTextureString",
                            "m_pstrMTextureString2",
                        ] {
                            if let Some(path) = self.add_field_route(
                                mesh,
                                field,
                                "texture",
                                "dds",
                                SemanticFamily::Items,
                                &owner,
                            ) {
                                item_owned_routes.insert(path);
                            }
                            if special_face_head {
                                self.add_face_head_texture_aliases(
                                    mesh,
                                    field,
                                    &owner,
                                    &mut item_owned_routes,
                                );
                            }
                        }
                    }
                }

                for sound_index in [
                    positive_index(item, "m_iSound1"),
                    positive_index(item, "m_iSound2"),
                ]
                .into_iter()
                .flatten()
                .collect::<BTreeSet<_>>()
                {
                    let Some(sound) = sounds.get(sound_index) else {
                        continue;
                    };
                    for field in [
                        "m_pstrSoundString1",
                        "m_pstrSoundString2",
                        "m_pstrSoundString3",
                    ] {
                        if let Some(path) = self.add_field_route(
                            sound,
                            field,
                            "sound",
                            "wav",
                            SemanticFamily::Items,
                            &owner,
                        ) {
                            item_owned_routes.insert(path);
                        }
                    }
                }
            }
        }
        item_owned_routes
    }

    fn add_face_head_model_aliases(
        &mut self,
        mesh: &UnityValue,
        field: &str,
        owner: &str,
        item_owned_routes: &mut BTreeSet<String>,
    ) {
        let Some(value) = string_field(mesh, field) else {
            return;
        };
        let Some(base) = alias_base(value, "nif", &[] as &[&str]) else {
            return;
        };
        for shape in 1..=5 {
            if let Some(path) = make_route("wear", &format!("{base}_type0{shape}"), "nif") {
                self.add(path.clone(), SemanticFamily::Items, Some(owner));
                item_owned_routes.insert(path);
            }
        }
    }

    fn add_face_head_texture_aliases(
        &mut self,
        mesh: &UnityValue,
        field: &str,
        owner: &str,
        item_owned_routes: &mut BTreeSet<String>,
    ) {
        let Some(value) = string_field(mesh, field) else {
            return;
        };
        let Some(base) = alias_base(value, "dds", &["_a", "_b", "_c", "_d", "_e"]) else {
            return;
        };
        for color in ['a', 'b', 'c', 'd', 'e'] {
            if let Some(path) = make_route("texture", &format!("{base}_{color}"), "dds") {
                self.add(path.clone(), SemanticFamily::Items, Some(owner));
                item_owned_routes.insert(path);
            }
        }
    }

    fn add_hnpc_table(&mut self, all_hnpc: &UnityValue, item_owned_routes: &BTreeSet<String>) {
        const GROUPS: &[(&str, &[&str])] = &[
            ("strHairMesh", &["strHairTexture"]),
            ("strFaceMesh", &["strFaceTexture"]),
            ("strShirtMesh", &["strShirtTexture"]),
            ("strPantsMesh", &["strPantsTexture"]),
            ("strShoesMesh", &["strShoesTexture"]),
            ("strHatMesh", &["strHatTextureM", "strHatTextureS"]),
            ("strGlassMesh", &["strGlassTextureM", "strGlassTextureS"]),
            ("strBackMesh", &["strBackTextureM", "strBackTextureS"]),
            (
                "strLHandWpnMesh",
                &["strLHandWpnTextureM", "strLHandWpnTextureS"],
            ),
            (
                "strRHandWpnMesh",
                &["strRHandWpnTextureM", "strRHandWpnTextureS"],
            ),
        ];

        for row in value_array(all_hnpc.get("TableElement")) {
            for (mesh_field, texture_fields) in GROUPS {
                let mesh_value = string_field(row, mesh_field);
                let owner_stem = mesh_value
                    .and_then(asset_stem)
                    .or_else(|| {
                        texture_fields
                            .iter()
                            .find_map(|field| string_field(row, field).and_then(asset_stem))
                    })
                    .unwrap_or_else(|| "unknown".to_string());
                let owner = format!("hnpcwear:{owner_stem}");
                if let Some(path) = mesh_value.and_then(|value| make_route("wear", value, "nif")) {
                    self.add_hnpc_route(path, &owner, item_owned_routes);
                }
                for field in *texture_fields {
                    if let Some(path) = route_from_field(row, field, "texture", "dds") {
                        self.add_hnpc_route(path, &owner, item_owned_routes);
                    }
                }
            }
        }
    }

    fn add_hnpc_route(&mut self, path: String, owner: &str, item_owned_routes: &BTreeSet<String>) {
        // HNPCs can consume the exact same route as a player wearable.  Preserve
        // the canonical item-table family/owner for those shared assets instead
        // of duplicating them into an HNPC pack.  Routes that exist only in the
        // HNPC table form their own character-oriented family, never the NPC one.
        if item_owned_routes.contains(&path) {
            self.add(path, SemanticFamily::Items, None);
        } else {
            self.add(path, SemanticFamily::Hnpc, Some(owner));
        }
    }

    fn add_player_base(&mut self) {
        let owner = "player:actor";
        for (prefix, value, extension) in [
            ("actor", "m", "kfm"),
            ("actor", "w", "kfm"),
            ("texture", "m_skin", "dds"),
            ("texture", "f_skin", "dds"),
        ] {
            if let Some(path) = make_route(prefix, value, extension) {
                self.add(path, SemanticFamily::Player, Some(owner));
            }
        }
    }
}

fn collect_files(root: &Path, output: &mut Vec<PathBuf>) -> Result<(), String> {
    for entry in fs::read_dir(root).map_err(|err| format!("{}: {err}", root.display()))? {
        let entry = entry.map_err(|err| format!("{}: {err}", root.display()))?;
        let path = entry.path();
        if path.is_dir() {
            collect_files(&path, output)?;
        } else if path.is_file() {
            output.push(path);
        }
    }
    Ok(())
}

fn collect_table_roots(
    asset: &Asset,
    xdtdatas: &mut Option<UnityValue>,
    all_hnpc: &mut Option<UnityValue>,
) -> Result<(), String> {
    let mut objects = asset.objects.values().collect::<Vec<_>>();
    objects.sort_by_key(|info| match info.path_id {
        ALL_HNPC_PATH_ID => 0,
        XDTDATAS_PATH_ID => 1,
        _ => 2,
    });

    for info in objects {
        if asset.object_type_name(info) != "MonoBehaviour" {
            continue;
        }
        if xdtdatas.is_some() && all_hnpc.is_some() {
            break;
        }
        let body = asset.read_object(0, info).map_err(|err| {
            format!(
                "could not parse {}#{} while locating TableData roots: {err}",
                asset.name, info.path_id
            )
        })?;
        match object_name(&body).to_ascii_lowercase().as_str() {
            "xdtdatas" => set_unique_root(xdtdatas, body, "xdtdatas", asset, info.path_id)?,
            "all_hnpc" => set_unique_root(all_hnpc, body, "all_hnpc", asset, info.path_id)?,
            _ => {}
        }
    }
    Ok(())
}

fn set_unique_root(
    slot: &mut Option<UnityValue>,
    value: UnityValue,
    name: &str,
    asset: &Asset,
    path_id: i64,
) -> Result<(), String> {
    if slot.is_some() {
        return Err(format!(
            "duplicate {name} root found at {}#{path_id}",
            asset.name
        ));
    }
    *slot = Some(value);
    Ok(())
}

fn value_array(value: Option<&UnityValue>) -> &[UnityValue] {
    value.and_then(UnityValue::as_array).unwrap_or(&[])
}

fn int_field(value: &UnityValue, key: &str) -> Option<i64> {
    value.get(key).and_then(UnityValue::as_i64)
}

fn positive_index(value: &UnityValue, key: &str) -> Option<usize> {
    nonnegative_index(value, key).filter(|index| *index > 0)
}

fn nonnegative_index(value: &UnityValue, key: &str) -> Option<usize> {
    usize::try_from(int_field(value, key)?).ok()
}

fn string_field<'a>(value: &'a UnityValue, key: &str) -> Option<&'a str> {
    clean_value(value.get(key)?.as_str()?)
}

fn clean_value(value: &str) -> Option<&str> {
    let value = value.trim().trim_matches('"').trim();
    if value.is_empty() || value.eq_ignore_ascii_case("null") || value.eq_ignore_ascii_case("none")
    {
        None
    } else {
        Some(value)
    }
}

fn normalize_path(value: &str) -> String {
    value
        .trim()
        .trim_matches('"')
        .replace('\\', "/")
        .split('/')
        .filter(|part| !part.is_empty() && *part != ".")
        .collect::<Vec<_>>()
        .join("/")
        .to_ascii_lowercase()
}

fn route_from_field(
    row: &UnityValue,
    field: &str,
    prefix: &str,
    extension: &str,
) -> Option<String> {
    make_route(prefix, string_field(row, field)?, extension)
}

fn make_route(prefix: &str, value: &str, extension: &str) -> Option<String> {
    let value = clean_value(value)?;
    let prefix = normalize_path(prefix).trim_matches('/').to_string();
    let mut value = normalize_path(value);
    if value.is_empty() {
        return None;
    }
    let expected_extension = extension.trim_start_matches('.').to_ascii_lowercase();
    let current_extension = value
        .rsplit('/')
        .next()
        .and_then(|name| name.rsplit_once('.'))
        .map(|(_, extension)| extension.to_ascii_lowercase());
    match current_extension.as_deref() {
        Some(current) if current == expected_extension => {}
        Some(current) if matches!(current, "nif" | "kfm" | "dds" | "wav") => {
            value.truncate(value.len().saturating_sub(current.len()));
            value.push_str(&expected_extension);
        }
        Some(_) => {}
        None => {
            value.push('.');
            value.push_str(&expected_extension);
        }
    }
    if value == prefix || value.starts_with(&format!("{prefix}/")) {
        Some(value)
    } else {
        Some(format!("{prefix}/{}", value.trim_start_matches('/')))
    }
}

fn asset_stem(value: &str) -> Option<String> {
    let value = clean_value(value)?;
    let normalized = normalize_path(value);
    let name = normalized.rsplit('/').next()?;
    let stem = name
        .rsplit_once('.')
        .filter(|(_, extension)| matches!(*extension, "nif" | "kfm" | "dds" | "wav"))
        .map(|(stem, _)| stem)
        .unwrap_or(name);
    (!stem.is_empty()).then(|| stem.to_string())
}

fn alias_base(value: &str, extension: &str, suffixes: &[&str]) -> Option<String> {
    let mut stem = asset_stem(value)?;
    let extension = extension.trim_start_matches('.');
    if stem.ends_with(&format!(".{extension}")) {
        stem.truncate(stem.len().saturating_sub(extension.len() + 1));
    }
    for suffix in suffixes {
        if stem.ends_with(suffix) {
            stem.truncate(stem.len().saturating_sub(suffix.len()));
            break;
        }
    }
    for shape in 1..=5 {
        let suffix = format!("_type0{shape}");
        if stem.ends_with(&suffix) {
            stem.truncate(stem.len().saturating_sub(suffix.len()));
            break;
        }
    }
    (!stem.is_empty()).then_some(stem)
}

#[cfg(test)]
mod tests;
