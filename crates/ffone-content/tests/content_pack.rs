use std::{collections::BTreeSet, fs, path::Path};

use ffone_content::{
    CONTENT_PACK_SCHEMA, CONTENT_PROTOCOL, ContentError, ContentKind, ContentManifest, ContentPack,
    ContentPackBuilder, MANIFEST_FILE, Provenance, SourceFingerprint, validate_pack,
};
use tempfile::TempDir;

fn digest(bytes: &[u8]) -> String {
    blake3::hash(bytes).to_hex().to_string()
}

fn provenance() -> Provenance {
    Provenance::new(
        "FFOne native exporter",
        vec![SourceFingerprint::new(
            "retrobution-content",
            digest(b"neutral source fingerprint"),
        )],
    )
}

fn stage(root: &Path, path: &str, bytes: &[u8]) {
    let absolute = root.join(path.replace('/', std::path::MAIN_SEPARATOR_STR));
    if let Some(parent) = absolute.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(absolute, bytes).unwrap();
}

fn one_file_pack(path: &str, bytes: &[u8]) -> (TempDir, ContentManifest) {
    let temp = tempfile::tempdir().unwrap();
    stage(temp.path(), path, bytes);
    let mut builder = ContentPackBuilder::new("ru-RU", provenance());
    builder.register(path, ContentKind::Table).unwrap();
    let manifest = builder.write(temp.path()).unwrap();
    (temp, manifest)
}

fn rewrite_manifest(root: &Path, manifest: &ContentManifest) {
    let mut bytes = serde_json::to_vec_pretty(manifest).unwrap();
    bytes.push(b'\n');
    fs::write(root.join(MANIFEST_FILE), bytes).unwrap();
}

#[test]
fn valid_pack_is_deterministic_and_runtime_only_opens_listed_files() {
    let temp = tempfile::tempdir().unwrap();
    stage(temp.path(), "world/sector.ffworld", b"native world");
    stage(temp.path(), "tables/nanos.json", br#"{"nanos":[]}"#);

    let mut builder = ContentPackBuilder::new("ru-RU", provenance());
    builder
        .register("world/sector.ffworld", ContentKind::World)
        .unwrap()
        .register("tables/nanos.json", ContentKind::Table)
        .unwrap();
    let manifest = builder.write(temp.path()).unwrap();

    assert_eq!(manifest.schema, CONTENT_PACK_SCHEMA);
    assert_eq!(manifest.protocol, CONTENT_PROTOCOL);
    assert_eq!(manifest.locale, "ru-RU");
    assert_eq!(manifest.files[0].path, "tables/nanos.json");
    assert_eq!(manifest.files[1].path, "world/sector.ffworld");

    let first_manifest = fs::read(temp.path().join(MANIFEST_FILE)).unwrap();
    let second_manifest = builder.write(temp.path()).unwrap();
    assert_eq!(manifest, second_manifest);
    assert_eq!(
        first_manifest,
        fs::read(temp.path().join(MANIFEST_FILE)).unwrap()
    );
    assert_eq!(validate_pack(temp.path()).unwrap(), manifest);

    let pack = ContentPack::open(temp.path()).unwrap();
    assert_eq!(pack.read("tables/nanos.json").unwrap(), br#"{"nanos":[]}"#);
    assert!(pack.entry("world/sector.ffworld").is_some());
    assert!(pack.entry("WORLD/SECTOR.FFWORLD").is_none());
    assert!(matches!(
        pack.read("../tables/nanos.json"),
        Err(ContentError::NotManifestListed(_))
    ));
    assert!(matches!(
        pack.read(MANIFEST_FILE),
        Err(ContentError::NotManifestListed(_))
    ));

    let json: serde_json::Value = serde_json::from_slice(&first_manifest).unwrap();
    let top_fields: BTreeSet<_> = json
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(
        top_fields,
        BTreeSet::from(["files", "locale", "protocol", "provenance", "schema"])
    );
    let provenance_fields: BTreeSet<_> = json["provenance"]
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(provenance_fields, BTreeSet::from(["producer", "sources"]));
    let source_fields: BTreeSet<_> = json["provenance"]["sources"][0]
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(source_fields, BTreeSet::from(["blake3", "label"]));
    let file_fields: BTreeSet<_> = json["files"][0]
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(
        file_fields,
        BTreeSet::from(["blake3", "bytes", "kind", "path"])
    );
    assert!(fs::read_dir(temp.path()).unwrap().all(|entry| {
        !entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with(".content-pack.json.")
    }));
}

#[test]
fn paths_are_portable_printable_ascii_and_windows_safe() {
    let rejected = [
        "tables/таблица.json",
        "tables/bad?.json",
        "tables/bad*.json",
        "tables/bad<.json",
        "tables/bad>.json",
        "tables/bad\".json",
        "tables/bad|.json",
        "CON",
        "tables/nul.txt",
        "tables/AUX.data",
        "tables/PrN.json",
        "tables/COM1.bin",
        "tables/lPt9.bin",
        "tables/CLOCK$",
        "tables/CONIN$.txt",
        "tables/CONOUT$",
    ];

    for path in rejected {
        let mut builder = ContentPackBuilder::new("ru-RU", provenance());
        assert!(
            matches!(
                builder.register(path, ContentKind::Table),
                Err(ContentError::UnsafePath { .. })
            ),
            "unsafe Windows path was accepted: {path}"
        );
    }

    let mut builder = ContentPackBuilder::new("ru-RU", provenance());
    builder
        .register("tables/Items.json", ContentKind::Table)
        .unwrap();
    assert!(matches!(
        builder.register("TABLES/ITEMS.JSON", ContentKind::Table),
        Err(ContentError::DuplicateRegistration(_))
    ));
    assert!(
        builder
            .register("tables/COM10.bin", ContentKind::Table)
            .is_ok()
    );
    assert!(
        builder
            .register("shaders/native.wgsl", ContentKind::Shader)
            .is_ok()
    );
}

#[test]
fn every_legacy_name_and_extension_is_rejected_case_insensitively() {
    let cases = [
        "main.UNITY3D",
        "stream.RESOURCEFILE",
        "sharedassets0.AsSeTs",
        "project.FFCLIENT/native.mesh",
        "content.AssetBundle",
        "content.BUNDLE",
        "texture.ReSS",
        "AssetBundles/native.bin",
        "GlobalGameManagers",
        "MainData",
        "CAB-deadbeef/native.bin",
        "LeVeL17",
        "meshes/legacy.NIF",
        "animations/legacy.KFM",
        "animations/legacy.KF",
        "shaders/legacy.CG",
        "meshes/intermediate.OBJ",
    ];

    for path in cases {
        let temp = tempfile::tempdir().unwrap();
        stage(temp.path(), path, b"legacy payload");
        let mut builder = ContentPackBuilder::new("ru-RU", provenance());
        let result = match builder.register(path, ContentKind::Mesh) {
            Ok(_) => builder.write(temp.path()).map(|_| ()),
            Err(error) => Err(error),
        };
        assert!(
            matches!(result, Err(ContentError::ForbiddenLegacy { .. })),
            "legacy path was accepted: {path}"
        );
    }
}

#[test]
fn legacy_magic_is_rejected_even_with_native_looking_name() {
    for signature in [
        b"UnityFS".as_slice(),
        b"UnityRaw",
        b"UnityWeb",
        b"Gamebryo File Format",
        b"NetImmerse File Format",
    ] {
        let temp = tempfile::tempdir().unwrap();
        let mut bytes = signature.to_vec();
        bytes.extend_from_slice(b"\0hidden bundle");
        stage(temp.path(), "meshes/innocent.ffmesh", &bytes);
        let mut builder = ContentPackBuilder::new("ru-RU", provenance());
        builder
            .register("meshes/innocent.ffmesh", ContentKind::Mesh)
            .unwrap();
        assert!(matches!(
            builder.write(temp.path()),
            Err(ContentError::ForbiddenLegacy { .. })
        ));
    }
}

#[test]
fn legacy_authoring_metadata_is_rejected_inside_native_json() {
    let hostile = [
        br#"{"sourceBundle":"main.unity3d"}"#.as_slice(),
        br#"{"pathId":7}"#,
        br#"{"path":"C:/authoring/source"}"#,
        br#"{"path":"\\\\server\\share\\source"}"#,
        br#"{"path":"/tmp/authoring/source"}"#,
        br#"{"nested":{"file":"archive:/CAB-deadbeef/object"}}"#,
    ];

    for bytes in hostile {
        let temp = tempfile::tempdir().unwrap();
        stage(temp.path(), "tables/native.json", bytes);
        let mut builder = ContentPackBuilder::new("ru-RU", provenance());
        builder
            .register("tables/native.json", ContentKind::Table)
            .unwrap();
        assert!(
            matches!(
                builder.write(temp.path()),
                Err(ContentError::ForbiddenLegacy { .. })
            ),
            "legacy JSON metadata was accepted: {}",
            String::from_utf8_lossy(bytes)
        );
    }
}

#[test]
fn native_json_allows_relative_paths_and_dialogue_slashes() {
    let bytes = br#"{
        "path":"textures/native.png",
        "dialogue":"Use /help when you need it.",
        "shader":"native.wgsl"
    }"#;
    let (temp, _) = one_file_pack("tables/native.json", bytes);
    assert!(ContentPack::open(temp.path()).is_ok());
}

#[test]
fn validator_and_open_pack_detect_hash_tampering() {
    let (temp, _) = one_file_pack("tables/items.json", br#"{"v":"good"}"#);
    let pack = ContentPack::open(temp.path()).unwrap();
    stage(temp.path(), "tables/items.json", br#"{"v":"evil"}"#);

    assert!(matches!(
        validate_pack(temp.path()),
        Err(ContentError::HashMismatch { .. })
    ));
    assert!(matches!(
        pack.read("tables/items.json"),
        Err(ContentError::HashMismatch { .. })
    ));
}

#[test]
fn validator_detects_size_tampering() {
    let (temp, _) = one_file_pack("tables/items.json", br#"{"v":"good"}"#);
    stage(
        temp.path(),
        "tables/items.json",
        br#"{"v":"longer malicious content"}"#,
    );
    assert!(matches!(
        validate_pack(temp.path()),
        Err(ContentError::SizeMismatch { .. })
    ));
}

#[test]
fn validator_rejects_traversal_and_absolute_manifest_paths() {
    for hostile in [
        "../escape.bin",
        "tables/../escape.bin",
        "C:/escape.bin",
        "/escape.bin",
    ] {
        let (temp, mut manifest) = one_file_pack("tables/items.json", br#"{"native":true}"#);
        manifest.files[0].path = hostile.to_owned();
        rewrite_manifest(temp.path(), &manifest);
        assert!(
            matches!(
                validate_pack(temp.path()),
                Err(ContentError::UnsafePath { .. })
            ),
            "hostile path was accepted: {hostile}"
        );
    }
}

#[test]
fn validator_rejects_unsorted_file_entries() {
    let temp = tempfile::tempdir().unwrap();
    stage(temp.path(), "tables/a.json", br#""a""#);
    stage(temp.path(), "tables/b.json", br#""b""#);
    let mut builder = ContentPackBuilder::new("ru-RU", provenance());
    builder
        .register("tables/a.json", ContentKind::Table)
        .unwrap()
        .register("tables/b.json", ContentKind::Table)
        .unwrap();
    let mut manifest = builder.write(temp.path()).unwrap();
    manifest.files.reverse();
    rewrite_manifest(temp.path(), &manifest);

    assert!(matches!(
        validate_pack(temp.path()),
        Err(ContentError::UnsortedFiles(_))
    ));
}

#[test]
fn validator_rejects_duplicate_and_case_colliding_paths() {
    let (temp, mut manifest) = one_file_pack("tables/items.json", br#"{"native":true}"#);
    let mut duplicate = manifest.files[0].clone();
    duplicate.path = "TABLES/ITEMS.JSON".to_owned();
    manifest.files.push(duplicate);
    rewrite_manifest(temp.path(), &manifest);

    assert!(matches!(
        validate_pack(temp.path()),
        Err(ContentError::DuplicatePath(_))
    ));
}

#[test]
fn builder_and_validator_reject_unregistered_or_unlisted_files() {
    let temp = tempfile::tempdir().unwrap();
    stage(temp.path(), "tables/items.json", br#"{"native":true}"#);
    stage(temp.path(), "meshes/extra.ffmesh", b"extra");
    let mut builder = ContentPackBuilder::new("ru-RU", provenance());
    builder
        .register("tables/items.json", ContentKind::Table)
        .unwrap();
    assert!(matches!(
        builder.write(temp.path()),
        Err(ContentError::UnregisteredStagingFile(_))
    ));

    fs::remove_file(temp.path().join("meshes/extra.ffmesh")).unwrap();
    builder.write(temp.path()).unwrap();
    stage(temp.path(), "meshes/extra.ffmesh", b"extra");
    assert!(matches!(
        validate_pack(temp.path()),
        Err(ContentError::UnlistedPackFile(_))
    ));
}

#[test]
fn symlinked_content_cannot_escape_the_pack_root() {
    let pack = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    stage(outside.path(), "outside.json", b"outside");
    fs::create_dir_all(pack.path().join("tables")).unwrap();
    let link = pack.path().join("tables/linked.json");
    let target = outside.path().join("outside.json");

    #[cfg(unix)]
    let link_result = std::os::unix::fs::symlink(&target, &link);
    #[cfg(windows)]
    let link_result = std::os::windows::fs::symlink_file(&target, &link);
    if let Err(error) = link_result {
        #[cfg(windows)]
        if error.raw_os_error() == Some(1314) {
            return;
        }
        if error.kind() == std::io::ErrorKind::PermissionDenied {
            return;
        }
        panic!("failed to create test symlink: {error}");
    }

    let mut builder = ContentPackBuilder::new("ru-RU", provenance());
    builder
        .register("tables/linked.json", ContentKind::Table)
        .unwrap();
    assert!(matches!(
        builder.write(pack.path()),
        Err(ContentError::Symlink(_) | ContentError::ReparsePoint(_))
    ));
}

#[cfg(windows)]
#[test]
fn windows_directory_junctions_are_rejected_as_reparse_points() {
    use std::process::Command;

    let pack = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    stage(outside.path(), "native.json", b"outside");
    let junction = pack.path().join("linked");
    let status = Command::new("cmd")
        .args(["/C", "mklink", "/J"])
        .arg(&junction)
        .arg(outside.path())
        .status()
        .unwrap();
    assert!(status.success(), "failed to create test directory junction");

    let mut builder = ContentPackBuilder::new("ru-RU", provenance());
    builder
        .register("linked/native.json", ContentKind::Table)
        .unwrap();
    assert!(matches!(
        builder.write(pack.path()),
        Err(ContentError::Symlink(_) | ContentError::ReparsePoint(_))
    ));
}
