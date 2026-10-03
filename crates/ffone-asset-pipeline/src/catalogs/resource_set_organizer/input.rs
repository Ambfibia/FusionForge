use super::*;

#[derive(Clone, Debug)]
pub(super) struct UnionFind {
    pub(super) parent: Vec<usize>,
}

impl UnionFind {
    pub(super) fn new(length: usize) -> Self {
        Self {
            parent: (0..length).collect(),
        }
    }

    pub(super) fn find(&mut self, mut value: usize) -> usize {
        let mut root = value;
        while self.parent[root] != root {
            root = self.parent[root];
        }
        while self.parent[value] != value {
            let next = self.parent[value];
            self.parent[value] = root;
            value = next;
        }
        root
    }

    pub(super) fn join(&mut self, left: usize, right: usize) {
        let left = self.find(left);
        let right = self.find(right);
        if left != right {
            self.parent[right] = left;
        }
    }
}

pub(super) fn load_player_models(
    asset_root: &Path,
    equipment_root: &Path,
) -> Result<(Vec<PlayerModel>, BTreeMap<String, PlayerTexture>)> {
    let mut textures = BTreeMap::<String, PlayerTexture>::new();
    let mut models = Vec::new();
    for glb in collect_files(equipment_root)?
        .into_iter()
        .filter(|path| path.extension().and_then(|value| value.to_str()) == Some("glb"))
    {
        let directory = glb
            .parent()
            .ok_or_else(|| invalid_error("player GLB has no parent"))?
            .to_path_buf();
        let relative = glb
            .strip_prefix(equipment_root)
            .map_err(|_| invalid_error("player GLB escaped equipment root"))?;
        let category = relative
            .components()
            .next()
            .and_then(|component| component.as_os_str().to_str())
            .ok_or_else(|| invalid_error("player GLB has no category"))?
            .to_owned();
        let true_name = file_stem(&glb)?;
        let document = read_glb_json(&glb)?;
        let mut atlas_hashes = BTreeSet::new();
        let mut support_hashes = BTreeSet::new();
        for image in document
            .get("images")
            .and_then(JsonValue::as_array)
            .into_iter()
            .flatten()
        {
            let Some(uri) = image.get("uri").and_then(JsonValue::as_str) else {
                continue;
            };
            if uri.starts_with("data:") {
                continue;
            }
            let texture_path =
                canonical_file(&directory.join(Path::new(uri)), "player model texture")?;
            let image_name = image
                .get("name")
                .and_then(JsonValue::as_str)
                .unwrap_or_else(|| {
                    texture_path
                        .file_stem()
                        .and_then(|value| value.to_str())
                        .unwrap_or("texture")
                });
            let support = image_name.to_ascii_lowercase().contains("toonramp")
                || uri.to_ascii_lowercase().contains("toonramp");
            let hash = register_player_texture(
                &mut textures,
                &texture_path,
                image_name.to_owned(),
                support,
            )?;
            if support {
                support_hashes.insert(hash);
            } else {
                atlas_hashes.insert(hash);
            }
        }
        models.push(PlayerModel {
            old_glb: glb.clone(),
            old_rooted: asset_relative(asset_root, &glb)?,
            directory: directory.clone(),
            category,
            true_name,
            source_route: slash_path(relative),
            files: collect_files(&directory)?,
            atlas_hashes,
            support_hashes,
        });
    }
    Ok((models, textures))
}

pub(super) fn load_map_objects(object_root: &Path) -> Result<Vec<MapObject>> {
    let definitions = find_named_files(object_root, "object.json")?;
    let mut objects = Vec::with_capacity(definitions.len());
    for definition_path in definitions {
        let directory = definition_path
            .parent()
            .ok_or_else(|| invalid_error("object definition has no parent"))?;
        let old_relative = slash_path(
            directory
                .strip_prefix(object_root)
                .map_err(|_| invalid_error("object definition escaped object root"))?,
        );
        let definition = read_json(&definition_path, "map object definition")?;
        let field = |name: &str| -> Result<String> {
            definition
                .get(name)
                .and_then(JsonValue::as_str)
                .map(str::to_owned)
                .ok_or_else(|| invalid_error(format!("map object has no {name}")))
        };
        let files = collect_files(directory)?;
        let glbs = files
            .iter()
            .filter(|path| path.extension().and_then(|value| value.to_str()) == Some("glb"))
            .cloned()
            .collect::<Vec<_>>();
        let mut textures = Vec::new();
        for glb in &glbs {
            let document = read_glb_json(glb)?;
            let materials = document
                .get("materials")
                .and_then(JsonValue::as_array)
                .into_iter()
                .flatten()
                .filter_map(|material| material.get("name").and_then(JsonValue::as_str))
                .map(str::to_owned)
                .collect::<Vec<_>>();
            for image in document
                .get("images")
                .and_then(JsonValue::as_array)
                .into_iter()
                .flatten()
            {
                let Some(uri) = image.get("uri").and_then(JsonValue::as_str) else {
                    continue;
                };
                if uri.starts_with("data:") {
                    continue;
                }
                let absolute = canonical_file(
                    &glb.parent()
                        .ok_or_else(|| invalid_error("GLB has no parent"))?
                        .join(uri),
                    "map-object texture",
                )?;
                let bytes = fs::read(&absolute).map_err(|error| io_at(&absolute, error))?;
                textures.push(TextureUse {
                    absolute,
                    hash: hash_bytes(&bytes),
                    image_name: image
                        .get("name")
                        .and_then(JsonValue::as_str)
                        .unwrap_or_default()
                        .to_owned(),
                    material_names: materials.clone(),
                });
            }
        }
        textures.sort_by(|left, right| left.hash.cmp(&right.hash));
        textures.dedup_by(|left, right| left.hash == right.hash);
        objects.push(MapObject {
            old_relative,
            category: field("category")?,
            prefix: field("prefix")?,
            family: field("family")?,
            id: field("id")?,
            name: field("name")?,
            definition,
            files,
            textures,
        });
    }
    Ok(objects)
}

pub(super) fn collect_staged_artifacts(stage: &Path, rooted: &str) -> Result<Vec<ResourceSetArtifact>> {
    let directory = stage_path_for_rooted(stage, rooted)?;
    let mut artifacts = collect_files(&directory)?
        .into_iter()
        .map(|path| {
            let rooted_path = format!(
                "{rooted}/{}",
                slash_path(
                    path.strip_prefix(&directory)
                        .map_err(|_| invalid_error("staged file escaped its package"))?
                )
            );
            artifact_from_staged(stage, &rooted_path)
        })
        .collect::<Result<Vec<_>>>()?;
    artifacts.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(artifacts)
}

pub(super) fn find_named_files(root: &Path, name: &str) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(directory) = pending.pop() {
        let mut entries = fs::read_dir(&directory)
            .map_err(|error| io_at(&directory, error))?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|error| io_at(&directory, error))?;
        entries.sort_by_key(|entry| entry.file_name());
        for entry in entries {
            let kind = entry
                .file_type()
                .map_err(|error| io_at(entry.path(), error))?;
            if kind.is_symlink() {
                return invalid(format!(
                    "resource tree contains a symlink: {:?}",
                    entry.path()
                ));
            }
            if kind.is_dir() {
                pending.push(entry.path());
            } else if kind.is_file() && entry.file_name() == name {
                files.push(entry.path());
            }
        }
    }
    files.sort();
    Ok(files)
}

pub(super) fn collect_files(root: &Path) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(directory) = pending.pop() {
        let mut entries = fs::read_dir(&directory)
            .map_err(|error| io_at(&directory, error))?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|error| io_at(&directory, error))?;
        entries.sort_by_key(|entry| entry.file_name());
        for entry in entries {
            let kind = entry
                .file_type()
                .map_err(|error| io_at(entry.path(), error))?;
            if kind.is_symlink() {
                return invalid(format!(
                    "resource tree contains a symlink: {:?}",
                    entry.path()
                ));
            }
            if kind.is_dir() {
                pending.push(entry.path());
            } else if kind.is_file() {
                files.push(entry.path());
            }
        }
    }
    files.sort();
    Ok(files)
}

pub(super) fn read_u32(bytes: &[u8], offset: usize) -> Result<u32> {
    let raw = bytes
        .get(offset..offset + 4)
        .ok_or_else(|| invalid_error("truncated GLB integer"))?;
    Ok(u32::from_le_bytes(
        raw.try_into()
            .map_err(|_| invalid_error("invalid GLB integer"))?,
    ))
}

pub(super) fn read_json(path: &Path, label: &str) -> Result<JsonValue> {
    let bytes = fs::read(path).map_err(|error| io_at(path, error))?;
    serde_json::from_slice(&bytes).map_err(|source| PipelineError::Json {
        path: format!("{label}: {}", path.display()),
        source,
    })
}
