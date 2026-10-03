use super::*;
use tempfile::tempdir;

#[test]
fn verifies_a_published_file_without_creating_global_metadata() {
    let root = tempdir().unwrap();
    fs::create_dir(root.path().join("data")).unwrap();
    fs::write(root.path().join("data/catalog.json"), b"{\"v\":2}\n").unwrap();

    let published =
        refresh_project_asset_manifest_entry(root.path(), "data/catalog.json").unwrap();
    assert_eq!(published.bytes, 8);
    assert_eq!(
        published.blake3,
        blake3::hash(b"{\"v\":2}\n").to_hex().to_string()
    );
    assert!(!root.path().join("asset-manifest.json").exists());
}

#[test]
fn rejects_missing_and_unsafe_paths() {
    let root = tempdir().unwrap();
    assert!(refresh_project_asset_manifest_entry(root.path(), "../known.json").is_err());
    assert!(refresh_project_asset_manifest_entry(root.path(), "unknown.json").is_err());
}
