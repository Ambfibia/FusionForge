//! Typed, in-process publishers. No shell, Python or PowerShell execution.
use base64::{Engine, engine::general_purpose::STANDARD};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Component, Path, PathBuf},
};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Recipe {
    schema: String,
    id: String,
    sources: Vec<Source>,
    steps: Vec<Step>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Source {
    alias: String,
    path: String,
    sha256: String,
    bytes: Option<u64>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "operation", rename_all = "kebab-case", deny_unknown_fields)]
enum Step {
    CorruptionProjectiles { source: usize },
    NativeJson {
        output: String,
        value: Value,
        #[serde(default, rename = "lineEnding")]
        line_ending: LineEnding,
    },
    Model {
        source: usize,
        #[serde(rename = "exactRoute")]
        exact_route: String,
        family: String,
        outputs: Vec<ModelOutput>,
    },
    Collision {
        mesh: String,
        collider: String,
        name: String,
        output: String,
        #[serde(rename = "reverseWinding")]
        reverse_winding: bool,
        sha256: String,
        contract: Option<String>,
    },
    JsonPatch {
        output: String,
        edits: Vec<super::native_json_patch::Edit>,
    },
    AudioPlan {
        source: usize,
        clips: Vec<Value>,
    },
    Evidence {
        source: usize,
        #[serde(rename = "serializedAsset")]
        asset: String,
        #[serde(rename = "pathId")]
        path_id: i64,
        #[serde(rename = "type")]
        type_name: String,
        name: String,
        #[serde(default)]
        assertions: Vec<Assertion>,
    },
    Texture {
        source: usize,
        #[serde(rename = "serializedAsset")]
        asset: String,
        #[serde(rename = "pathId")]
        path_id: i64,
        output: String,
        #[serde(default)]
        assertions: Vec<Assertion>,
        sha256: Option<String>,
        bytes: Option<u64>,
        #[serde(default)]
        additive: bool,
    },
    HnpcPalette {
        evidence: String,
        output: String,
    },
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "lowercase")]
enum LineEnding {
    #[default]
    Lf,
    Crlf,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ModelOutput {
    candidate: String,
    output: String,
    sha256: String,
    bytes: u64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Assertion {
    pointer: String,
    equals: Value,
}

/// Replay accepted one-time table edits with caller-owned native/server destinations.
pub(super) fn patch_files(
    recipe_path: &Path,
    source_root: &Path,
    destinations: &[(String, PathBuf)],
) -> Result<Vec<(String, Vec<u8>)>, String> {
    let recipe: Recipe = read_recipe(recipe_path)?;
    if recipe.schema != "fusionforge.native-cli-recipe.v1" {
        return Err("unsupported patch recipe".into());
    }
    for input in recipe.sources {
        check_bytes(
            &fs::read(relative(source_root, &input.path)?).map_err(|e| e.to_string())?,
            Some(&input.sha256),
            input.bytes,
        )?;
    }
    let mut outputs = Vec::new();
    for step in recipe.steps {
        let Step::JsonPatch { output, edits } = step else {
            return Err("table recipe must contain guarded JSON edits only".into());
        };
        let destination = destinations
            .iter()
            .find(|(route, _)| *route == output)
            .ok_or("undeclared table output")?
            .1
            .clone();
        let before = fs::read(&destination).map_err(|e| e.to_string())?;
        outputs.push((
            destination.to_string_lossy().into_owned(),
            super::native_json_patch::apply(&before, &edits)?,
        ));
    }
    Ok(outputs)
}

fn read_recipe(path: &Path) -> Result<Recipe, String> {
    let bytes = fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    serde_json::from_slice(bytes.strip_prefix(b"\xef\xbb\xbf").unwrap_or(&bytes))
        .map_err(|e| e.to_string())
}

// Native JSON constants remain readable recipe data. Remove the recipe's indentation
// without changing field order, numeric spelling or the selected line endings.
fn native_json_bytes(
    value: &serde_json::value::RawValue,
    line_ending: &LineEnding,
) -> Result<Vec<u8>, String> {
    let text = value.get();
    let lines = text.lines().collect::<Vec<_>>();
    let indentation = if lines.len() > 1 {
        lines
            .last()
            .unwrap()
            .chars()
            .take_while(|c| *c == ' ')
            .count()
    } else {
        0
    };
    let newline = match line_ending {
        LineEnding::Lf => "\n",
        LineEnding::Crlf => "\r\n",
    };
    let prefix = " ".repeat(indentation);
    let mut result = lines[0].to_string();
    for line in &lines[1..] {
        result.push_str(newline);
        result.push_str(
            line.strip_prefix(&prefix)
                .ok_or("inconsistent native JSON indentation")?,
        );
    }
    result.push_str(newline);
    let before: Value = serde_json::from_str(text).map_err(|e| e.to_string())?;
    let after: Value = serde_json::from_str(&result).map_err(|e| e.to_string())?;
    if before != after {
        return Err("native JSON formatting changed values".into());
    }
    Ok(result.into_bytes())
}

pub(super) fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

pub(super) fn read_json(path: &Path) -> Result<Value, String> {
    let bytes = fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    serde_json::from_slice(bytes.strip_prefix(b"\xef\xbb\xbf").unwrap_or(&bytes))
        .map_err(|e| format!("{}: {e}", path.display()))
}

pub(super) fn encode(value: &impl Serialize) -> Result<Vec<u8>, String> {
    let mut bytes = serde_json::to_vec_pretty(value).map_err(|e| e.to_string())?;
    bytes.push(b'\n');
    Ok(bytes)
}

pub(super) fn relative(root: &Path, route: &str) -> Result<PathBuf, String> {
    if route.is_empty()
        || route.contains('\\')
        || route.contains(':')
        || Path::new(route)
            .components()
            .any(|c| !matches!(c, Component::Normal(_)))
    {
        return Err(format!("not a safe relative path: {route:?}"));
    }
    let mut current = root.to_path_buf();
    for component in Path::new(route).components() {
        let name = component.as_os_str().to_string_lossy();
        let base = name.split('.').next().unwrap_or("").to_ascii_uppercase();
        if name.ends_with(['.', ' '])
            || name.chars().any(|c| c.is_control() || "<>|?*".contains(c))
            || matches!(base.as_str(), "CON" | "PRN" | "AUX" | "NUL")
            || (base.len() == 4
                && (base.starts_with("COM") || base.starts_with("LPT"))
                && matches!(base.as_bytes()[3], b'1'..=b'9'))
        {
            return Err(format!("ambiguous or reserved native path: {route:?}"));
        }
        current.push(component);
        if let Ok(metadata) = fs::symlink_metadata(&current) {
            if metadata.file_type().is_symlink() || reparse_point(&metadata) {
                return Err(format!(
                    "refusing linked publication path {}",
                    current.display()
                ));
            }
        }
    }
    Ok(current)
}

#[cfg(windows)]
fn reparse_point(metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    metadata.file_attributes() & 0x400 != 0
}
#[cfg(not(windows))]
fn reparse_point(_: &fs::Metadata) -> bool {
    false
}

fn check_assertions(value: &Value, assertions: &[Assertion]) -> Result<(), String> {
    for assertion in assertions {
        if value.pointer(&assertion.pointer) != Some(&assertion.equals) {
            return Err(format!(
                "evidence assertion failed at {}: expected {}, got {:?}",
                assertion.pointer,
                assertion.equals,
                value.pointer(&assertion.pointer)
            ));
        }
    }
    Ok(())
}

fn check_bytes(bytes: &[u8], sha: Option<&str>, length: Option<u64>) -> Result<(), String> {
    if length.is_some_and(|n| n != bytes.len() as u64)
        || sha.is_some_and(|sha| sha != digest(bytes))
    {
        return Err("payload differs from accepted size/hash".into());
    }
    Ok(())
}

fn check_additive(target: &Path, output: &str, bytes: &[u8]) -> Result<(), String> {
    match fs::read(relative(target, output)?) {
        Ok(old) if old != bytes => Err(format!(
            "additive publication refuses different existing artwork: {output}"
        )),
        Ok(_) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e.to_string()),
    }
}

pub(super) fn write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    fs::create_dir_all(path.parent().ok_or("output has no parent")?).map_err(|e| e.to_string())?;
    fs::write(path, bytes).map_err(|e| format!("{}: {e}", path.display()))
}

/// Verify the complete declaration before writing any target. Retain rollback copies in Editor.
#[cfg(test)]
fn install(
    target: &Path,
    stage: &Path,
    outputs: &[(String, Vec<u8>)],
    apply: bool,
) -> Result<Value, String> {
    install_checked(target, stage, outputs, apply, &BTreeMap::new())
}

pub(super) fn snapshot(
    root: &Path,
    routes: &[String],
) -> Result<BTreeMap<String, Option<String>>, String> {
    let mut result = BTreeMap::new();
    for route in routes {
        let before = match fs::read(relative(root, route)?) {
            Ok(bytes) => Some(digest(&bytes)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
            Err(e) => return Err(e.to_string()),
        };
        result.insert(route.clone(), before);
    }
    Ok(result)
}

pub(super) fn install_checked(
    target: &Path,
    stage: &Path,
    outputs: &[(String, Vec<u8>)],
    apply: bool,
    expected: &BTreeMap<String, Option<String>>,
) -> Result<Value, String> {
    install_with(target, stage, outputs, apply, expected, replace)
}

fn install_with(
    target: &Path,
    stage: &Path,
    outputs: &[(String, Vec<u8>)],
    apply: bool,
    expected: &BTreeMap<String, Option<String>>,
    mut commit: impl FnMut(&Path, &[u8]) -> Result<(), String>,
) -> Result<Value, String> {
    let target = target
        .canonicalize()
        .map_err(|e| format!("target root: {e}"))?;
    let mut names = BTreeSet::new();
    let mut plan = Vec::new();
    if snapshot(&target, &expected.keys().cloned().collect::<Vec<_>>())? != *expected {
        return Err("native inputs changed during conversion; nothing installed".into());
    }
    for (route, bytes) in outputs {
        if !names.insert(route.to_ascii_lowercase()) {
            return Err(format!("duplicate output {route}"));
        }
        let destination = relative(&target, route)?;
        let before = match fs::read(&destination) {
            Ok(bytes) => Some(bytes),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
            Err(e) => return Err(e.to_string()),
        };
        write(&relative(&stage.join("native"), route)?, bytes)?;
        if let Some(old) = &before {
            write(&relative(&stage.join("before"), route)?, old)?;
        }
        plan.push((route, bytes, destination, before));
    }
    let report = json!({"applied":apply,"outputs":plan.iter().map(|(path, bytes, _, before)| {
        json!({"path":path,"bytes":bytes.len(),"sha256":digest(bytes),
            "beforeSha256":before.as_ref().map(|b| digest(b)),"matchesTarget":before.as_ref() == Some(*bytes)})
    }).collect::<Vec<_>>()});
    if apply {
        // Validate all destinations again before beginning the transaction.
        for (route, _, destination, before) in &plan {
            relative(&target, route)?;
            let current = match fs::read(destination) {
                Ok(bytes) => Some(bytes),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
                Err(e) => return Err(e.to_string()),
            };
            if &current != before {
                return Err(format!("target changed during staging: {route}"));
            }
        }
        let mut installed = Vec::new();
        for (index, (_, bytes, destination, before)) in plan.iter().enumerate() {
            if before.as_ref() == Some(*bytes) {
                continue;
            }
            if let Err(error) = commit(destination, bytes) {
                let mut failures = Vec::new();
                for &previous in installed.iter().rev() {
                    let (_, _, destination, before): &(
                        &String,
                        &Vec<u8>,
                        PathBuf,
                        Option<Vec<u8>>,
                    ) = &plan[previous];
                    let result = match before {
                        Some(old) => replace(destination, old),
                        None => fs::remove_file(destination).map_err(|e| e.to_string()),
                    };
                    if let Err(e) = result {
                        failures.push(e);
                    }
                }
                return Err(format!(
                    "publication failed: {error}; rollback failures: {failures:?}; backups: {}",
                    stage.display()
                ));
            }
            installed.push(index);
        }
    }
    write(&stage.join("publication.json"), &encode(&report)?)?;
    Ok(report)
}

fn replace(path: &Path, bytes: &[u8]) -> Result<(), String> {
    use std::io::Write;
    fs::create_dir_all(path.parent().ok_or("destination has no parent")?)
        .map_err(|e| e.to_string())?;
    let mut next =
        tempfile::NamedTempFile::new_in(path.parent().unwrap()).map_err(|e| e.to_string())?;
    next.write_all(bytes).map_err(|e| e.to_string())?;
    next.as_file().sync_all().map_err(|e| e.to_string())?;
    next.persist(path).map_err(|e| e.to_string())?;
    Ok(())
}

pub(super) fn run(args: &[String]) -> Result<(), String> {
    let mut recipe_path = None;
    let mut source = None;
    let mut target = None;
    let mut mode = None;
    let mut replace_existing = false;
    let mut i = 0;
    while i < args.len() {
        let flag = &args[i];
        if flag == "--replace-existing" {
            replace_existing = true;
        } else if flag == "--apply" || flag == "--check" {
            if mode.replace(flag == "--apply").is_some() {
                return Err("choose --check or --apply once".into());
            }
        } else {
            i += 1;
            let value = args
                .get(i)
                .ok_or_else(|| format!("missing value for {flag}"))?;
            let slot = match flag.as_str() {
                "--recipe" => &mut recipe_path,
                "--source-root" => &mut source,
                "--target-root" => &mut target,
                _ => return Err(format!("unknown publication option {flag}")),
            };
            if slot.replace(PathBuf::from(value)).is_some() {
                return Err(format!("repeated option {flag}"));
            }
        }
        i += 1;
    }
    let recipe_path = recipe_path.ok_or("--recipe is required")?;
    let recipe_bytes = fs::read(&recipe_path).map_err(|e| e.to_string())?;
    let recipe_bytes = recipe_bytes
        .strip_prefix(b"\xef\xbb\xbf")
        .unwrap_or(&recipe_bytes);
    let recipe: Recipe = serde_json::from_slice(recipe_bytes).map_err(|e| e.to_string())?;
    #[derive(Deserialize)]
    struct RawRecipe {
        steps: Vec<Box<serde_json::value::RawValue>>,
    }
    let raw_recipe: RawRecipe = serde_json::from_slice(recipe_bytes).map_err(|e| e.to_string())?;
    if recipe.schema != "fusionforge.native-cli-recipe.v1" {
        return Err("unsupported native CLI recipe schema".into());
    }
    let source = source
        .or_else(|| {
            recipe
                .sources
                .is_empty()
                .then(|| crate::repository_root().to_path_buf())
        })
        .ok_or("--source-root is required")?
        .canonicalize()
        .map_err(|e| e.to_string())?;
    let target = target
        .ok_or("--target-root is required")?
        .canonicalize()
        .map_err(|e| e.to_string())?;
    if !recipe.sources.is_empty() && (target.starts_with(&source) || source.starts_with(&target)) {
        return Err("source and target must be separate".into());
    }
    let mut inputs = Vec::new();
    for input in &recipe.sources {
        let path = relative(&source, &input.path)?;
        let bytes = fs::read(&path).map_err(|e| e.to_string())?;
        check_bytes(&bytes, Some(&input.sha256), input.bytes)?;
        inputs.push(path);
    }
    // Preflight declarations before allocating a case or opening a Unity environment.
    let mut output_names = BTreeSet::new();
    let mut input_routes = Vec::new();
    for step in &recipe.steps {
        match step {
            Step::CorruptionProjectiles { .. } => {
                input_routes.extend(["effects/npc-skills/catalog.json".into(),
                    "effects/skill-hits/projectiles.json".into()]);
            }
            Step::Model { outputs, .. } => {
                input_routes.extend(outputs.iter().map(|o| o.output.clone()))
            }
            Step::Collision {
                contract: Some(route),
                ..
            } => {
                input_routes.push(route.clone());
                let doc = read_json(&relative(&target, route)?)?;
                input_routes.push(
                    doc["glb"]
                        .as_str()
                        .ok_or("collision visual path missing")?
                        .into(),
                );
            }
            Step::AudioPlan { clips, .. } => {
                input_routes.push("data/tables/table-set.json".into());
                for clip in clips {
                    let route = &clip["route"];
                    let path = route["path"].as_str().ok_or("audio path missing")?;
                    input_routes.push(if route["category"] == "voice" {
                        format!("audio/voice/en/{path}")
                    } else {
                        path.into()
                    });
                }
            }
            _ => {}
        }
        let route = match step {
            Step::Texture { output, .. }
            | Step::HnpcPalette { output, .. }
            | Step::JsonPatch { output, .. } => Some(output),
            Step::NativeJson { output, .. } => Some(output),
            Step::Collision { output, .. } => Some(output),
            _ => None,
        };
        if let Some(route) = route {
            input_routes.push(route.clone());
            relative(&target, route)?;
            if !output_names.insert(route.to_ascii_lowercase()) {
                return Err(format!("duplicate output {route}"));
            }
        }
    }
    let preimages = snapshot(&target, &input_routes)?;
    let mut outputs = Vec::new();
    let mut audio_routes = None;
    let mut evidence_by_name = BTreeMap::<String, Value>::new();
    let mut environments = BTreeMap::new();
    for (index, input) in inputs.iter().enumerate() {
        environments.insert(index, super::direct_input::load(input)?.0);
    }
    let evidence =
        |source_index: usize, asset: &str, path_id: i64, kind: &str| -> Result<Value, String> {
            let input = recipe
                .sources
                .get(source_index)
                .ok_or("invalid evidence source")?;
            let bytes = fs::read(&inputs[source_index]).map_err(|e| e.to_string())?;
            super::object_evidence::build_object_evidence(
                &environments[&source_index],
                super::object_evidence::ObjectEvidenceRequest {
                    source_alias: &input.alias,
                    relative_container: &input.path,
                    container_bytes: &bytes,
                    serialized_asset: Some(asset),
                    expected_type: Some(kind),
                    path_id,
                    allow_unresolved_pointers: false,
                },
            )
        };
    for (index, step) in recipe.steps.iter().enumerate() {
        match step {
            Step::CorruptionProjectiles { source } => {
                outputs.extend(super::native_corruption_effects::convert(
                    environments.get(source).ok_or("invalid corruption source")?, &target)?);
            }
            Step::NativeJson {
                output,
                value,
                line_ending,
            } => {
                #[derive(Deserialize)]
                struct Payload {
                    value: Box<serde_json::value::RawValue>,
                }
                let payload: Payload = serde_json::from_str(raw_recipe.steps[index].get())
                    .map_err(|e| e.to_string())?;
                let bytes = native_json_bytes(&payload.value, line_ending)?;
                if serde_json::from_slice::<Value>(&bytes).map_err(|e| e.to_string())? != *value {
                    return Err("native JSON payload differs".into());
                }
                outputs.push((output.clone(), bytes));
            }
            Step::Model {
                source: source_index,
                exact_route,
                family,
                outputs: selected,
            } => {
                let input = inputs.get(*source_index).ok_or("invalid model source")?;
                let model = crate::preview_bundle_container_model_exact(
                    input.display().to_string(),
                    None,
                    exact_route.clone(),
                )?;
                let options = ffone_asset_pipeline::LogicalModelPublishOptions::new(
                    "in-memory",
                    family,
                    &target,
                );
                let (_, files) =
                    ffone_asset_pipeline::prepare_direct_model(&options, &encode(&model)?)
                        .map_err(|e| e.to_string())?;
                for row in selected {
                    let bytes = files
                        .iter()
                        .find(|(path, _)| path == Path::new(&row.candidate))
                        .ok_or_else(|| format!("missing converted model output {}", row.candidate))?
                        .1
                        .clone();
                    check_bytes(&bytes, Some(&row.sha256), Some(row.bytes))?;
                    outputs.push((row.output.clone(), bytes));
                }
            }
            Step::Collision {
                mesh,
                collider,
                name,
                output,
                reverse_winding,
                sha256,
                contract,
            } => {
                let mesh = evidence_by_name
                    .get(mesh)
                    .ok_or("missing mesh evidence step")?;
                let collider = evidence_by_name
                    .get(collider)
                    .ok_or("missing collider evidence step")?;
                let (bytes, _report) =
                    super::native_collision::convert(&mesh, &collider, name, *reverse_winding)?;
                check_bytes(&bytes, Some(sha256), None)?;
                if let Some(route) = contract {
                    let path = relative(&target, route)?;
                    let original = fs::read(&path).map_err(|e| e.to_string())?;
                    let mut document = read_json(&path)?;
                    let visual = relative(
                        &target,
                        document["glb"]
                            .as_str()
                            .ok_or("missing collision visual route")?,
                    )?;
                    if document["glbBlake3"]
                        != blake3::hash(&fs::read(visual).map_err(|e| e.to_string())?)
                            .to_hex()
                            .to_string()
                        || document["colliderGlbBlake3"]
                            != blake3::hash(
                                &fs::read(relative(&target, output)?).map_err(|e| e.to_string())?,
                            )
                            .to_hex()
                            .to_string()
                    {
                        return Err("stale visual/collision binding".into());
                    }
                    let hash = blake3::hash(&bytes).to_hex().to_string();
                    if document["colliderGlbBlake3"] != hash
                        && document["colliderGlbBlake3"]
                            != "b22f3bf6e51f637f749c9ddd4bfff9eea149a401ba1d726f6a5af19bef78b31f"
                    {
                        return Err("unrecognized collision preimage".into());
                    }
                    document["colliderGlbBlake3"] = json!(hash);
                    outputs.push((
                        route.clone(),
                        super::native_json_patch::rewrite(&original, &document)?,
                    ));
                }
                outputs.push((output.clone(), bytes));
            }
            Step::JsonPatch { output, edits } => {
                let before = fs::read(relative(&target, output)?).map_err(|e| e.to_string())?;
                outputs.push((
                    output.clone(),
                    super::native_json_patch::apply(&before, edits)?,
                ));
            }
            Step::AudioPlan {
                source: source_index,
                clips,
            } => {
                let mut routes = match audio_routes.take() {
                    Some(routes) => routes,
                    None => read_json(&target.join("data/tables/table-set.json"))?,
                };
                let tables = routes["tables"]
                    .as_array_mut()
                    .ok_or("native tables missing")?;
                let matches = tables
                    .iter_mut()
                    .filter(|t| t["name"] == "native_asset_routes")
                    .collect::<Vec<_>>();
                if matches.len() != 1 {
                    return Err("expected one native_asset_routes table".into());
                }
                let table = matches.into_iter().next().unwrap();
                let rows = table["value"]["m_pAudioData"]
                    .as_array_mut()
                    .ok_or("audio routes missing")?;
                for clip in clips {
                    let asset = clip["serializedAsset"]
                        .as_str()
                        .ok_or("audio asset missing")?;
                    let pid = clip["pathId"].as_i64().ok_or("audio pathId missing")?;
                    let name = clip["trueName"].as_str().ok_or("audio name missing")?;
                    let evidence = evidence(*source_index, asset, pid, "AudioClip")?;
                    let value = &evidence["object"]["value"];
                    if value["m_Name"] != name {
                        return Err("audio name differs from scoped owner".into());
                    }
                    let data = value["audio data"]["base64"]
                        .as_str()
                        .ok_or("audio bytes missing")?;
                    let bytes = STANDARD.decode(data).map_err(|e| e.to_string())?;
                    if !bytes.starts_with(b"OggS") {
                        return Err("expected native Ogg payload".into());
                    }
                    check_bytes(
                        &bytes,
                        clip["outputSha256"].as_str(),
                        value["m_Size"].as_u64(),
                    )?;
                    let route = &clip["route"];
                    let path = route["path"].as_str().ok_or("audio native path missing")?;
                    let path = if route["category"] == "voice" {
                        format!("audio/voice/en/{path}")
                    } else {
                        path.into()
                    };
                    let destination = relative(&target, &path)?;
                    if destination.exists()
                        && fs::read(destination).map_err(|e| e.to_string())? != bytes
                    {
                        return Err(format!("refusing distinct audio at {path}"));
                    }
                    let key = route["logicalKey"].as_str().ok_or("audio key missing")?;
                    let existing = rows
                        .iter()
                        .filter(|r| r["logicalKey"] == key)
                        .collect::<Vec<_>>();
                    if existing.len() > 1 || existing.first().is_some_and(|old| *old != route) {
                        return Err(format!("conflicting audio route {key}"));
                    }
                    if existing.is_empty() {
                        rows.push(route.clone());
                    }
                    outputs.push((path, bytes));
                }
                rows.sort_by(|a, b| a["logicalKey"].as_str().cmp(&b["logicalKey"].as_str()));
                audio_routes = Some(routes);
            }
            Step::Evidence {
                source: source_index,
                asset,
                path_id,
                type_name,
                name,
                assertions,
            } => {
                let recovered = evidence(*source_index, asset, *path_id, type_name)?;
                check_assertions(&recovered, assertions)?;
                if evidence_by_name.insert(name.clone(), recovered).is_some() {
                    return Err("duplicate evidence name".into());
                }
            }
            Step::Texture {
                source: source_index,
                asset,
                path_id,
                output,
                assertions,
                sha256,
                bytes,
                additive,
            } => {
                let env = environments
                    .get(source_index)
                    .ok_or("invalid texture source")?;
                let matches = env
                    .assets
                    .iter()
                    .enumerate()
                    .filter(|(_, a)| a.name == *asset)
                    .collect::<Vec<_>>();
                let [(asset_index, _)] = matches.as_slice() else {
                    return Err("missing/ambiguous texture asset".into());
                };
                let texture = crate::logical_model_material::exact_texture(
                    env,
                    super::unity::ObjectKey {
                        asset: *asset_index,
                        path_id: *path_id,
                    },
                )?;
                if texture["id"] != format!("{asset}:{path_id}") {
                    return Err("texture ownership differs".into());
                }
                check_assertions(&texture, assertions)?;
                let payload = texture
                    .pointer("/payload/dataUrl")
                    .and_then(Value::as_str)
                    .ok_or("texture PNG payload missing")?;
                let payload = STANDARD
                    .decode(
                        payload
                            .strip_prefix("data:image/png;base64,")
                            .ok_or("expected PNG data URL")?,
                    )
                    .map_err(|e| e.to_string())?;
                check_bytes(&payload, sha256.as_deref(), *bytes)?;
                let recorded_sha = texture
                    .pointer("/payload/sha256")
                    .and_then(Value::as_str)
                    .ok_or("texture payload hash missing")?;
                check_bytes(&payload, Some(recorded_sha), None)?;
                if *additive {
                    check_additive(&target, output, &payload)?;
                }
                outputs.push((output.clone(), payload));
            }
            Step::HnpcPalette { evidence, output } => {
                let evidence = evidence_by_name
                    .get(evidence)
                    .ok_or("missing palette evidence step")?;
                let value = &evidence["object"]["value"];
                if value["m_Name"] != "male.bsd" {
                    return Err("unexpected HNPC palette owner".into());
                }
                #[derive(Serialize)]
                struct Palette {
                    schema: &'static str,
                    skin: Vec<Vec<Value>>,
                    hair: Vec<Vec<Value>>,
                }
                let mut result = Palette {
                    schema: "ffone.hnpc-palette.v1",
                    skin: Vec::new(),
                    hair: Vec::new(),
                };
                for (name, field) in [("skin", "cSkinColor"), ("hair", "cHairColor")] {
                    let colors = value[field][format!("{name}Color")]
                        .as_array()
                        .ok_or("palette missing")?;
                    if colors.len() != 20 {
                        return Err("HNPC palette must retain 20 colors".into());
                    }
                    let colors = colors
                        .iter()
                        .map(|color| {
                            ["r", "g", "b", "a"]
                                .iter()
                                .map(|channel| {
                                    color[*channel]
                                        .as_f64()
                                        .map(Value::from)
                                        .ok_or("palette channel missing".to_string())
                                })
                                .collect::<Result<Vec<_>, _>>()
                        })
                        .collect::<Result<Vec<_>, _>>()?;
                    if name == "skin" {
                        result.skin = colors;
                    } else {
                        result.hair = colors;
                    }
                }
                outputs.push((output.clone(), encode(&result)?));
            }
        }
    }
    if let Some(routes) = audio_routes {
        let original =
            fs::read(target.join("data/tables/table-set.json")).map_err(|e| e.to_string())?;
        outputs.push((
            "data/tables/table-set.json".into(),
            super::native_json_patch::rewrite(&original, &routes)?,
        ));
    }
    for (input, path) in recipe.sources.iter().zip(&inputs) {
        check_bytes(
            &fs::read(path).map_err(|e| e.to_string())?,
            Some(&input.sha256),
            input.bytes,
        )?;
    }
    // Conversion and source assertions have completed; existing files remain untouched on conflict.
    for (route, before) in &preimages {
        if fs::read(relative(&target, route)?).ok().map(|b| digest(&b)) != *before {
            return Err(format!("target changed during conversion: {route}"));
        }
    }
    let files = outputs
        .iter()
        .map(|(route, bytes)| (PathBuf::from(route), bytes.clone()))
        .collect::<Vec<_>>();
    if mode == Some(false) {
        if files
            .iter()
            .any(|(route, bytes)| fs::read(target.join(route)).ok().as_ref() != Some(bytes))
        {
            return Err("converted files differ from target; nothing written".into());
        }
    } else {
        ffone_asset_pipeline::direct_output::install_with_permission(
            &target,
            &files,
            replace_existing,
        )?;
    }
    println!(
        "{}: validated {} final files{}",
        recipe.id,
        files.len(),
        if mode == Some(false) {
            " (check only)"
        } else {
            ""
        }
    );
    Ok(())
}

#[cfg(test)]
mod tests;
