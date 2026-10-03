use super::*;

#[derive(Clone, Debug)]
pub(super) struct TextureUse {
    pub(super) absolute: PathBuf,
    pub(super) hash: String,
    pub(super) image_name: String,
    pub(super) material_names: Vec<String>,
}

#[derive(Clone, Debug)]
pub(super) struct TextureDestination {
    pub(super) rooted_path: String,
    pub(super) source: PathBuf,
}

#[derive(Clone, Debug, Default)]
pub(super) struct PlayerTexture {
    pub(super) sources: BTreeSet<PathBuf>,
    pub(super) names: BTreeSet<String>,
    pub(super) support: bool,
}

pub(super) fn register_player_texture(
    textures: &mut BTreeMap<String, PlayerTexture>,
    path: &Path,
    name: String,
    support: bool,
) -> Result<String> {
    let canonical = canonical_file(path, "player texture")?;
    let bytes = fs::read(&canonical).map_err(|error| io_at(&canonical, error))?;
    let hash = hash_bytes(&bytes);
    let entry = textures.entry(hash.clone()).or_default();
    entry.sources.insert(canonical);
    entry.names.insert(safe_slug(&name, "texture"));
    entry.support |= support;
    Ok(hash)
}

pub(super) fn player_model_texture_roots(
    model: &PlayerModel,
    textures: &BTreeMap<String, PlayerTexture>,
) -> BTreeSet<PathBuf> {
    model
        .atlas_hashes
        .iter()
        .chain(&model.support_hashes)
        .filter_map(|hash| textures.get(hash))
        .flat_map(|texture| texture.sources.iter())
        .filter_map(|path| path.parent())
        .filter(|parent| {
            parent
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.ends_with(".textures"))
        })
        .map(Path::to_path_buf)
        .collect()
}

pub(super) fn copy_player_texture_mips(
    asset_root: &Path,
    stage: &Path,
    source: &Path,
    destination: &str,
    path_map: &mut BTreeMap<String, String>,
) -> Result<()> {
    let source_stem = file_stem(source)?;
    let source_mips = source.with_file_name(format!("{source_stem}.mips"));
    if !source_mips.is_dir() {
        return Ok(());
    }
    let destination_stem = Path::new(destination)
        .file_stem()
        .and_then(|value| value.to_str())
        .ok_or_else(|| invalid_error("player texture destination has no stem"))?;
    let destination_parent = Path::new(destination)
        .parent()
        .ok_or_else(|| invalid_error("player texture destination has no parent"))?;
    for mip in collect_files(&source_mips)? {
        let within = mip
            .strip_prefix(&source_mips)
            .map_err(|_| invalid_error("player texture mip escaped root"))?;
        let rooted = slash_path(
            &destination_parent
                .join(format!("{destination_stem}.mips"))
                .join(within),
        );
        copy_new_or_equal(&mip, &player_stage_path(stage, &rooted)?)?;
        path_map.insert(asset_relative(asset_root, &mip)?, rooted);
    }
    Ok(())
}

pub(super) fn texture_groups(objects: &[MapObject]) -> (Vec<Vec<usize>>, BTreeMap<String, Vec<TextureUse>>) {
    let mut union = UnionFind::new(objects.len());
    let mut first = BTreeMap::<String, usize>::new();
    let mut users = BTreeMap::<String, Vec<TextureUse>>::new();
    for (index, object) in objects.iter().enumerate() {
        for texture in &object.textures {
            if let Some(other) = first.get(&texture.hash) {
                union.join(index, *other);
            } else {
                first.insert(texture.hash.clone(), index);
            }
            users
                .entry(texture.hash.clone())
                .or_default()
                .push(texture.clone());
        }
    }
    let mut groups = BTreeMap::<String, Vec<usize>>::new();
    for (index, object) in objects.iter().enumerate() {
        let key = if object.textures.is_empty() {
            let base = strip_variant(object.old_relative.rsplit('/').next().unwrap_or("object"));
            format!(
                "empty/{}/{}/{}/{}",
                object.category, object.prefix, object.family, base
            )
        } else {
            format!("texture/{}", union.find(index))
        };
        groups.entry(key).or_default().push(index);
    }
    (groups.into_values().collect(), users)
}

pub(super) fn semantic_texture_name(uses: &[TextureUse], fallback: &str) -> String {
    let image_names = uses
        .iter()
        .map(|usage| usage.image_name.as_str())
        .filter(|name| meaningful_image_name(name))
        .map(str::to_owned)
        .collect::<Vec<_>>();
    if let Some(name) = common_semantic_suffix(&image_names) {
        return name;
    }
    let materials = uses
        .iter()
        .flat_map(|usage| usage.material_names.iter().cloned())
        .collect::<Vec<_>>();
    common_semantic_suffix(&materials).unwrap_or_else(|| safe_slug(fallback, "atlas"))
}

pub(super) fn copy_texture_mips(
    asset_root: &Path,
    stage: &Path,
    source: &Path,
    destination: &str,
    path_map: &mut BTreeMap<String, String>,
) -> Result<()> {
    let Some(source_stem) = source.file_stem().and_then(|value| value.to_str()) else {
        return Ok(());
    };
    let source_mips = source.with_file_name(format!("{source_stem}.mips"));
    if !source_mips.is_dir() {
        return Ok(());
    }
    let destination_stem = Path::new(destination)
        .file_stem()
        .and_then(|value| value.to_str())
        .ok_or_else(|| invalid_error("texture destination has no stem"))?;
    let destination_parent = Path::new(destination)
        .parent()
        .ok_or_else(|| invalid_error("texture destination has no parent"))?;
    for mip in collect_files(&source_mips)? {
        let within = mip
            .strip_prefix(&source_mips)
            .map_err(|_| invalid_error("texture mip escaped its directory"))?;
        let rooted = slash_path(
            &destination_parent
                .join(format!("{destination_stem}.mips"))
                .join(within),
        );
        copy_new_or_equal(&mip, &stage_path_for_rooted(stage, &rooted)?)?;
        path_map.insert(asset_relative(asset_root, &mip)?, rooted);
    }
    Ok(())
}

pub(super) fn object_texture_roots(object: &MapObject) -> BTreeSet<PathBuf> {
    object
        .textures
        .iter()
        .filter_map(|texture| texture.absolute.parent())
        .filter(|parent| {
            parent
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.ends_with(".textures"))
        })
        .map(Path::to_path_buf)
        .collect()
}

pub(super) fn is_owned_texture_file(path: &Path, roots: &BTreeSet<PathBuf>) -> bool {
    roots.iter().any(|root| path.starts_with(root))
}
