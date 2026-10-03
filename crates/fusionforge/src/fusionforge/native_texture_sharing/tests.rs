use super::*;
fn model(root: &Path, name: &str, wrap: &str, lower: &[u8]) -> String {
    let owner = format!("characters/npcs/{name}/{name}.glb");
    let base = format!("characters/npcs/{name}/{name}.textures/palette.png");
    let mip = format!("characters/npcs/{name}/{name}.textures/palette.mips/mip-01.png");
    publication::write(&root.join(&base), b"exact base fixture").unwrap();
    publication::write(&root.join(&mip), lower).unwrap();
    let uri = format!("{name}.textures/palette.png");
    let d = json!({"images":[{"uri":uri}],"materials":[{"extras":{"ffone":{"textureBindings":[{"uri":uri,"colorSpace":"srgb","sampler":{"descriptor":{"name":name,"wrapS":wrap,"wrapT":wrap,"magFilter":"linear"}},"mipLevels":[{"uri":uri,"width":2,"height":2},{"uri":format!("{name}.textures/palette.mips/mip-01.png"),"width":1,"height":1}]}]}}}]});
    let mut j = serde_json::to_vec(&d).unwrap();
    j.resize(j.len().next_multiple_of(4), b' ');
    let mut bytes = b"glTF".to_vec();
    for n in [2, 32 + j.len() as u32, j.len() as u32, 0x4e4f534a] {
        bytes.extend(n.to_le_bytes());
    }
    bytes.extend(j);
    bytes.extend(4u32.to_le_bytes());
    bytes.extend(0x004e4942u32.to_le_bytes());
    bytes.extend(b"BIN!");
    publication::write(&root.join(owner), &bytes).unwrap();
    base
}
#[test]
fn identical_chains_apply_and_repeat_without_binary_changes() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("assets/game");
    fs::create_dir_all(&root).unwrap();
    let a = model(&root, "alpha", "repeat", b"lower");
    let b = model(&root, "beta", "repeat", b"lower");
    let work = tmp.path().join("work");
    run(&root, &work, true).unwrap();
    assert!(!root.join(a).exists() && !root.join(b).exists());
    assert_eq!(
        fs::read(root.join("characters/npcs/shared/textures/palette.png")).unwrap(),
        b"exact base fixture"
    );
    for name in ["alpha", "beta"] {
        let raw = fs::read(root.join(format!("characters/npcs/{name}/{name}.glb"))).unwrap();
        let (_, suffix) = decode(&raw, true).unwrap();
        assert_eq!(&suffix[8..], b"BIN!");
    }
    run(&root, &tmp.path().join("repeat"), false).unwrap();
    assert_eq!(
        publication::read_json(&tmp.path().join("repeat/plan.json")).unwrap()["deletions"],
        json!([])
    );
}
#[test]
fn sampler_and_lower_levels_prevent_sharing() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    model(root, "alpha", "repeat", b"one");
    model(root, "beta", "clamp", b"one");
    model(root, "gamma", "repeat", b"two");
    let mut routes = vec![];
    files(root, root, &mut routes).unwrap();
    assert!(plan(root, &routes, "").unwrap().redirects.is_empty());
}
#[test]
fn native_code_literal_blocks_whole_group() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    let a = model(root, "alpha", "repeat", b"one");
    model(root, "beta", "repeat", b"one");
    let mut routes = vec![];
    files(root, root, &mut routes).unwrap();
    assert!(plan(root, &routes, &a).unwrap().redirects.is_empty());
}
#[test]
fn changes_after_snapshot_are_rejected_without_deletion() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    let a = model(root, "alpha", "repeat", b"one");
    let b = model(root, "beta", "repeat", b"one");
    let before = publication::snapshot(root, &[a.clone(), b.clone()]).unwrap();
    fs::write(root.join(&a), b"owner edit").unwrap();
    assert!(
        publication::install_checked(root, &root.join("stage"), &[], true, &before).is_err()
    );
    assert!(root.join(a).exists() && root.join(b).exists());
}
#[test]
fn malformed_chunk_and_path_escape_are_rejected() {
    assert!(decode(b"glTF", true).is_err());
    assert_eq!(normalized("a/../../escape"), None);
    assert_eq!(normalized("a/../b"), Some("b".into()));
}
#[test]
fn failed_removal_restores_previously_moved_texture_files() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("assets/game");
    fs::create_dir_all(&root).unwrap();
    let a = model(&root, "alpha", "repeat", b"lower");
    let b = model(&root, "beta", "repeat", b"lower");
    let work = tmp.path().join("work");
    let blocked = work.join("removed/characters/npcs/beta");
    fs::create_dir_all(blocked.parent().unwrap()).unwrap();
    fs::write(&blocked, b"simulated storage failure").unwrap();
    assert!(run(&root, &work, true).is_err());
    for path in [a, b] {
        assert_eq!(fs::read(root.join(path)).unwrap(), b"exact base fixture");
    }
    for name in ["alpha", "beta"] {
        assert_eq!(
            fs::read(root.join(format!(
                "characters/npcs/{name}/{name}.textures/palette.mips/mip-01.png"
            )))
            .unwrap(),
            b"lower"
        );
    }
    assert!(
        !root
            .join("characters/npcs/shared/textures/palette.png")
            .exists()
    );
}
