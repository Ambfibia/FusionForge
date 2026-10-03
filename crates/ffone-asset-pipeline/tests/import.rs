use std::{fs, path::Path};

use ffone_asset_pipeline::{
    ASSET_MANIFEST_FILE, ImportOptions, PipelineError, ProjectAssetManifest, import_content_pack,
};
use ffone_content::{ContentKind, ContentPackBuilder, Provenance, SourceFingerprint};
use tempfile::TempDir;

const MESH: &str = r#"{"schema":"ffone.mesh.v1","name":"Triangle","positions":[[0,0,0],[1,0,0],[0,1,0]],"normals":[[0,0,1],[0,0,1],[0,0,1]],"uvs":[[0,0],[1,0],[0,1]],"submeshes":[{"indices":[0,1,2]}]}"#;

#[test]
fn imports_a_standalone_deterministic_bevy_asset_tree() {
    let temp = TempDir::new().unwrap();
    let pack = temp.path().join("pack");
    build_pack(&pack, b"\x89PNG\r\n\x1a\nfixture");

    let output_a = temp.path().join("assets-a").join("game");
    let output_b = temp.path().join("assets-b").join("game");
    let manifest_a =
        import_content_pack(&ImportOptions::new(&pack).with_output(&output_a)).unwrap();
    let manifest_b =
        import_content_pack(&ImportOptions::new(&pack).with_output(&output_b)).unwrap();

    assert_eq!(manifest_a, manifest_b);
    let paths = manifest_a
        .files
        .iter()
        .map(|file| file.path.as_str())
        .collect::<Vec<_>>();
    assert_eq!(
        paths,
        [
            "audio/sound.ogg",
            "data/fonts/font-index.json",
            "data/tables/game.json",
            "fonts/ui.ttf",
            "models/triangle.glb",
            "textures/ui.png",
        ]
    );
    assert_eq!(
        &fs::read(output_a.join("models/triangle.glb")).unwrap()[..4],
        b"glTF"
    );
    assert_eq!(
        fs::read(output_a.join("textures/ui.png")).unwrap(),
        b"\x89PNG\r\n\x1a\nfixture"
    );
    assert_eq!(
        fs::read(output_a.join("audio/sound.ogg")).unwrap(),
        b"OggSfixture"
    );

    let disk_manifest: ProjectAssetManifest =
        serde_json::from_slice(&fs::read(output_a.join(ASSET_MANIFEST_FILE)).unwrap()).unwrap();
    assert_eq!(disk_manifest, manifest_a);
    assert_eq!(
        fs::read(output_a.join(ASSET_MANIFEST_FILE)).unwrap(),
        fs::read(output_b.join(ASSET_MANIFEST_FILE)).unwrap()
    );
    assert_eq!(
        fs::read(output_a.join("models/triangle.glb")).unwrap(),
        fs::read(output_b.join("models/triangle.glb")).unwrap()
    );

    fs::write(output_a.join("textures/ui.png"), b"changed").unwrap();
    assert_eq!(
        fs::read(pack.join("textures/ui.png")).unwrap(),
        b"\x89PNG\r\n\x1a\nfixture"
    );
}

#[test]
fn failed_import_never_publishes_a_partial_output() {
    let temp = TempDir::new().unwrap();
    let pack = temp.path().join("pack");
    build_pack(&pack, b"not a png");
    let output = temp.path().join("assets").join("game");

    let error = import_content_pack(&ImportOptions::new(&pack).with_output(&output)).unwrap_err();
    assert!(matches!(error, PipelineError::UnsupportedAsset { .. }));
    assert!(!output.exists());
    let parent = output.parent().unwrap();
    let leftovers = fs::read_dir(parent)
        .unwrap()
        .filter_map(Result::ok)
        .collect::<Vec<_>>();
    assert!(leftovers.is_empty(), "staging leftovers: {leftovers:?}");
}

#[test]
fn existing_output_is_never_mutated() {
    let temp = TempDir::new().unwrap();
    let pack = temp.path().join("pack");
    build_pack(&pack, b"\x89PNG\r\n\x1a\nfixture");
    let output = temp.path().join("game");
    fs::create_dir(&output).unwrap();
    fs::write(output.join("keep.txt"), b"keep").unwrap();

    let error = import_content_pack(&ImportOptions::new(&pack).with_output(&output)).unwrap_err();
    assert!(matches!(error, PipelineError::OutputExists(_)));
    assert_eq!(fs::read(output.join("keep.txt")).unwrap(), b"keep");
}

fn build_pack(root: &Path, png: &[u8]) {
    fs::create_dir_all(root).unwrap();
    stage(root, "meshes/triangle.json", MESH.as_bytes());
    stage(root, "textures/ui.png", png);
    stage(root, "audio/sound.ogg", b"OggSfixture");
    stage(root, "fonts/ui.ttf", b"\x00\x01\x00\x00fixture");
    stage(
        root,
        "fonts/font-index.json",
        br#"{"schema":"ffone.font-index.v1","fonts":[{"path":"fonts/ui.ttf"}]}"#,
    );
    stage(
        root,
        "tables/game.json",
        br#"{"schema":"ffone.table.v1","value":7}"#,
    );

    let provenance = Provenance::new(
        "native-test",
        vec![SourceFingerprint::new("fixture", "a".repeat(64))],
    );
    let mut builder = ContentPackBuilder::new("ru-RU", provenance);
    builder
        .register("meshes/triangle.json", ContentKind::Mesh)
        .unwrap()
        .register("textures/ui.png", ContentKind::Texture)
        .unwrap()
        .register("audio/sound.ogg", ContentKind::Audio)
        .unwrap()
        .register("fonts/ui.ttf", ContentKind::Font)
        .unwrap()
        .register("fonts/font-index.json", ContentKind::Font)
        .unwrap()
        .register("tables/game.json", ContentKind::Table)
        .unwrap();
    builder.write(root).unwrap();
}

fn stage(root: &Path, relative: &str, bytes: &[u8]) {
    let path = relative
        .split('/')
        .fold(root.to_path_buf(), |path, part| path.join(part));
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, bytes).unwrap();
}
