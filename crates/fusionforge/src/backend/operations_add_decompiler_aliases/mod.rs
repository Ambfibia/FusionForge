use super::super::*;

pub(in super::super) fn default_repo_root() -> PathBuf {
    crate::repository_root().to_path_buf()
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."))
}

pub(in super::super) fn stable_hash(value: &str) -> String {
    let mut hasher = DefaultHasher::new();
    value.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

pub(in super::super) fn stable_bytes_hash(value: &[u8]) -> String {
    let mut hasher = DefaultHasher::new();
    value.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

pub(in super::super) fn metadata_modified_ms(metadata: &fs::Metadata) -> Result<u128, String> {
    metadata
        .modified()
        .map_err(|err| err.to_string())?
        .duration_since(UNIX_EPOCH)
        .map_err(|err| err.to_string())
        .map(|duration| duration.as_millis())
}

pub(in super::super) fn json_string(value: &serde_json::Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

pub(in super::super) fn json_string_array(value: &serde_json::Value, key: &str) -> Option<Vec<String>> {
    let raw = value.get(key)?;
    let items = if let Some(items) = raw.as_array() {
        items
            .iter()
            .filter_map(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned)
            .collect::<Vec<_>>()
    } else if let Some(items) = raw.as_str() {
        items
            .split(',')
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned)
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };
    (!items.is_empty()).then_some(items)
}

pub(in super::super) fn json_bool(value: &serde_json::Value, key: &str, default: bool) -> bool {
    value
        .get(key)
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(default)
}

pub(in super::super) fn json_f64_string(value: &serde_json::Value, key: &str) -> Option<f64> {
    value.get(key).and_then(|value| {
        value
            .as_f64()
            .or_else(|| value.as_str().and_then(|text| text.parse::<f64>().ok()))
    })
}

pub(in super::super) fn safe_file_stem(value: &str) -> String {
    let mut output = value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '.' | '-' | '_') {
                ch
            } else {
                '_'
            }
        })
        .collect::<String>();
    if output.is_empty() {
        output = "assembly".to_string();
    }
    output
}

pub(in super::super) fn list_script_assemblies(root: String) -> EditorResult<Vec<ScriptAssemblyRecord>> {
    let root = PathBuf::from(root);
    if !root.is_dir() {
        return Err(format!("{} is not a directory", root.display()));
    }
    let mut records = Vec::new();
    scan_script_assemblies(&root, &mut records)?;
    records.sort_by_key(|record| record.name.to_ascii_lowercase());
    Ok(records)
}

pub(in super::super) fn scan_script_assemblies(
    root: &Path,
    records: &mut Vec<ScriptAssemblyRecord>,
) -> EditorResult<()> {
    for entry in fs::read_dir(root).map_err(|err| format!("{}: {err}", root.display()))? {
        let entry = entry.map_err(|err| err.to_string())?;
        let path = entry.path();
        if path.is_dir() {
            scan_script_assemblies(&path, records)?;
            continue;
        }
        if path.extension().and_then(|value| value.to_str()) != Some("dll") {
            continue;
        }
        let metadata = path
            .metadata()
            .map_err(|err| format!("{}: {err}", path.display()))?;
        records.push(ScriptAssemblyRecord {
            path: path.to_string_lossy().to_string(),
            name: path
                .file_name()
                .and_then(|value| value.to_str())
                .unwrap_or("assembly.dll")
                .to_string(),
            size: metadata.len(),
        });
    }
    Ok(())
}

pub(in super::super) fn decompile_script_assembly(
    project_dir: String,
    backend_project: String,
    assembly_path: String,
) -> EditorResult<ScriptDecompileResult> {
    let project = PathBuf::from(project_dir);
    if !project.is_dir() {
        return Err(format!("{} is not a project directory", project.display()));
    }
    let backend = PathBuf::from(backend_project.trim());
    if !backend.is_file() {
        return Err(format!(
            "FFSpy backend project was not found: {}",
            backend.display()
        ));
    }
    let assembly = PathBuf::from(assembly_path.trim());
    if !assembly.is_file() {
        return Err(format!("Assembly was not found: {}", assembly.display()));
    }

    let assembly_name = assembly
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("assembly");
    let output_dir = project.join("scripts").join("decompiled").join(format!(
        "{}_{}",
        safe_file_stem(assembly_name),
        path_hash(&assembly)
    ));
    fs::create_dir_all(&output_dir).map_err(|err| format!("{}: {err}", output_dir.display()))?;
    stage_script_reference_dlls(&project, assembly.parent())?;

    let mut command = Command::new("dotnet");
    command
        .arg("run")
        .arg("--project")
        .arg(&backend)
        .arg("--framework")
        .arg("net6.0")
        .arg("--")
        .arg(&assembly)
        .arg("-o")
        .arg(&output_dir)
        .arg("-p")
        .arg("--preserve-iterator-state-machines");
    if let Some(parent) = assembly.parent() {
        command.arg("-r").arg(parent);
    }

    let output = command
        .output()
        .map_err(|err| format!("Failed to start FFSpy backend: {err}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        return Err(format!(
            "FFSpy backend failed with status {}.{}{}",
            output.status,
            if stderr.trim().is_empty() {
                String::new()
            } else {
                format!(" stderr: {}", stderr.trim())
            },
            if stdout.trim().is_empty() {
                String::new()
            } else {
                format!(" stdout: {}", stdout.trim())
            }
        ));
    }

    Ok(ScriptDecompileResult {
        assembly: assembly.to_string_lossy().to_string(),
        output_dir: output_dir.to_string_lossy().to_string(),
        backend_project: backend.to_string_lossy().to_string(),
        status: String::from_utf8_lossy(&output.stdout).trim().to_string(),
    })
}

pub(in super::super) fn default_script_reference_roots(project: &Path, assembly_parent: Option<&Path>) -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if let Some(parent) = assembly_parent {
        roots.push(parent.to_path_buf());
    }
    if let Some(root) = workspace_root_dir() {
        roots.push(
            root.join("OpenFusionLauncherPortable")
                .join("mono")
                .join("fusion-2.x.x")
                .join("Data")
                .join("lib"),
        );
        roots.push(
            root.join("OpenFusionLauncherPortable")
                .join("player")
                .join("fusion-2.x.x")
                .join("Data")
                .join("lib"),
        );
        roots.push(root.join("OpenFusionLauncherPortable"));
    }
    roots.push(project.join("scripts").join("references"));
    roots
}

pub(in super::super) fn stage_script_reference_dlls(
    project: &Path,
    assembly_parent: Option<&Path>,
) -> EditorResult<PathBuf> {
    let references_dir = project.join("scripts").join("references");
    fs::create_dir_all(&references_dir)
        .map_err(|err| format!("{}: {err}", references_dir.display()))?;
    let mut copied = BTreeSet::<String>::new();
    for root in default_script_reference_roots(project, assembly_parent) {
        if !root.is_dir() {
            continue;
        }
        for entry in fs::read_dir(&root).map_err(|err| format!("{}: {err}", root.display()))? {
            let entry = entry.map_err(|err| err.to_string())?;
            let path = entry.path();
            if path.extension().and_then(|value| value.to_str()) != Some("dll") {
                continue;
            }
            let Some(name) = path.file_name().and_then(|value| value.to_str()) else {
                continue;
            };
            if !copied.insert(name.to_ascii_lowercase()) {
                continue;
            }
            let target = references_dir.join(name);
            if path == target {
                continue;
            }
            fs::copy(&path, &target)
                .map_err(|err| format!("{} -> {}: {err}", path.display(), target.display()))?;
        }
    }
    Ok(references_dir)
}

pub(in super::super) fn sanitize_decompiled_sources(root: &Path) -> EditorResult<usize> {
    let mut changed = 0usize;
    sanitize_decompiled_sources_inner(root, &mut changed)?;
    Ok(changed)
}

pub(in super::super) fn sanitize_decompiled_sources_inner(root: &Path, changed: &mut usize) -> EditorResult<()> {
    for entry in fs::read_dir(root).map_err(|err| format!("{}: {err}", root.display()))? {
        let entry = entry.map_err(|err| err.to_string())?;
        let path = entry.path();
        if path.is_dir() {
            sanitize_decompiled_sources_inner(&path, changed)?;
            continue;
        }
        if path.extension().and_then(|value| value.to_str()) != Some("cs") {
            continue;
        }
        let data = fs::read_to_string(&path).map_err(|err| format!("{}: {err}", path.display()))?;
        let mut fixed = apply_known_decompiler_patches(&data, &path);
        fixed = add_decompiler_aliases(&fixed);
        if fixed != data {
            fs::write(&path, fixed).map_err(|err| format!("{}: {err}", path.display()))?;
            *changed += 1;
        }
    }
    Ok(())
}

pub(in super::super) fn add_decompiler_aliases(data: &str) -> String {
    let mut aliases = Vec::new();
    if data.contains("using System;")
        && data.contains("using UnityEngine;")
        && !data.contains("using Object = UnityEngine.Object;")
        && contains_token(data, "Object")
    {
        aliases.push("using Object = UnityEngine.Object;");
    }
    if data.contains("using System;")
        && data.contains("using UnityEngine;")
        && !data.contains("using Random = UnityEngine.Random;")
        && contains_token(data, "Random")
    {
        aliases.push("using Random = UnityEngine.Random;");
    }
    if !data.contains("public class ConfigurableInput")
        && !data.contains("using Key = ConfigurableInput.Key;")
        && (contains_token(data, "Key") || contains_token(data, "Axis"))
    {
        aliases.push("using Key = ConfigurableInput.Key;");
        aliases.push("using Axis = ConfigurableInput.Axis;");
    }
    if !data.contains("public class cnAvatarStatus")
        && !data.contains("using MoveType = cnAvatarStatus.MoveType;")
        && contains_token(data, "MoveType")
    {
        aliases.push("using MoveType = cnAvatarStatus.MoveType;");
    }
    if !data.contains("public class MusicDataStorage")
        && !data.contains("using SoundData = MusicDataStorage.SoundData;")
        && contains_token(data, "SoundData")
    {
        aliases.push("using SoundData = MusicDataStorage.SoundData;");
    }
    if !data.contains("public class AssetLoader")
        && !data.contains("using Mode = AssetLoader.Mode;")
        && contains_token(data, "Mode")
    {
        aliases.push("using Mode = AssetLoader.Mode;");
    }
    if !contains_public_class(data, "AssetLoader")
        && !data.contains("using ResultHolder = AssetLoader.ResultHolder;")
        && contains_token(data, "ResultHolder")
    {
        aliases.push("using ResultHolder = AssetLoader.ResultHolder;");
    }
    if !data.contains("public class EpTrigger")
        && !data.contains("using TriggerType = EpTrigger.TriggerType;")
        && contains_token(data, "TriggerType")
    {
        aliases.push("using TriggerType = EpTrigger.TriggerType;");
    }
    if !data.contains("public class AnimationEventHandler")
        && !data.contains("using delEndAnimation = AnimationEventHandler.delEndAnimation;")
        && contains_token(data, "delEndAnimation")
    {
        aliases.push("using delEndAnimation = AnimationEventHandler.delEndAnimation;");
    }
    if !data.contains("public class AnimationEventHandler")
        && !data
            .contains("using CheckCurrentAnimation = AnimationEventHandler.CheckCurrentAnimation;")
        && contains_token(data, "CheckCurrentAnimation")
    {
        aliases.push("using CheckCurrentAnimation = AnimationEventHandler.CheckCurrentAnimation;");
    }
    if !data.contains("public class ActorSkinCombiner")
        && !data.contains("using AttachName = ActorSkinCombiner.AttachName;")
        && contains_token(data, "AttachName")
    {
        aliases.push("using AttachName = ActorSkinCombiner.AttachName;");
    }
    if !contains_public_class(data, "cnOption")
        && !data.contains("using DetailLevel = cnOption.DetailLevel;")
        && contains_token(data, "DetailLevel")
    {
        aliases.push("using DetailLevel = cnOption.DetailLevel;");
    }
    if !contains_public_class(data, "cnOption")
        && !data.contains("using ShadowDetail = cnOption.ShadowDetail;")
        && contains_token(data, "ShadowDetail")
    {
        aliases.push("using ShadowDetail = cnOption.ShadowDetail;");
    }
    if !contains_public_class(data, "cnOption")
        && !data.contains("using TextureQuality = cnOption.TextureQuality;")
        && contains_token(data, "TextureQuality")
    {
        aliases.push("using TextureQuality = cnOption.TextureQuality;");
    }
    if data.contains("using UnityEngine;")
        && !data.contains("using WindowFunction = UnityEngine.GUI.WindowFunction;")
        && contains_token(data, "WindowFunction")
    {
        aliases.push("using WindowFunction = UnityEngine.GUI.WindowFunction;");
    }
    if !data.contains("using CheckOverHit = ActorShadowCaster.CheckOverHit;")
        && contains_token(data, "CheckOverHit")
    {
        aliases.push("using CheckOverHit = ActorShadowCaster.CheckOverHit;");
    }
    if !data.contains("using SoundDataOverrideFunc = MusicController.SoundDataOverrideFunc;")
        && contains_token(data, "SoundDataOverrideFunc")
    {
        aliases.push("using SoundDataOverrideFunc = MusicController.SoundDataOverrideFunc;");
    }
    if !data.contains("using SoundDataDelayOverride = MusicController.SoundDataDelayOverride;")
        && contains_token(data, "SoundDataDelayOverride")
    {
        aliases.push("using SoundDataDelayOverride = MusicController.SoundDataDelayOverride;");
    }
    if !data.contains("using EffectKey = EffectEmitterController.EffectKey;")
        && contains_token(data, "EffectKey")
    {
        aliases.push("using EffectKey = EffectEmitterController.EffectKey;");
    }
    if !data.contains("using EmitterType = ParticleEmitterController.EmitterType;")
        && contains_token(data, "EmitterType")
    {
        aliases.push("using EmitterType = ParticleEmitterController.EmitterType;");
    }
    if aliases.is_empty() {
        return data.to_string();
    }
    let mut insert_at = 0usize;
    for line in data.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("using ") {
            insert_at += line.len() + 2;
        } else {
            break;
        }
    }
    let mut output = String::new();
    output.push_str(&data[..insert_at.min(data.len())]);
    for alias in aliases {
        output.push_str(alias);
        output.push_str("\r\n");
    }
    output.push_str(&data[insert_at.min(data.len())..]);
    output
}

pub(in super::super) fn rewrite_csharp_accessor_calls(data: &str) -> String {
    let with_indexers = rewrite_csharp_indexer_getter_calls(data);
    let with_getters = rewrite_csharp_getter_calls(&with_indexers);
    rewrite_csharp_setter_calls(&with_getters)
}

pub(in super::super) fn rewrite_known_out_argument_calls(data: &str) -> String {
    let mut output = String::with_capacity(data.len());
    for line in data.split_inclusive('\n') {
        let trimmed = line.trim_start();
        if trimmed.contains(".CalcMinMaxWidth(")
            || trimmed.contains("DongLoader.GetPositionColor(")
            || trimmed.contains("Physics.Raycast(")
        {
            output.push_str(&line.replace(", ref ", ", out "));
        } else {
            output.push_str(line);
        }
    }
    output
}

pub(in super::super) fn repair_empty_gui_color_temporaries(data: &str) -> String {
    let mut output = String::with_capacity(data.len());
    for line in data.split_inclusive('\n') {
        let (body, newline) = line
            .strip_suffix("\r\n")
            .map(|body| (body, "\r\n"))
            .or_else(|| line.strip_suffix('\n').map(|body| (body, "\n")))
            .unwrap_or((line, ""));
        let indent_len = body.len() - body.trim_start().len();
        let trimmed = &body[indent_len..];
        if trimmed.starts_with("Color color") && trimmed.ends_with(" = ;") {
            output.push_str(&body[..indent_len]);
            output.push_str(trimmed.trim_end_matches(" = ;"));
            output.push_str(" = GUI.color;");
            output.push_str(newline);
        } else if trimmed == "color = ;" {
            output.push_str(&body[..indent_len]);
            output.push_str("color = GUI.color;");
            output.push_str(newline);
        } else {
            output.push_str(line);
        }
    }
    output
}

pub(in super::super) fn rewrite_unknown_isinst_placeholders(data: &str) -> String {
    let mut fixed = data.to_string();
    fixed = fixed.replace(
        "(Object)/*isinst with value type is only supported in some contexts*/",
        "(Object)null",
    );
    fixed = fixed.replace(
        "(MusicController)/*isinst with value type is only supported in some contexts*/",
        "(MusicController)((Component)hostBehaviour).GetComponent(typeof(MusicController))",
    );
    fixed = fixed.replace(
        "(Texture2D)/*isinst with value type is only supported in some contexts*/",
        "(Texture2D)null",
    );
    fixed
}

pub(in super::super) fn remove_standalone_csharp_expression_statement(data: &str, expression: &str) -> String {
    let mut output = String::with_capacity(data.len());
    for line in data.split_inclusive('\n') {
        let (body, newline) = line
            .strip_suffix("\r\n")
            .map(|body| (body, "\r\n"))
            .or_else(|| line.strip_suffix('\n').map(|body| (body, "\n")))
            .unwrap_or((line, ""));
        if body.trim() == format!("{expression};") {
            output.push_str(&body[..body.len() - body.trim_start().len()]);
            output.push_str(";\n".strip_suffix('\n').unwrap_or(";"));
            output.push_str(newline);
        } else {
            output.push_str(line);
        }
    }
    output
}

pub(in super::super) fn remove_known_csharp_noop_statements(data: &str) -> String {
    let mut output = String::with_capacity(data.len());
    for line in data.split_inclusive('\n') {
        let (body, newline) = line
            .strip_suffix("\r\n")
            .map(|body| (body, "\r\n"))
            .or_else(|| line.strip_suffix('\n').map(|body| (body, "\n")))
            .unwrap_or((line, ""));
        let trimmed = body.trim();
        let is_noop = trimmed == "(TrackedReference)(object)upperAniState;"
            || (trimmed.ends_with(".material;")
                && !trimmed.contains('=')
                && !trimmed.starts_with("Material "));
        if is_noop {
            output.push_str(&body[..body.len() - body.trim_start().len()]);
            output.push(';');
            output.push_str(newline);
        } else {
            output.push_str(line);
        }
    }
    output
}

pub(in super::super) fn dedupe_csharp_using_aliases(data: &str) -> String {
    let mut seen = HashSet::new();
    let mut output = String::with_capacity(data.len());
    for line in data.split_inclusive('\n') {
        let trimmed = line.trim();
        if trimmed.starts_with("using ")
            && trimmed.contains(" = ")
            && !seen.insert(trimmed.to_string())
        {
            continue;
        }
        output.push_str(line);
    }
    output
}

pub(in super::super) fn rewrite_value_type_constructor_calls(data: &str) -> String {
    let mut output = String::with_capacity(data.len());
    for line in data.split_inclusive('\n') {
        let (body, newline) = line
            .strip_suffix("\r\n")
            .map(|body| (body, "\r\n"))
            .or_else(|| line.strip_suffix('\n').map(|body| (body, "\n")))
            .unwrap_or((line, ""));
        let indent_len = body.len() - body.trim_start().len();
        let indent = &body[..indent_len];
        let trimmed = &body[indent_len..];
        let mut rewritten = None;
        for ty in [
            "Vector2",
            "Vector3",
            "Vector4",
            "Rect",
            "Quaternion",
            "Color",
            "Matrix4x4",
            "Resolution",
            "RaycastHit",
        ] {
            let prefix = format!("(({ty})(");
            if let Some(rest) = trimmed.strip_prefix(&prefix) {
                let Some(target_end) = rest.find("))._002Ector(") else {
                    continue;
                };
                let target = &rest[..target_end];
                if target.is_empty() {
                    continue;
                }
                let args_start = target_end + "))._002Ector(".len();
                let Some(args_end) = rest[args_start..].rfind(");") else {
                    continue;
                };
                let args = &rest[args_start..args_start + args_end];
                rewritten = Some(format!("{indent}{target} = new {ty}({args});"));
                break;
            }
        }
        output.push_str(rewritten.as_deref().unwrap_or(body));
        output.push_str(newline);
    }
    output
}

pub(in super::super) fn strip_redundant_value_type_casts(data: &str) -> String {
    let mut fixed = data.to_string();
    for ty in [
        "Vector2",
        "Vector3",
        "Vector4",
        "Rect",
        "Quaternion",
        "Color",
        "Matrix4x4",
        "Resolution",
        "RaycastHit",
    ] {
        fixed = strip_redundant_cast_to_type(&fixed, ty);
    }
    fixed
}

pub(in super::super) fn strip_redundant_cast_to_type(data: &str, ty: &str) -> String {
    let mut output = String::with_capacity(data.len());
    let mut cursor = 0usize;
    let needle = format!("(({ty})(");
    while let Some(relative) = data[cursor..].find(&needle) {
        let start = cursor + relative;
        let inner_open = start + needle.len() - 1;
        let Some(inner_close) = find_matching_paren(data, inner_open) else {
            break;
        };
        if !data[inner_close + 1..].starts_with(')') {
            output.push_str(&data[cursor..inner_close + 1]);
            cursor = inner_close + 1;
            continue;
        }
        output.push_str(&data[cursor..start]);
        output.push_str(&data[inner_open + 1..inner_close]);
        cursor = inner_close + 2;
    }
    output.push_str(&data[cursor..]);
    output
}

pub(in super::super) fn rewrite_csharp_indexer_getter_calls(data: &str) -> String {
    let mut output = String::with_capacity(data.len());
    let mut cursor = 0usize;
    let needle = ".get_Item(";
    while let Some(relative) = data[cursor..].find(needle) {
        let index = cursor + relative;
        let paren = index + ".get_Item".len();
        let Some(close) = find_matching_paren(data, paren) else {
            break;
        };
        output.push_str(&data[cursor..index]);
        output.push('[');
        output.push_str(&data[paren + 1..close]);
        output.push(']');
        cursor = close + 1;
    }
    output.push_str(&data[cursor..]);
    output
}

pub(in super::super) fn rewrite_csharp_getter_calls(data: &str) -> String {
    let mut output = String::with_capacity(data.len());
    let mut rest = data;
    while let Some(index) = rest.find(".get_") {
        output.push_str(&rest[..index]);
        let name_start = index + ".get_".len();
        let Some(paren_offset) = rest[name_start..].find("()") else {
            output.push_str(&rest[index..]);
            return output;
        };
        let name_end = name_start + paren_offset;
        let property = &rest[name_start..name_end];
        if property
            .chars()
            .all(|ch| ch == '_' || ch.is_ascii_alphanumeric())
        {
            output.push('.');
            output.push_str(property);
            rest = &rest[name_end + 2..];
        } else {
            output.push_str(&rest[index..name_end + 2]);
            rest = &rest[name_end + 2..];
        }
    }
    output.push_str(rest);
    output
}

pub(in super::super) fn rewrite_csharp_setter_calls(data: &str) -> String {
    let mut output = String::with_capacity(data.len());
    let mut cursor = 0usize;
    while let Some(relative) = data[cursor..].find(".set_") {
        let index = cursor + relative;
        let name_start = index + ".set_".len();
        let Some(paren_relative) = data[name_start..].find('(') else {
            break;
        };
        let paren = name_start + paren_relative;
        let property = &data[name_start..paren];
        if property.is_empty()
            || !property
                .chars()
                .all(|ch| ch == '_' || ch.is_ascii_alphanumeric())
        {
            output.push_str(&data[cursor..paren + 1]);
            cursor = paren + 1;
            continue;
        }
        let Some(close) = find_matching_paren(data, paren) else {
            break;
        };
        let argument = &data[paren + 1..close];
        output.push_str(&data[cursor..index]);
        if property == "Item" {
            if let Some((item_index, item_value)) = split_top_level_comma(argument) {
                output.push('[');
                output.push_str(item_index.trim());
                output.push_str("] = ");
                output.push_str(item_value.trim());
            } else {
                output.push_str(".set_Item(");
                output.push_str(argument);
                output.push(')');
            }
        } else {
            output.push('.');
            output.push_str(property);
            output.push_str(" = ");
            output.push_str(argument);
        }
        cursor = close + 1;
    }
    output.push_str(&data[cursor..]);
    output
}
