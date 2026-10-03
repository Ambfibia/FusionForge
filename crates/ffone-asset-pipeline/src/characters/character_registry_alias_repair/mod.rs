//! Restore the `legacyAliases` a runtime character entry must carry when its
//! package route and its Unity root name differ.
//!
//! `semantic_character_install::legacy_aliases` derives the alias from the
//! legacy route stem, and `ffone-client`'s `network_world_runtime` resolves an
//! XDT `m_pstrMMeshModelString` against `logicalName` *or* any alias. Entries
//! installed through the older single-model and tutorial-promotion paths were
//! written with an empty alias list, so an XDT name such as `npc_key` cannot
//! reach the installed `npc_icekey` GLB and its NPCs render nothing.
//!
//! This repair only rewrites the generated registry. It never touches a GLB, a
//! texture, or any other payload.
//!
//! The runtime lookup is an exact, case-sensitive map probe on the XDT string,
//! so the alias has to carry the table's own spelling. The package route alone
//! is not always enough: `characters/fusions/fusion_belladonnaphase1/` is
//! lowercase while the table says `fusion_belladonnaPhase1`. The repair
//! therefore reads the installed table set and adopts every XDT mesh name that
//! matches the package route case-insensitively, falling back to the route
//! itself when the table names it exactly or not at all.

use std::{
    collections::BTreeSet,
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

use serde::{Deserialize, Serialize};

use crate::{
    PipelineError, Result, SEMANTIC_CHARACTER_REGISTRY_PATH, SEMANTIC_CHARACTER_REGISTRY_SCHEMA,
    SemanticCharacterRegistry, error::io_at,
};

static TRANSACTION_SEQUENCE: AtomicU64 = AtomicU64::new(0);

pub const CHARACTER_REGISTRY_ALIAS_REPAIR_SCHEMA: &str = "ffone.character-registry-alias-repair.v1";

/// Installed table set, relative to the asset root.
const TABLE_SET_PATH: &str = "data/tables/table-set.json";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CharacterRegistryAliasRepairOptions {
    pub asset_root: PathBuf,
    pub apply: bool,
}

impl CharacterRegistryAliasRepairOptions {
    pub fn new(asset_root: impl Into<PathBuf>, apply: bool) -> Self {
        Self {
            asset_root: asset_root.into(),
            apply,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CharacterRegistryAliasChange {
    pub id: String,
    pub logical_name: String,
    pub glb: String,
    /// Package directory of the installed GLB. This is the legacy route stem the
    /// installer derives its alias from, and the identity XDT rows use.
    pub package_route: String,
    pub added_aliases: Vec<String>,
    /// True when the alias spelling came from the installed table set rather
    /// than from the package directory name.
    pub from_table_set: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CharacterRegistryAliasRepairReport {
    pub schema: String,
    pub registry: String,
    pub applied: bool,
    /// Distinct mesh names collected from the installed table set, or zero when
    /// no table set is present beside the registry.
    pub table_set_mesh_names: u64,
    pub models: u64,
    /// Entries whose package route already equals `logicalName`; they need no alias.
    pub canonical: u64,
    /// Entries that already carried the required alias.
    pub already_aliased: u64,
    pub changes: Vec<CharacterRegistryAliasChange>,
    pub registry_blake3_before: String,
    pub registry_blake3_after: String,
}

pub fn repair_character_registry_aliases(
    options: &CharacterRegistryAliasRepairOptions,
) -> Result<CharacterRegistryAliasRepairReport> {
    let asset_root = canonical_plain_directory(&options.asset_root, "asset root")?;
    let registry_path = asset_root.join(native_path(SEMANTIC_CHARACTER_REGISTRY_PATH));
    let registry_before = read_regular(&registry_path)?;
    let mut registry: SemanticCharacterRegistry = serde_json::from_slice(&registry_before)
        .map_err(|source| PipelineError::Json {
            path: registry_path.display().to_string(),
            source,
        })?;
    if registry.schema != SEMANTIC_CHARACTER_REGISTRY_SCHEMA {
        return invalid("semantic character registry has the wrong schema");
    }

    let table_mesh_names =
        read_table_set_mesh_names(&asset_root.join(native_path(TABLE_SET_PATH)))?;

    let mut changes = Vec::new();
    let mut canonical = 0u64;
    let mut already_aliased = 0u64;
    for model in &mut registry.models {
        let package_route = package_route(&model.glb)?;
        let canonical_package = package_route.eq_ignore_ascii_case(&model.logical_name);
        if canonical_package && !table_mesh_names.iter().any(|name| {
            name.eq_ignore_ascii_case(&package_route)
                && *name != model.logical_name
                && !model.legacy_aliases.contains(name)
        }) {
            canonical += 1;
            continue;
        }

        // The runtime probes the map with the exact XDT string, so adopt the
        // table's spelling wherever it names this package route.
        let matched = table_mesh_names
            .iter()
            .filter(|name| name.eq_ignore_ascii_case(&package_route))
            .cloned()
            .collect::<Vec<_>>();
        let from_table_set = !matched.is_empty();
        let wanted = if from_table_set {
            matched
        } else {
            vec![package_route.clone()]
        };
        let mut missing = wanted
            .into_iter()
            .filter(|name| !canonical_package || *name != model.logical_name)
            .filter(|name| !model.legacy_aliases.iter().any(|alias| alias == name))
            .collect::<Vec<_>>();
        if missing.is_empty() {
            already_aliased += 1;
            continue;
        }
        // The runtime registers a differently cased root automatically only
        // while aliases are empty. Adding the package spelling must not revoke
        // that existing exact route (fusion_Spidermonkey is the real case).
        if canonical_package && package_route != model.logical_name && model.legacy_aliases.is_empty() {
            missing.push(model.logical_name.clone());
            missing.sort();
            missing.dedup();
        }
        for name in &missing {
            validate_file_name(name)?;
        }
        changes.push(CharacterRegistryAliasChange {
            id: model.id.clone(),
            logical_name: model.logical_name.clone(),
            glb: model.glb.clone(),
            package_route: package_route.clone(),
            added_aliases: missing.clone(),
            from_table_set,
        });
        model.legacy_aliases.extend(missing);
        model.legacy_aliases.sort();
        model.legacy_aliases.dedup();
    }

    let registry_after = pretty_json(&registry, SEMANTIC_CHARACTER_REGISTRY_PATH)?;
    let mut report = CharacterRegistryAliasRepairReport {
        schema: CHARACTER_REGISTRY_ALIAS_REPAIR_SCHEMA.to_owned(),
        registry: SEMANTIC_CHARACTER_REGISTRY_PATH.to_owned(),
        applied: false,
        table_set_mesh_names: table_mesh_names.len() as u64,
        models: registry.models.len() as u64,
        canonical,
        already_aliased,
        changes,
        registry_blake3_before: blake3::hash(&registry_before).to_hex().to_string(),
        registry_blake3_after: blake3::hash(&registry_after).to_hex().to_string(),
    };
    if report.changes.is_empty() || !options.apply {
        return Ok(report);
    }

    commit_registry(&registry_path, &registry_before, &registry_after)?;
    report.applied = true;
    Ok(report)
}

/// Every distinct male mesh-model string the installed table set names for an
/// NPC or a Nano. Missing table set is not an error: the repair then falls back
/// to the package route, which is what the installer itself derives.
fn read_table_set_mesh_names(path: &Path) -> Result<BTreeSet<String>> {
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(BTreeSet::new()),
        Err(error) => return Err(io_at(path, error)),
    };
    let document: serde_json::Value =
        serde_json::from_slice(&bytes).map_err(|source| PipelineError::Json {
            path: path.display().to_string(),
            source,
        })?;
    let mut names = BTreeSet::new();
    let tables = document
        .get("tables")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| invalid_error("table set has no tables array"))?;
    for table in tables {
        let Some(value) = table.get("value") else {
            continue;
        };
        for (section, mesh_field) in [
            ("m_pNpcTable", "m_pNpcMeshData"),
            ("m_pNanoTable", "m_pNanoMeshData"),
        ] {
            let Some(rows) = value
                .get(section)
                .and_then(|table| table.get(mesh_field))
                .and_then(serde_json::Value::as_array)
            else {
                continue;
            };
            for row in rows {
                let Some(name) = row
                    .get("m_pstrMMeshModelString")
                    .and_then(serde_json::Value::as_str)
                else {
                    continue;
                };
                let name = name.trim();
                if name.is_empty() || name.eq_ignore_ascii_case("null") {
                    continue;
                }
                if validate_file_name(name).is_ok() {
                    names.insert(name.to_owned());
                }
            }
        }
    }
    Ok(names)
}

/// Package directory of `characters/<category>/<package>/<name>.glb`.
fn package_route(glb: &str) -> Result<String> {
    let normalized = glb.replace('\\', "/");
    let mut parts = normalized.rsplit('/');
    let file = parts
        .next()
        .filter(|value| value.ends_with(".glb"))
        .ok_or_else(|| invalid_error("registry GLB path must end with .glb"))?;
    let _ = file;
    parts
        .next()
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
        .ok_or_else(|| invalid_error("registry GLB path has no package directory"))
}

fn commit_registry(path: &Path, before: &[u8], after: &[u8]) -> Result<()> {
    let token = format!(
        "{}-{}",
        std::process::id(),
        TRANSACTION_SEQUENCE.fetch_add(1, Ordering::Relaxed)
    );
    let next = sibling(path, &token, "next")?;
    let backup = sibling(path, &token, "backup")?;
    write_new(&next, after)?;
    if read_regular(path)? != before {
        remove_file_if_exists(&next);
        return invalid("character registry changed while the repair was staged");
    }
    let result = (|| {
        fs::rename(path, &backup).map_err(|error| io_at(path, error))?;
        if let Err(error) = fs::rename(&next, path) {
            let _ = fs::rename(&backup, path);
            return Err(io_at(&next, error));
        }
        Ok(())
    })();
    if result.is_ok() {
        remove_file_if_exists(&backup);
    }
    remove_file_if_exists(&next);
    result
}

fn native_path(relative: &str) -> PathBuf {
    relative.split('/').collect()
}

fn canonical_plain_directory(path: &Path, label: &str) -> Result<PathBuf> {
    let metadata = fs::symlink_metadata(path).map_err(|error| io_at(path, error))?;
    if !metadata.file_type().is_dir() || metadata.file_type().is_symlink() {
        return invalid(format!("{label} must be a plain directory"));
    }
    fs::canonicalize(path).map_err(|error| io_at(path, error))
}

fn read_regular(path: &Path) -> Result<Vec<u8>> {
    let metadata = fs::symlink_metadata(path).map_err(|error| io_at(path, error))?;
    if !metadata.file_type().is_file() || metadata.file_type().is_symlink() {
        return invalid(format!("{} must be a plain regular file", path.display()));
    }
    fs::read(path).map_err(|error| io_at(path, error))
}

fn pretty_json<T: Serialize>(value: &T, label: &str) -> Result<Vec<u8>> {
    let mut bytes = serde_json::to_vec_pretty(value).map_err(|source| PipelineError::Json {
        path: label.to_owned(),
        source,
    })?;
    bytes.push(b'\n');
    Ok(bytes)
}

fn sibling(path: &Path, token: &str, suffix: &str) -> Result<PathBuf> {
    let parent = path
        .parent()
        .ok_or_else(|| invalid_error("transaction path has no parent"))?;
    let name = path
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| invalid_error("transaction filename is not UTF-8"))?;
    Ok(parent.join(format!(".{name}.alias-repair-{token}.{suffix}")))
}

fn write_new(path: &Path, bytes: &[u8]) -> Result<()> {
    let mut output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|error| io_at(path, error))?;
    output
        .write_all(bytes)
        .map_err(|error| io_at(path, error))?;
    output.sync_all().map_err(|error| io_at(path, error))
}

fn remove_file_if_exists(path: &Path) {
    let _ = fs::remove_file(path);
}

fn validate_file_name(value: &str) -> Result<()> {
    if value.is_empty()
        || value == "."
        || value == ".."
        || value.contains('/')
        || value.contains('\\')
        || value.contains(':')
    {
        return invalid(format!("unsafe registry package route {value:?}"));
    }
    Ok(())
}

fn invalid<T>(message: impl Into<String>) -> Result<T> {
    Err(invalid_error(message))
}

fn invalid_error(message: impl Into<String>) -> PipelineError {
    PipelineError::InvalidManifest(format!(
        "character registry alias repair failed: {}",
        message.into()
    ))
}

pub const CHARACTER_REGISTRY_ALIAS_PLAN_SCHEMA: &str = "ffone.character-registry-alias-plan.v1";

/// One stated alias: the table mesh string `alias` must resolve to the installed
/// model whose Unity root name is `logical_name`.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CharacterRegistryAliasPlanEntry {
    pub logical_name: String,
    pub alias: String,
    /// Why this mapping is true. Free text, carried into the report so the
    /// registry change can be traced back to the source that proved it.
    pub evidence: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CharacterRegistryAliasPlan {
    pub schema: String,
    #[serde(default)]
    pub aliases: Vec<CharacterRegistryAliasPlanEntry>,
    /// Aliases to take off a model. An alias stated here must currently be on
    /// the named model: a plan that removes something already absent is a plan
    /// written against a registry it does not describe.
    #[serde(default)]
    pub remove_aliases: Vec<CharacterRegistryAliasPlanEntry>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CharacterRegistryAliasPlanReport {
    pub schema: String,
    pub registry: String,
    pub applied: bool,
    pub planned: u64,
    pub planned_removals: u64,
    /// Entries whose alias the model already carried.
    pub already_aliased: u64,
    pub changes: Vec<CharacterRegistryAliasChange>,
    pub removals: Vec<CharacterRegistryAliasChange>,
    pub registry_blake3_before: String,
    pub registry_blake3_after: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CharacterRegistryAliasPlanOptions {
    pub asset_root: PathBuf,
    pub plan: PathBuf,
    pub apply: bool,
}

impl CharacterRegistryAliasPlanOptions {
    pub fn new(asset_root: impl Into<PathBuf>, plan: impl Into<PathBuf>, apply: bool) -> Self {
        Self {
            asset_root: asset_root.into(),
            plan: plan.into(),
            apply,
        }
    }
}

/// Add an explicitly evidenced alias to an already installed character model.
///
/// [`repair_character_registry_aliases`] recovers the alias an installer should
/// have derived from the package route. This covers the other direction: a table
/// row that names an installed model by a string no rule can derive from it. The
/// added NPCs of the modded build are the case that forced it -- NPC 3399 selects
/// mesh `npc_3399_tom` while the model it was built from is the `npc_tom` this
/// tree already publishes, so aliasing is right and installing a second copy of
/// the same character is not. Nothing derives `npc_3399_tom` from `npc_tom`, so
/// the caller must state the mapping and say what proves it.
///
/// Every entry is checked against the installed registry and table set: the alias
/// has to be a mesh string the tables actually name, exactly one model may carry
/// the given root name, and an alias already resolving elsewhere is refused
/// rather than moved.
pub fn apply_character_registry_alias_plan(
    options: &CharacterRegistryAliasPlanOptions,
) -> Result<CharacterRegistryAliasPlanReport> {
    let asset_root = canonical_plain_directory(&options.asset_root, "asset root")?;
    let plan: CharacterRegistryAliasPlan = serde_json::from_slice(&read_regular(&options.plan)?)
        .map_err(|source| PipelineError::Json {
            path: options.plan.display().to_string(),
            source,
        })?;
    if plan.schema != CHARACTER_REGISTRY_ALIAS_PLAN_SCHEMA {
        return invalid("alias plan has the wrong schema");
    }
    if plan.aliases.is_empty() && plan.remove_aliases.is_empty() {
        return invalid("alias plan names no aliases");
    }

    let registry_path = asset_root.join(native_path(SEMANTIC_CHARACTER_REGISTRY_PATH));
    let registry_before = read_regular(&registry_path)?;
    let mut registry: SemanticCharacterRegistry = serde_json::from_slice(&registry_before)
        .map_err(|source| PipelineError::Json {
            path: registry_path.display().to_string(),
            source,
        })?;
    if registry.schema != SEMANTIC_CHARACTER_REGISTRY_SCHEMA {
        return invalid("semantic character registry has the wrong schema");
    }

    let table_mesh_names =
        read_table_set_mesh_names(&asset_root.join(native_path(TABLE_SET_PATH)))?;

    let released = plan
        .remove_aliases
        .iter()
        .map(|entry| (entry.logical_name.clone(), entry.alias.clone()))
        .collect::<BTreeSet<_>>();

    let mut seen = BTreeSet::new();
    for entry in &plan.aliases {
        validate_file_name(&entry.alias)?;
        validate_file_name(&entry.logical_name)?;
        if entry.evidence.trim().is_empty() {
            return invalid(format!("alias {:?} carries no evidence", entry.alias));
        }
        if !seen.insert(entry.alias.clone()) {
            return invalid(format!("alias plan names {:?} twice", entry.alias));
        }
        if !table_mesh_names.contains(&entry.alias) {
            return invalid(format!(
                "alias {:?} is no mesh string of the installed table set",
                entry.alias
            ));
        }
        if registry
            .models
            .iter()
            .any(|model| model.logical_name == entry.alias)
        {
            return invalid(format!(
                "alias {:?} is the logical name of an installed model",
                entry.alias
            ));
        }
        // Moving an alias from one model to another is a single intent, so a
        // carrier this same plan releases is not a conflicting owner.
        let foreign_owners = registry
            .models
            .iter()
            .filter(|model| model.logical_name != entry.logical_name)
            .filter(|model| {
                model
                    .legacy_aliases
                    .iter()
                    .any(|alias| *alias == entry.alias)
            })
            .filter(|model| !released.contains(&(model.logical_name.clone(), entry.alias.clone())))
            .count();
        if foreign_owners != 0 {
            return invalid(format!(
                "alias {:?} already resolves to a different model",
                entry.alias
            ));
        }
        if registry
            .models
            .iter()
            .filter(|model| model.logical_name == entry.logical_name)
            .count()
            != 1
        {
            return invalid(format!(
                "alias plan target {:?} is not exactly one installed model",
                entry.logical_name
            ));
        }
    }

    let mut seen_removals = BTreeSet::new();
    for entry in &plan.remove_aliases {
        validate_file_name(&entry.alias)?;
        validate_file_name(&entry.logical_name)?;
        if entry.evidence.trim().is_empty() {
            return invalid(format!(
                "alias removal {:?} carries no evidence",
                entry.alias
            ));
        }
        if !seen_removals.insert((entry.logical_name.clone(), entry.alias.clone())) {
            return invalid(format!("alias plan removes {:?} twice", entry.alias));
        }
        let carriers = registry
            .models
            .iter()
            .filter(|model| model.logical_name == entry.logical_name)
            .filter(|model| {
                model
                    .legacy_aliases
                    .iter()
                    .any(|alias| *alias == entry.alias)
            })
            .count();
        if carriers != 1 {
            return invalid(format!(
                "alias {:?} is not carried by exactly one model named {:?}",
                entry.alias, entry.logical_name
            ));
        }
    }

    let mut removals = Vec::new();
    for entry in &plan.remove_aliases {
        let model = registry
            .models
            .iter_mut()
            .find(|model| {
                model.logical_name == entry.logical_name
                    && model
                        .legacy_aliases
                        .iter()
                        .any(|alias| *alias == entry.alias)
            })
            .ok_or_else(|| invalid_error("alias removal target vanished"))?;
        model.legacy_aliases.retain(|alias| *alias != entry.alias);
        removals.push(CharacterRegistryAliasChange {
            id: model.id.clone(),
            logical_name: model.logical_name.clone(),
            glb: model.glb.clone(),
            package_route: package_route(&model.glb)?,
            added_aliases: vec![entry.alias.clone()],
            from_table_set: true,
        });
    }

    let mut changes = Vec::new();
    let mut already_aliased = 0u64;
    for entry in &plan.aliases {
        let model = registry
            .models
            .iter_mut()
            .find(|model| model.logical_name == entry.logical_name)
            .ok_or_else(|| invalid_error("alias plan target vanished"))?;
        if model
            .legacy_aliases
            .iter()
            .any(|alias| *alias == entry.alias)
        {
            already_aliased += 1;
            continue;
        }
        changes.push(CharacterRegistryAliasChange {
            id: model.id.clone(),
            logical_name: model.logical_name.clone(),
            glb: model.glb.clone(),
            package_route: package_route(&model.glb)?,
            added_aliases: vec![entry.alias.clone()],
            from_table_set: true,
        });
        model.legacy_aliases.push(entry.alias.clone());
        model.legacy_aliases.sort();
        model.legacy_aliases.dedup();
    }

    let registry_after = pretty_json(&registry, SEMANTIC_CHARACTER_REGISTRY_PATH)?;
    let mut report = CharacterRegistryAliasPlanReport {
        schema: CHARACTER_REGISTRY_ALIAS_PLAN_SCHEMA.to_owned(),
        registry: SEMANTIC_CHARACTER_REGISTRY_PATH.to_owned(),
        applied: false,
        planned: plan.aliases.len() as u64,
        planned_removals: plan.remove_aliases.len() as u64,
        already_aliased,
        changes,
        removals,
        registry_blake3_before: blake3::hash(&registry_before).to_hex().to_string(),
        registry_blake3_after: blake3::hash(&registry_after).to_hex().to_string(),
    };
    if (report.changes.is_empty() && report.removals.is_empty()) || !options.apply {
        return Ok(report);
    }

    commit_registry(&registry_path, &registry_before, &registry_after)?;
    report.applied = true;
    Ok(report)
}

#[cfg(test)]
mod tests;
