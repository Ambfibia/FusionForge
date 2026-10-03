use std::{
    collections::{BTreeMap, BTreeSet},
    env, fs,
    path::{Path, PathBuf},
};

use serde::Serialize;
use serde_json::{Map as JsonMap, Value as JsonValue};
use sha2::{Digest, Sha256};

const INVENTORY_SCHEMA: &str = "fusionforge.legacy-inventory-diff.v1";
const UNITY_JSON_SCHEMA: &str = "fusionforge.focused-unity-json-diff.v1";
const UNITY_JSON_WARNING: &str =
    "Triage only; verify acceptance-critical identity and bytes against raw containers.";

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
struct InventoryEntry {
    relative_path: String,
    bytes: u64,
    extension: String,
    tags: Vec<String>,
}

#[derive(Debug)]
struct Inventory {
    display_path: String,
    sha256: String,
    entries: BTreeMap<String, InventoryEntry>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct InventoryInput {
    path: String,
    sha256: String,
    entries: usize,
}

#[derive(Debug, Serialize)]
struct InventoryFilter {
    tag: Option<String>,
}

#[derive(Debug, Serialize)]
struct InventoryCounts {
    added: usize,
    removed: usize,
    changed: usize,
}

#[derive(Debug, Serialize)]
struct InventoryChange {
    path: String,
    differences: Vec<String>,
    before: InventoryEntry,
    after: InventoryEntry,
}

#[derive(Debug, Serialize)]
struct InventoryDiffReport {
    schema: &'static str,
    filter: InventoryFilter,
    left: InventoryInput,
    right: InventoryInput,
    counts: InventoryCounts,
    added: Vec<InventoryEntry>,
    removed: Vec<InventoryEntry>,
    changed: Vec<InventoryChange>,
}

#[derive(Debug)]
struct JsonInput {
    display_path: String,
    sha256: String,
    value: JsonValue,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct JsonInputSummary {
    path: String,
    sha256: String,
}

#[derive(Debug, Serialize)]
struct JsonDiffCounts {
    changes: usize,
    emitted: usize,
}

#[derive(Debug, Serialize)]
struct JsonChange {
    operation: String,
    path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    before: Option<JsonValue>,
    #[serde(skip_serializing_if = "Option::is_none")]
    after: Option<JsonValue>,
}

#[derive(Debug, Serialize)]
struct UnityJsonDiffReport {
    schema: &'static str,
    warning: &'static str,
    left: JsonInputSummary,
    right: JsonInputSummary,
    counts: JsonDiffCounts,
    truncated: bool,
    changes: Vec<JsonChange>,
}

fn slash(value: &str) -> String {
    value.replace('\\', "/")
}

fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn display_path(path: &Path) -> Result<String, String> {
    let absolute = fs::canonicalize(path)
        .map_err(|err| format!("could not resolve {}: {err}", path.display()))?;
    let cwd = fs::canonicalize(env::current_dir().map_err(|err| err.to_string())?)
        .map_err(|err| format!("could not resolve current directory: {err}"))?;
    let display = absolute.strip_prefix(&cwd).unwrap_or(&absolute);
    Ok(slash(&display.to_string_lossy()))
}

fn read_utf8(path: &Path) -> Result<(Vec<u8>, String), String> {
    let bytes =
        fs::read(path).map_err(|err| format!("could not read {}: {err}", path.display()))?;
    let text = std::str::from_utf8(&bytes)
        .map_err(|err| format!("{} is not UTF-8: {err}", path.display()))?
        .strip_prefix('\u{feff}')
        .unwrap_or_else(|| std::str::from_utf8(&bytes).expect("UTF-8 was checked"))
        .to_owned();
    Ok((bytes, text))
}

fn parse_inventory(path: &Path, tag: Option<&str>) -> Result<Inventory, String> {
    let (bytes, text) = read_utf8(path)?;
    let lines = text
        .lines()
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>();
    let header = lines
        .first()
        .ok_or_else(|| format!("{} is empty", path.display()))?
        .split('\t')
        .collect::<Vec<_>>();

    let column = |name: &str| {
        header
            .iter()
            .position(|candidate| *candidate == name)
            .ok_or_else(|| format!("{} lacks column {name}", path.display()))
    };
    let relative_path_index = column("relative_path")?;
    let bytes_index = column("bytes")?;
    let extension_index = column("extension")?;
    let tags_index = column("tags")?;

    let mut entries = BTreeMap::new();
    for (offset, line) in lines.iter().skip(1).enumerate() {
        let line_number = offset + 2;
        let fields = line.split('\t').collect::<Vec<_>>();
        let relative_path = slash(fields.get(relative_path_index).copied().unwrap_or_default());
        let byte_count = fields
            .get(bytes_index)
            .copied()
            .unwrap_or_default()
            .parse::<u64>()
            .map_err(|_| format!("{}:{line_number} has an invalid byte count", path.display()))?;
        if relative_path.is_empty() {
            return Err(format!(
                "{}:{line_number} has an empty relative path",
                path.display()
            ));
        }
        let extension = fields
            .get(extension_index)
            .copied()
            .unwrap_or_default()
            .to_lowercase();
        let mut tags = fields
            .get(tags_index)
            .copied()
            .unwrap_or_default()
            .split(',')
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_lowercase)
            .collect::<Vec<_>>();
        tags.sort();

        if tag.is_some_and(|required| !tags.iter().any(|value| value == required)) {
            continue;
        }

        let key = relative_path.to_lowercase();
        let entry = InventoryEntry {
            relative_path,
            bytes: byte_count,
            extension,
            tags,
        };
        if let Some(previous) = entries.insert(key, entry.clone()) {
            return Err(format!(
                "{} has case-colliding duplicate paths: {} and {}",
                path.display(),
                previous.relative_path,
                entry.relative_path
            ));
        }
    }

    Ok(Inventory {
        display_path: display_path(path)?,
        sha256: sha256(&bytes),
        entries,
    })
}

fn compare_inventories(
    left: Inventory,
    right: Inventory,
    tag: Option<String>,
) -> InventoryDiffReport {
    let keys = left
        .entries
        .keys()
        .chain(right.entries.keys())
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut added = Vec::new();
    let mut removed = Vec::new();
    let mut changed = Vec::new();

    for key in keys {
        match (left.entries.get(&key), right.entries.get(&key)) {
            (None, Some(after)) => added.push(after.clone()),
            (Some(before), None) => removed.push(before.clone()),
            (Some(before), Some(after)) => {
                let mut differences = Vec::new();
                if before.relative_path != after.relative_path {
                    differences.push("pathCase".to_string());
                }
                if before.bytes != after.bytes {
                    differences.push("bytes".to_string());
                }
                if before.extension != after.extension {
                    differences.push("extension".to_string());
                }
                if before.tags != after.tags {
                    differences.push("tags".to_string());
                }
                if !differences.is_empty() {
                    changed.push(InventoryChange {
                        path: after.relative_path.clone(),
                        differences,
                        before: before.clone(),
                        after: after.clone(),
                    });
                }
            }
            (None, None) => unreachable!("key came from one of the inventories"),
        }
    }

    InventoryDiffReport {
        schema: INVENTORY_SCHEMA,
        filter: InventoryFilter { tag },
        left: InventoryInput {
            path: left.display_path,
            sha256: left.sha256,
            entries: left.entries.len(),
        },
        right: InventoryInput {
            path: right.display_path,
            sha256: right.sha256,
            entries: right.entries.len(),
        },
        counts: InventoryCounts {
            added: added.len(),
            removed: removed.len(),
            changed: changed.len(),
        },
        added,
        removed,
        changed,
    }
}

fn read_json(path: &Path) -> Result<JsonInput, String> {
    let (bytes, text) = read_utf8(path)?;
    let value = serde_json::from_str(&text)
        .map_err(|err| format!("could not parse {}: {err}", path.display()))?;
    Ok(JsonInput {
        display_path: display_path(path)?,
        sha256: sha256(&bytes),
        value,
    })
}

fn first_present<'a>(
    object: &'a JsonMap<String, JsonValue>,
    names: &[&str],
) -> Option<&'a JsonValue> {
    names.iter().find_map(|name| {
        object.get(*name).filter(|value| {
            !value.is_null() && !matches!(value, JsonValue::String(text) if text.is_empty())
        })
    })
}

fn identity_text(value: &JsonValue) -> Option<String> {
    match value {
        JsonValue::String(value) => Some(value.clone()),
        JsonValue::Number(value) => Some(value.to_string()),
        _ => None,
    }
}

fn identity_for(value: &JsonValue) -> Option<String> {
    let object = value.as_object()?;
    let mut identity = object.clone();
    if let Some(nested) = object.get("identity").and_then(JsonValue::as_object) {
        identity.extend(nested.clone());
    }

    let serialized_asset = first_present(
        &identity,
        &[
            "serializedAsset",
            "serialized_asset",
            "serializedFile",
            "assetFile",
            "assetName",
        ],
    )
    .and_then(identity_text);
    let unity_type = first_present(&identity, &["type", "unityType", "unity_type", "className"])
        .and_then(identity_text);
    let path_id = first_present(&identity, &["pathId", "pathID", "path_id", "m_PathID"])
        .and_then(identity_text);
    let container_route = first_present(
        &identity,
        &["containerRoute", "container_route", "assetBundlePath"],
    )
    .and_then(identity_text);

    if let (Some(serialized_asset), Some(unity_type), Some(path_id)) =
        (serialized_asset, unity_type, path_id)
    {
        return Some(format!(
            "object:{}|{}|{}|{}",
            container_route.unwrap_or_default(),
            serialized_asset,
            unity_type,
            path_id
        ));
    }

    let assembly = first_present(&identity, &["assembly", "assemblyName", "assembly_name"])
        .and_then(identity_text);
    let namespace = first_present(&identity, &["namespace", "namespaceName", "namespace_name"])
        .and_then(identity_text);
    let class_name =
        first_present(&identity, &["class", "className", "class_name"]).and_then(identity_text);

    match (assembly, class_name) {
        (Some(assembly), Some(class_name)) => Some(format!(
            "script:{}|{}|{}",
            assembly,
            namespace.unwrap_or_default(),
            class_name
        )),
        _ => None,
    }
}

fn keyed_array(values: &[JsonValue]) -> Option<BTreeMap<String, &JsonValue>> {
    let mut keyed = BTreeMap::new();
    for value in values {
        let identity = identity_for(value)?;
        if keyed.insert(identity, value).is_some() {
            return None;
        }
    }
    Some(keyed)
}

fn pointer_segment(value: &str) -> String {
    value.replace('~', "~0").replace('/', "~1")
}

#[derive(Debug)]
struct DiffAccumulator {
    total: usize,
    max: usize,
    changes: Vec<JsonChange>,
}

impl DiffAccumulator {
    fn add(
        &mut self,
        operation: &str,
        path: &str,
        before: Option<&JsonValue>,
        after: Option<&JsonValue>,
    ) {
        self.total += 1;
        if self.changes.len() < self.max {
            self.changes.push(JsonChange {
                operation: operation.to_string(),
                path: if path.is_empty() {
                    "/".to_string()
                } else {
                    path.to_string()
                },
                before: before.cloned(),
                after: after.cloned(),
            });
        }
    }

    fn visit(&mut self, before: &JsonValue, after: &JsonValue, path: &str) {
        if before == after {
            return;
        }

        match (before, after) {
            (JsonValue::Array(before), JsonValue::Array(after)) => {
                let before_keyed = keyed_array(before);
                let after_keyed = keyed_array(after);
                if let (Some(before_keyed), Some(after_keyed)) = (before_keyed, after_keyed) {
                    if !before.is_empty() || !after.is_empty() {
                        let keys = before_keyed
                            .keys()
                            .chain(after_keyed.keys())
                            .cloned()
                            .collect::<BTreeSet<_>>();
                        for key in keys {
                            let next_path = format!("{path}/@{}", pointer_segment(&key));
                            match (before_keyed.get(&key), after_keyed.get(&key)) {
                                (None, Some(value)) => {
                                    self.add("add", &next_path, None, Some(value))
                                }
                                (Some(value), None) => {
                                    self.add("remove", &next_path, Some(value), None)
                                }
                                (Some(left), Some(right)) => self.visit(left, right, &next_path),
                                (None, None) => {
                                    unreachable!("key came from one of the arrays")
                                }
                            }
                        }
                        return;
                    }
                }

                let length = before.len().max(after.len());
                for index in 0..length {
                    let next_path = format!("{path}/{index}");
                    match (before.get(index), after.get(index)) {
                        (None, Some(value)) => self.add("add", &next_path, None, Some(value)),
                        (Some(value), None) => self.add("remove", &next_path, Some(value), None),
                        (Some(left), Some(right)) => self.visit(left, right, &next_path),
                        (None, None) => unreachable!("index is below the maximum length"),
                    }
                }
            }
            (JsonValue::Object(before), JsonValue::Object(after)) => {
                let keys = before
                    .keys()
                    .chain(after.keys())
                    .cloned()
                    .collect::<BTreeSet<_>>();
                for key in keys {
                    let next_path = format!("{path}/{}", pointer_segment(&key));
                    match (before.get(&key), after.get(&key)) {
                        (None, Some(value)) => self.add("add", &next_path, None, Some(value)),
                        (Some(value), None) => self.add("remove", &next_path, Some(value), None),
                        (Some(left), Some(right)) => self.visit(left, right, &next_path),
                        (None, None) => unreachable!("key came from one of the objects"),
                    }
                }
            }
            _ => self.add("replace", path, Some(before), Some(after)),
        }
    }
}

fn structural_diff(before: &JsonValue, after: &JsonValue, max: usize) -> DiffAccumulator {
    let mut result = DiffAccumulator {
        total: 0,
        max,
        changes: Vec::new(),
    };
    result.visit(before, after, "");
    result
}

fn absolute_path(path: &Path) -> Result<PathBuf, String> {
    if path.is_absolute() {
        Ok(path.to_path_buf())
    } else {
        Ok(env::current_dir()
            .map_err(|err| err.to_string())?
            .join(path))
    }
}

fn write_report<T: Serialize>(output: &str, report: &T, inputs: &[&Path]) -> Result<bool, String> {
    let json = format!(
        "{}\n",
        serde_json::to_string_pretty(report).map_err(|err| err.to_string())?
    );
    if output == "-" {
        print!("{json}");
        return Ok(true);
    }

    let output_path = Path::new(output);
    let absolute_output = absolute_path(output_path)?;
    for input in inputs {
        if absolute_output == absolute_path(input)? {
            return Err(format!(
                "report output must not overwrite input {}",
                input.display()
            ));
        }
    }
    if let Some(parent) = output_path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)
            .map_err(|err| format!("could not create {}: {err}", parent.display()))?;
    }
    fs::write(output_path, json)
        .map_err(|err| format!("could not write {}: {err}", output_path.display()))?;
    Ok(false)
}

pub(crate) fn compare_inventories_cli(args: &[String]) -> Result<(), String> {
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        println!(
            "compare-inventories <left.tsv> <right.tsv> [--tag <tag>] [--out <report.json|->]"
        );
        return Ok(());
    }

    let mut positional = Vec::new();
    let mut tag = None;
    let mut output = None;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--tag" | "--out" => {
                let option = args[index].as_str();
                index += 1;
                let value = args
                    .get(index)
                    .ok_or_else(|| format!("{option} requires a value"))?
                    .clone();
                if option == "--tag" {
                    let value = value.trim().to_lowercase();
                    if value.is_empty() || tag.replace(value).is_some() {
                        return Err("--tag requires one non-empty value".to_string());
                    }
                } else if output.replace(value).is_some() {
                    return Err("--out may be supplied only once".to_string());
                }
            }
            value if value.starts_with('-') => {
                return Err(format!("unknown compare-inventories option {value}"));
            }
            _ => positional.push(args[index].clone()),
        }
        index += 1;
    }
    if positional.len() != 2 {
        return Err("compare-inventories expects left.tsv and right.tsv".to_string());
    }

    let left_path = Path::new(&positional[0]);
    let right_path = Path::new(&positional[1]);
    let left = parse_inventory(left_path, tag.as_deref())?;
    let right = parse_inventory(right_path, tag.as_deref())?;
    let report = compare_inventories(left, right, tag);
    let wrote_stdout = if let Some(output) = output.as_deref() {
        write_report(output, &report, &[left_path, right_path])?
    } else {
        false
    };
    if !wrote_stdout {
        println!(
            "inventory diff: left={} right={} added={} removed={} changed={}",
            report.left.entries,
            report.right.entries,
            report.counts.added,
            report.counts.removed,
            report.counts.changed
        );
    }
    Ok(())
}

pub(crate) fn compare_unity_json_cli(args: &[String]) -> Result<(), String> {
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        println!(
            "compare-unity-json <left.json> <right.json> [--max <count>] [--out <report.json|->]"
        );
        return Ok(());
    }

    let mut positional = Vec::new();
    let mut max = 1_000usize;
    let mut output = None;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--max" | "--out" => {
                let option = args[index].as_str();
                index += 1;
                let value = args
                    .get(index)
                    .ok_or_else(|| format!("{option} requires a value"))?;
                if option == "--max" {
                    max = value
                        .parse::<usize>()
                        .ok()
                        .filter(|value| *value > 0)
                        .ok_or_else(|| "--max must be a positive integer".to_string())?;
                } else if output.replace(value.clone()).is_some() {
                    return Err("--out may be supplied only once".to_string());
                }
            }
            value if value.starts_with('-') => {
                return Err(format!("unknown compare-unity-json option {value}"));
            }
            _ => positional.push(args[index].clone()),
        }
        index += 1;
    }
    if positional.len() != 2 {
        return Err("compare-unity-json expects left.json and right.json".to_string());
    }

    let left_path = Path::new(&positional[0]);
    let right_path = Path::new(&positional[1]);
    let left = read_json(left_path)?;
    let right = read_json(right_path)?;
    let diff = structural_diff(&left.value, &right.value, max);
    let report = UnityJsonDiffReport {
        schema: UNITY_JSON_SCHEMA,
        warning: UNITY_JSON_WARNING,
        left: JsonInputSummary {
            path: left.display_path,
            sha256: left.sha256,
        },
        right: JsonInputSummary {
            path: right.display_path,
            sha256: right.sha256,
        },
        counts: JsonDiffCounts {
            changes: diff.total,
            emitted: diff.changes.len(),
        },
        truncated: diff.total > diff.changes.len(),
        changes: diff.changes,
    };
    let wrote_stdout = if let Some(output) = output.as_deref() {
        write_report(output, &report, &[left_path, right_path])?
    } else {
        false
    };
    if !wrote_stdout {
        println!(
            "Unity JSON diff: changes={} emitted={} truncated={}",
            report.counts.changes, report.counts.emitted, report.truncated
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests;
