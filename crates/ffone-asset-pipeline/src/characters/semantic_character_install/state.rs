use super::*;

pub(super) const RUNTIME_CHARACTER_PACKAGE_ROOTS: &[&str] = &[
    "characters/mobs",
    "characters/fusions",
    "characters/npcs",
    "characters/nanos",
    "characters/shared",
];

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeCharacterCategory {
    Nano,
    Npc,
    Mob,
    Fusion,
    Shared,
}

impl RuntimeCharacterCategory {
    pub(super) fn directory(self) -> &'static str {
        match self {
            Self::Nano => "characters/nanos",
            Self::Npc => "characters/npcs",
            Self::Mob => "characters/mobs",
            Self::Fusion => "characters/fusions",
            Self::Shared => "characters/shared",
        }
    }

    pub(super) fn label(self) -> &'static str {
        match self {
            Self::Nano => "nano",
            Self::Npc => "npc",
            Self::Mob => "mob",
            Self::Fusion => "fusion",
            Self::Shared => "shared",
        }
    }

    pub(super) fn spawn_policy(self) -> &'static str {
        match self {
            Self::Nano => "nano-preserve-authored-root-scale",
            Self::Npc | Self::Mob | Self::Fusion | Self::Shared => {
                "npc-setup-replace-root-trs-with-spawn-and-table-row-scale"
            }
        }
    }
}

pub(super) fn runtime_closure_files(
    candidate_root: &Path,
    source_glb: &str,
    destination_directory: &str,
) -> Result<Vec<RuntimeCopy>> {
    let glb_path = join_relative(candidate_root, source_glb)?;
    let source_directory = glb_path
        .parent()
        .ok_or_else(|| invalid_error("candidate GLB has no parent"))?;
    let source_glb_name = glb_path
        .file_name()
        .ok_or_else(|| invalid_error("candidate GLB has no filename"))?;
    let mut pending = vec![source_directory.to_path_buf()];
    let mut copies = Vec::new();
    let mut glbs = 0_u64;
    while let Some(directory) = pending.pop() {
        for entry in fs::read_dir(&directory).map_err(|error| io_at(&directory, error))? {
            let entry = entry.map_err(|error| io_at(&directory, error))?;
            let file_type = entry
                .file_type()
                .map_err(|error| io_at(entry.path(), error))?;
            if file_type.is_symlink() {
                return invalid("candidate character closure contains a symlink");
            }
            if file_type.is_dir() {
                pending.push(entry.path());
                continue;
            }
            if !file_type.is_file() {
                return invalid("candidate character closure contains a non-regular file");
            }
            let path = entry.path();
            let extension = path
                .extension()
                .and_then(|value| value.to_str())
                .unwrap_or_default()
                .to_ascii_lowercase();
            let kind = match extension.as_str() {
                "glb" => {
                    if path.file_name() != Some(source_glb_name) {
                        return invalid("candidate model directory contains an extra GLB");
                    }
                    glbs += 1;
                    ProjectAssetKind::Model
                }
                "png" => ProjectAssetKind::Texture,
                "json"
                    if path
                        .file_name()
                        .and_then(|value| value.to_str())
                        .is_some_and(|value| value.ends_with(".publish.json")) =>
                {
                    continue;
                }
                _ => {
                    return invalid(format!(
                        "candidate model closure has an unsupported runtime file: {path:?}"
                    ));
                }
            };
            let tail = relative_path(source_directory, &path)?;
            copies.push(RuntimeCopy {
                source: path,
                destination: format!("{destination_directory}/{tail}"),
                kind,
            });
        }
    }
    if glbs != 1 {
        return invalid("candidate model closure must contain exactly one GLB");
    }
    copies.sort_by(|left, right| left.destination.cmp(&right.destination));
    Ok(copies)
}

pub(super) fn runtime_character_package_root(path: &str) -> Result<Option<String>> {
    for category_root in RUNTIME_CHARACTER_PACKAGE_ROOTS {
        let prefix = format!("{category_root}/");
        let Some(remainder) = path.strip_prefix(&prefix) else {
            continue;
        };
        let Some((stable_id, relative)) = remainder.split_once('/') else {
            return invalid(format!(
                "modern character asset is outside a stable package directory: {path:?}"
            ));
        };
        validate_file_name(stable_id)?;
        if relative.is_empty() {
            return invalid(format!(
                "modern character asset path is incomplete: {path:?}"
            ));
        }
        return Ok(Some(format!("{category_root}/{stable_id}")));
    }
    Ok(None)
}
