//! Editor-only index of installed immutable texture contracts. Hashes locate
//! candidates; every mip byte and the complete sampling contract prove reuse.
use super::*;
use std::path::Component;

const SCHEMA: &str = "ffone.native-texture-reuse-index.v1";

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Index {
    schema: String,
    root: PathBuf,
    entries: Vec<Entry>,
    #[serde(default)]
    excluded: Vec<String>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Entry {
    owner: PathBuf,
    owner_sha256: String,
    binding: MaterialTextureBinding,
}

pub(super) fn normalize_relative(path: &Path) -> Result<PathBuf> {
    let mut result = PathBuf::new();
    for part in path.components() {
        match part {
            Component::Normal(name) => result.push(name),
            Component::CurDir => (),
            Component::ParentDir if result.pop() => (),
            _ => {
                return invalid(format!(
                    "native reference escapes asset root: {}",
                    path.display()
                ));
            }
        }
    }
    Ok(result)
}

fn contained_file(root: &Path, relative: &Path) -> Result<PathBuf> {
    let path = root.join(normalize_relative(relative)?);
    let actual = path.canonicalize().map_err(|e| io_at(&path, e))?;
    if !actual.starts_with(root) {
        return invalid(format!(
            "native reference resolves outside asset root: {}",
            path.display()
        ));
    }
    Ok(actual)
}

fn glb_json(bytes: &[u8]) -> Result<Value> {
    if bytes.len() < 20 || &bytes[..4] != b"glTF" || &bytes[16..20] != b"JSON" {
        return invalid("invalid native GLB header");
    }
    let length = u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize;
    let json = bytes
        .get(20..20 + length)
        .ok_or_else(|| invalid_error("truncated GLB JSON"))?;
    serde_json::from_slice(json).map_err(|e| invalid_error(e.to_string()))
}

/// Scan once on explicit request. The index is disposable Editor metadata and
/// must not be installed with native content. Publication revalidates matches.
pub fn index_native_textures(root: &Path, output: &Path) -> Result<usize> {
    let root = root.canonicalize().map_err(|e| io_at(root, e))?;
    let mut pending = vec![root.clone()];
    let mut glbs = Vec::new();
    while let Some(dir) = pending.pop() {
        for entry in fs::read_dir(&dir).map_err(|e| io_at(&dir, e))? {
            let entry = entry.map_err(|e| io_at(&dir, e))?;
            let kind = entry.file_type().map_err(|e| io_at(entry.path(), e))?;
            // Never follow links out of the native tree or introduce cycles.
            if kind.is_dir() && !kind.is_symlink() {
                pending.push(entry.path());
            }
            if kind.is_file() && entry.path().extension().is_some_and(|x| x == "glb") {
                glbs.push(entry.path());
            }
        }
    }
    glbs.sort();
    let mut entries = Vec::new();
    let mut excluded = Vec::new();
    let mut seen = BTreeSet::new();
    for path in glbs {
        let bytes = fs::read(&path).map_err(|e| io_at(&path, e))?;
        let doc = glb_json(&bytes)?;
        let owner = path.strip_prefix(&root).unwrap().to_path_buf();
        for material in doc
            .get("materials")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            let Some(native) = material.pointer("/extras/ffone") else {
                continue;
            };
            let material: NativeMaterial = serde_json::from_value(native.clone())
                .map_err(|e| invalid_error(format!("{}: {e}", path.display())))?;
            'binding: for binding in material.texture_bindings {
                if binding.dynamic_texture.is_some()
                    || binding.texture.is_none()
                    || binding.mip_levels.as_ref().is_none_or(Vec::is_empty)
                {
                    continue;
                }
                let levels = binding.mip_levels.as_ref().unwrap();
                let mut identity = Vec::new();
                for level in levels {
                    let rel = normalize_relative(&owner.parent().unwrap().join(&level.uri))?;
                    if let Err(error) = contained_file(&root, &rel) {
                        excluded.push(format!("{}: {error}", owner.display()));
                        continue 'binding;
                    }
                    identity.push(slash_path(&rel));
                }
                let key = format!(
                    "{:?}|{:?}|{:?}",
                    identity, binding.sampler, binding.color_space
                );
                if seen.insert(key) {
                    entries.push(Entry {
                        owner: owner.clone(),
                        owner_sha256: sha256_hex(&bytes),
                        binding,
                    });
                }
            }
        }
    }
    let count = entries.len();
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent).map_err(|e| io_at(parent, e))?;
    }
    if !excluded.is_empty() {
        eprintln!(
            "Excluded {} unresolved native texture bindings; details are recorded in the index",
            excluded.len()
        );
    }
    let bytes = serde_json::to_vec(&Index {
        schema: SCHEMA.into(),
        root,
        entries,
        excluded,
    })
    .map_err(|e| invalid_error(e.to_string()))?;
    fs::write(output, bytes).map_err(|e| io_at(output, e))?;
    Ok(count)
}

fn same_contract(left: &MaterialTextureBinding, right: &MaterialTextureBinding) -> bool {
    let (Some(ls), Some(rs), Some(ll), Some(rl)) = (
        &left.sampler,
        &right.sampler,
        &left.mip_levels,
        &right.mip_levels,
    ) else {
        return false;
    };
    let mut ls = ls.descriptor.clone();
    let mut rs = rs.descriptor.clone();
    ls.name.clear();
    rs.name.clear();
    // Provenance checks are deliberately conservative. Never infer equality
    // from equal base images or discard unknown future native fields.
    if ls != rs
        || left.color_space != right.color_space
        || left.mip_provenance != right.mip_provenance
        || ll.len() != rl.len()
    {
        return false;
    }
    ll.iter().zip(rl).all(|(l, r)| {
        let mut l = l.clone();
        let mut r = r.clone();
        l.uri.clear();
        r.uri.clear();
        l == r
    })
}

fn relative_uri(owner: &Path, target: &Path) -> String {
    let from: Vec<_> = owner.parent().unwrap().components().collect();
    let to: Vec<_> = target.components().collect();
    let common = from.iter().zip(&to).take_while(|(a, b)| a == b).count();
    let mut result = PathBuf::new();
    for _ in common..from.len() {
        result.push("..");
    }
    for part in &to[common..] {
        result.push(part.as_os_str());
    }
    slash_path(&result)
}

pub(super) fn reuse_textures(
    index_path: &Path,
    output_glb: &Path,
    converted: &mut ConvertedModel,
) -> Result<()> {
    let bytes = fs::read(index_path).map_err(|e| io_at(index_path, e))?;
    let index: Index = serde_json::from_slice(&bytes).map_err(|e| invalid_error(e.to_string()))?;
    if index.schema != SCHEMA {
        return invalid("unsupported native texture reuse index");
    }
    let root = index
        .root
        .canonicalize()
        .map_err(|e| io_at(&index.root, e))?;
    let mut verified_owners = BTreeSet::new();
    let mut redirects = BTreeMap::new();
    let mut used_destinations = BTreeSet::new();
    for texture_index in 0..converted.model.textures.len() {
        let bindings: Vec<_> = converted
            .model
            .materials
            .iter()
            .flat_map(|m| &m.texture_bindings)
            .filter(|b| b.texture == Some(texture_index as u32))
            .collect();
        let Some(binding) = bindings.first() else {
            continue;
        };
        if bindings.iter().any(|b| !same_contract(binding, b)) {
            continue;
        }
        for candidate in &index.entries {
            if !same_contract(binding, &candidate.binding) {
                continue;
            }
            let candidate_uri = candidate.binding.uri.as_deref().unwrap_or("");
            if binding.source_name != candidate.binding.source_name
                && !candidate_uri.split('/').any(|s| s == "textures")
            {
                continue;
            }

            let owner = contained_file(&root, &candidate.owner)?;
            if !verified_owners.contains(&candidate.owner) {
                let bytes = fs::read(&owner).map_err(|e| io_at(&owner, e))?;
                if sha256_hex(&bytes) != candidate.owner_sha256 {
                    return invalid(format!(
                        "stale native texture index: {}; rebuild the index",
                        owner.display()
                    ));
                }
                verified_owners.insert(candidate.owner.clone());
            }
            let mut proposed = Vec::new();
            for (source, target) in binding
                .mip_levels
                .as_ref()
                .unwrap()
                .iter()
                .zip(candidate.binding.mip_levels.as_ref().unwrap())
            {
                let rel = normalize_relative(&candidate.owner.parent().unwrap().join(&target.uri))?;
                let path = contained_file(&root, &rel)?;
                let bytes = fs::read(&path).map_err(|e| io_at(&path, e))?;
                let generated = converted
                    .texture_files
                    .iter()
                    .find(|f| f.uri == source.uri)
                    .ok_or_else(|| invalid_error("missing generated texture level"))?;
                if bytes != generated.bytes {
                    proposed.clear();
                    break;
                }
                proposed.push((source.uri.clone(), relative_uri(output_glb, &rel)));
            }
            if proposed.len() == binding.mip_levels.as_ref().unwrap().len()
                && proposed
                    .iter()
                    .all(|(_, target)| !used_destinations.contains(target))
            {
                used_destinations.extend(proposed.iter().map(|(_, target)| target.clone()));
                redirects.extend(proposed);
                break;
            }
        }
    }
    let rewrite = |uri: &mut String| {
        if let Some(target) = redirects.get(uri) {
            *uri = target.clone();
        }
    };
    for texture in &mut converted.model.textures {
        rewrite(&mut texture.uri);
        for mip in &mut texture.mip_levels {
            rewrite(&mut mip.uri);
        }
    }
    for material in &mut converted.model.materials {
        for binding in &mut material.texture_bindings {
            if let Some(uri) = &mut binding.uri {
                rewrite(uri);
            }
            for mip in binding.mip_levels.iter_mut().flatten() {
                rewrite(&mut mip.uri);
            }
        }
    }
    for report in &mut converted.texture_reports {
        rewrite(&mut report.uri);
        for mip in &mut report.mip_levels {
            rewrite(&mut mip.uri);
        }
    }
    for file in &mut converted.texture_files {
        rewrite(&mut file.uri);
    }
    let mut files = BTreeMap::new();
    for file in std::mem::take(&mut converted.texture_files) {
        if let Some(previous) = files.insert(file.uri.clone(), file.bytes.clone()) {
            if previous != file.bytes {
                return invalid("texture reuse produced conflicting output bytes");
            }
        }
    }
    converted.texture_files = files
        .into_iter()
        .map(|(uri, bytes)| TexturePublication { uri, bytes })
        .collect();
    Ok(())
}

#[cfg(test)]
#[path = "native_texture_reuse/tests.rs"]
mod tests;
