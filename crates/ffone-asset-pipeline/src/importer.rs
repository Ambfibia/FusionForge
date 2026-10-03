use std::{
    collections::BTreeMap,
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

use ffone_content::{ContentFileEntry, ContentKind, ContentPack};

use crate::{
    ASSET_MANIFEST_FILE, PROJECT_ASSET_SCHEMA, PipelineError, ProjectAssetFile, ProjectAssetKind,
    ProjectAssetManifest, Result, SourcePackIdentity, mesh_json_to_glb,
    policy::{extension, reject_legacy_path, reject_legacy_text},
};

pub const DEFAULT_OUTPUT: &str = "work/ffone/imported-native-assets";

static STAGING_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImportOptions {
    pub pack_root: PathBuf,
    pub output_root: PathBuf,
}

impl ImportOptions {
    pub fn new(pack_root: impl Into<PathBuf>) -> Self {
        Self {
            pack_root: pack_root.into(),
            output_root: PathBuf::from(DEFAULT_OUTPUT),
        }
    }

    pub fn with_output(mut self, output_root: impl Into<PathBuf>) -> Self {
        self.output_root = output_root.into();
        self
    }
}

/// Imports a validated native content pack into a conventional, standalone Bevy asset tree.
///
/// The destination must not exist. All work is written to a sibling staging directory and
/// published with one rename, so a failed import cannot expose a partially updated asset tree.
pub fn import_content_pack(options: &ImportOptions) -> Result<ProjectAssetManifest> {
    if fs::symlink_metadata(&options.output_root).is_ok() {
        return Err(PipelineError::OutputExists(options.output_root.clone()));
    }

    let pack = ContentPack::open(&options.pack_root)?;
    let plans = plan_assets(pack.manifest().files.iter())?;
    let mut stage = StagingDirectory::create(&options.output_root)?;
    let mut files = Vec::with_capacity(plans.len());

    for plan in &plans {
        let source = pack.read(&plan.source_path)?;
        let output = transform_asset(plan, &source)?;
        write_new_file(stage.path(), &plan.target_path, &output)?;
        files.push(ProjectAssetFile {
            source_path: plan.source_path.clone(),
            path: plan.target_path.clone(),
            kind: plan.target_kind,
            bytes: output.len() as u64,
            blake3: blake3::hash(&output).to_hex().to_string(),
        });
    }
    files.sort_by(|left, right| left.path.cmp(&right.path));

    let canonical_source =
        serde_json::to_vec(pack.manifest()).map_err(|source| PipelineError::Json {
            path: "content-pack.json".to_owned(),
            source,
        })?;
    let manifest = ProjectAssetManifest {
        schema: PROJECT_ASSET_SCHEMA.to_owned(),
        protocol: pack.manifest().protocol,
        locale: pack.manifest().locale.clone(),
        source_pack: SourcePackIdentity {
            schema: pack.manifest().schema.clone(),
            manifest_blake3: blake3::hash(&canonical_source).to_hex().to_string(),
        },
        files,
    };
    write_project_manifest(stage.path(), &manifest)?;
    stage.publish(&options.output_root)?;
    Ok(manifest)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Transform {
    MeshToGlb,
    CopyPng,
    CopyOgg,
    CopyFont,
    CopyJson,
    CopyWgsl,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct AssetPlan {
    source_path: String,
    target_path: String,
    target_kind: ProjectAssetKind,
    transform: Transform,
}

fn plan_assets<'a>(entries: impl Iterator<Item = &'a ContentFileEntry>) -> Result<Vec<AssetPlan>> {
    let mut plans = Vec::new();
    let mut targets = BTreeMap::<String, String>::new();
    for entry in entries {
        reject_legacy_path(&entry.path)?;
        let plan = plan_asset(entry)?;
        reject_legacy_path(&plan.target_path)?;
        let folded = plan.target_path.to_ascii_lowercase();
        if let Some(first) = targets.insert(folded, entry.path.clone()) {
            return Err(PipelineError::OutputCollision {
                output: plan.target_path,
                first,
                second: entry.path.clone(),
            });
        }
        plans.push(plan);
    }
    Ok(plans)
}

fn plan_asset(entry: &ContentFileEntry) -> Result<AssetPlan> {
    let Some(extension) = extension(&entry.path).map(str::to_ascii_lowercase) else {
        return unsupported(&entry.path, "native asset has no filename extension");
    };
    let (target_path, target_kind, transform) = match extension.as_str() {
        "json" if entry.kind == ContentKind::Mesh => (
            replace_extension(
                &prefixed("models", trim_category(&entry.path, "meshes")),
                "glb",
            ),
            ProjectAssetKind::Model,
            Transform::MeshToGlb,
        ),
        "json" => (
            prefixed("data", &entry.path),
            ProjectAssetKind::Data,
            Transform::CopyJson,
        ),
        "png" if entry.kind == ContentKind::Texture => (
            prefixed("textures", trim_category(&entry.path, "textures")),
            ProjectAssetKind::Texture,
            Transform::CopyPng,
        ),
        "ogg" if entry.kind == ContentKind::Audio => (
            prefixed("audio", trim_category(&entry.path, "audio")),
            ProjectAssetKind::Audio,
            Transform::CopyOgg,
        ),
        "ttf" | "otf" if entry.kind == ContentKind::Font => (
            prefixed("fonts", trim_category(&entry.path, "fonts")),
            ProjectAssetKind::Font,
            Transform::CopyFont,
        ),
        "wgsl" if entry.kind == ContentKind::Shader => (
            prefixed("shaders", trim_category(&entry.path, "shaders")),
            ProjectAssetKind::Shader,
            Transform::CopyWgsl,
        ),
        _ => {
            return unsupported(
                &entry.path,
                &format!(
                    "extension .{extension} is not valid for content kind {:?}",
                    entry.kind
                ),
            );
        }
    };
    Ok(AssetPlan {
        source_path: entry.path.clone(),
        target_path,
        target_kind,
        transform,
    })
}

fn transform_asset(plan: &AssetPlan, source: &[u8]) -> Result<Vec<u8>> {
    match plan.transform {
        Transform::MeshToGlb => mesh_json_to_glb(&plan.source_path, source),
        Transform::CopyPng => {
            if !source.starts_with(b"\x89PNG\r\n\x1a\n") {
                return unsupported(&plan.source_path, "PNG signature is missing");
            }
            Ok(source.to_vec())
        }
        Transform::CopyOgg => {
            if !source.starts_with(b"OggS") {
                return unsupported(&plan.source_path, "Ogg stream signature is missing");
            }
            Ok(source.to_vec())
        }
        Transform::CopyFont => {
            let valid = source.starts_with(&[0x00, 0x01, 0x00, 0x00])
                || source.starts_with(b"OTTO")
                || source.starts_with(b"true")
                || source.starts_with(b"typ1");
            if !valid {
                return unsupported(&plan.source_path, "OpenType/TrueType signature is missing");
            }
            Ok(source.to_vec())
        }
        Transform::CopyJson => {
            reject_legacy_text(&plan.source_path, source)?;
            let _: serde_json::Value =
                serde_json::from_slice(source).map_err(|source| PipelineError::Json {
                    path: plan.source_path.clone(),
                    source,
                })?;
            Ok(source.to_vec())
        }
        Transform::CopyWgsl => {
            reject_legacy_text(&plan.source_path, source)?;
            if source.iter().all(u8::is_ascii_whitespace) {
                return unsupported(&plan.source_path, "WGSL source is empty");
            }
            Ok(source.to_vec())
        }
    }
}

fn write_project_manifest(root: &Path, manifest: &ProjectAssetManifest) -> Result<()> {
    let mut bytes = serde_json::to_vec_pretty(manifest).map_err(|source| PipelineError::Json {
        path: ASSET_MANIFEST_FILE.to_owned(),
        source,
    })?;
    bytes.push(b'\n');
    write_new_file(root, ASSET_MANIFEST_FILE, &bytes)
}

fn write_new_file(root: &Path, relative: &str, bytes: &[u8]) -> Result<()> {
    let absolute = relative
        .split('/')
        .fold(root.to_path_buf(), |path, component| path.join(component));
    let parent = absolute
        .parent()
        .ok_or_else(|| PipelineError::InvalidOutputPath(absolute.clone()))?;
    fs::create_dir_all(parent).map_err(|source| crate::error::io_at(parent, source))?;
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&absolute)
        .map_err(|source| crate::error::io_at(&absolute, source))?;
    file.write_all(bytes)
        .map_err(|source| crate::error::io_at(&absolute, source))?;
    Ok(())
}

fn trim_category<'a>(path: &'a str, category: &str) -> &'a str {
    path.split_once('/').map_or(path, |(first, rest)| {
        if first.eq_ignore_ascii_case(category) {
            rest
        } else {
            path
        }
    })
}

fn prefixed(prefix: &str, path: &str) -> String {
    format!("{prefix}/{path}")
}

fn replace_extension(path: &str, extension: &str) -> String {
    let stem = path.rsplit_once('.').map_or(path, |(stem, _)| stem);
    format!("{stem}.{extension}")
}

fn unsupported<T>(path: &str, reason: impl Into<String>) -> Result<T> {
    Err(PipelineError::UnsupportedAsset {
        path: path.to_owned(),
        reason: reason.into(),
    })
}

struct StagingDirectory {
    path: PathBuf,
    published: bool,
}

impl StagingDirectory {
    fn create(output: &Path) -> Result<Self> {
        let parent = output.parent().filter(|path| !path.as_os_str().is_empty());
        let parent = parent.unwrap_or_else(|| Path::new("."));
        fs::create_dir_all(parent).map_err(|source| crate::error::io_at(parent, source))?;
        let name = output
            .file_name()
            .and_then(|value| value.to_str())
            .filter(|value| !value.is_empty() && *value != "." && *value != "..")
            .ok_or_else(|| PipelineError::InvalidOutputPath(output.to_path_buf()))?;

        for _ in 0..64 {
            let sequence = STAGING_SEQUENCE.fetch_add(1, Ordering::Relaxed);
            let path = parent.join(format!(
                ".{name}.ffone-import.{}.{}",
                std::process::id(),
                sequence
            ));
            match fs::create_dir(&path) {
                Ok(()) => {
                    return Ok(Self {
                        path,
                        published: false,
                    });
                }
                Err(source) if source.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(source) => return Err(crate::error::io_at(path, source)),
            }
        }
        Err(PipelineError::StagingCollision(output.to_path_buf()))
    }

    fn path(&self) -> &Path {
        &self.path
    }

    fn publish(&mut self, output: &Path) -> Result<()> {
        if fs::symlink_metadata(output).is_ok() {
            return Err(PipelineError::OutputExists(output.to_path_buf()));
        }
        fs::rename(&self.path, output).map_err(|source| crate::error::io_at(output, source))?;
        self.published = true;
        Ok(())
    }
}

impl Drop for StagingDirectory {
    fn drop(&mut self) {
        if !self.published {
            let _ = fs::remove_dir_all(&self.path);
        }
    }
}
