//! Read immutable containers without an extraction directory.
use super::unity::{Asset, UnityEnvironment, collect_archive_dependencies, normalize_bundle_name};
use std::{collections::BTreeSet, path::Path};

pub(crate) fn read_assets(path: &Path) -> Result<Vec<Asset>, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    if !bytes.starts_with(b"Unity") && !bytes.starts_with(b"streamed") {
        return Ok(vec![Asset::from_bytes(
            path.file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned(),
            bytes,
        )?]);
    }
    let (_, bundle) = ffbuildtool::bundle::AssetBundle::from_bytes(&bytes)?;
    let mut assets = Vec::new();
    for (_, name, payload) in bundle.iter_files() {
        // Managed assemblies and raw resources are not serialized object files.
        if name.ends_with(".dll") || name.ends_with(".resource") {
            continue;
        }
        assets.push(
            Asset::from_bytes(name.to_owned(), payload.to_vec())
                .map_err(|e| format!("{}::{name}: {e}", path.display()))?,
        );
    }
    if assets.is_empty() {
        return Err("container contains no serialized assets".into());
    }
    Ok(assets)
}

pub(crate) fn load(path: &Path) -> Result<(UnityEnvironment, BTreeSet<String>), String> {
    let assets = read_assets(path)?;
    let roots = assets.iter().map(|a| a.name.clone()).collect();
    let mut environment = UnityEnvironment::from_assets(assets);
    let mut visited = BTreeSet::from([path.canonicalize().map_err(|e| e.to_string())?]);
    let root = path.parent().ok_or("container has no parent")?;
    let mut index = None;
    loop {
        let present: BTreeSet<_> = environment
            .assets
            .iter()
            .map(|a| normalize_bundle_name(&a.name))
            .collect();
        let missing: Vec<_> = collect_archive_dependencies(&environment)
            .into_iter()
            .filter(|name| !present.contains(&normalize_bundle_name(name)))
            .collect();
        if missing.is_empty() {
            return Ok((environment, roots));
        }
        let candidates =
            index.get_or_insert_with(|| super::world::build_archive_candidate_index(root));
        let mut additions = Vec::new();
        for name in missing {
            let paths = candidates
                .get(&normalize_bundle_name(&name))
                .ok_or_else(|| format!("unresolved archive dependency {name}"))?;
            let [dependency] = paths.as_slice() else {
                return Err(format!("ambiguous archive dependency {name}: {paths:?}"));
            };
            let canonical = dependency.canonicalize().map_err(|e| e.to_string())?;
            if visited.insert(canonical) {
                additions.extend(read_assets(dependency)?);
            }
        }
        if additions.is_empty() {
            return Err("archive references remain unresolved after dependency traversal".into());
        }
        let mut assets = environment.assets;
        for asset in additions {
            if assets.iter().any(|a| a.name == asset.name) {
                return Err(format!("ambiguous serialized asset {}", asset.name));
            }
            assets.push(asset);
        }
        environment = UnityEnvironment::from_assets(assets);
    }
}
