use std::{
    env,
    sync::atomic::{AtomicU64, Ordering},
};

use super::*;

static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(0);

struct FixtureRoot(PathBuf);

impl FixtureRoot {
    fn new(label: &str) -> Self {
        let sequence = NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed);
        let path = env::temp_dir().join(format!(
            "ffone-cooker-{label}-{}-{sequence}",
            std::process::id()
        ));
        cleanup_created(&path);
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}

impl Drop for FixtureRoot {
    fn drop(&mut self) {
        cleanup_created(&self.0);
    }
}

fn fixture_ogg() -> Vec<u8> {
    let mut page = vec![0_u8; 29];
    page[..4].copy_from_slice(b"OggS");
    page[5] = 2;
    page[14] = 1;
    page[26] = 1;
    page[27] = 1;
    page[28] = 0x7f;
    let checksum = ogg_page_crc(&page);
    page[22..26].copy_from_slice(&checksum.to_le_bytes());
    page
}

#[test]
fn cooks_native_overlay_pack_and_keeps_legacy_report_outside() {
    let fixture = FixtureRoot::new("native-overlay");
    let project = fixture.0.join("project.ffclient");
    let build = fixture.0.join("build");
    let output = fixture.0.join("native-pack");
    fs::create_dir_all(project.join("audio/voices")).unwrap();
    fs::create_dir_all(project.join("fonts")).unwrap();
    fs::create_dir_all(project.join("translations")).unwrap();
    fs::create_dir_all(project.join("tabledata")).unwrap();
    fs::create_dir_all(&build).unwrap();

    fs::write(
        project.join("ffpatch.json"),
        br#"{"FontDir":"fonts","TableDataDir":"tabledata","TranslationJson":"translations/translation.index.json"}"#,
    )
    .unwrap();
    let original_ogg = fixture_ogg();
    fs::write(project.join("audio/voices/hello.ogg"), &original_ogg).unwrap();
    fs::write(
        project.join("audio/manifest.json"),
        br#"{"format":"fftools.audio-patch.v1","entries":[{"asset":"AudioClip","bytes":1,"container":"Legacy.resourceFile","file":"voices/hello.ogg","name":"Russian Voice","path_id":9}]}"#,
    )
    .unwrap();
    fs::write(project.join("fonts/test.ttf"), b"\0\x01\0\0fixture-font").unwrap();
    fs::write(project.join("fonts/alias.ttf"), b"\0\x01\0\0fixture-font").unwrap();
    fs::write(
        project.join("fonts/manifest.json"),
        r#"{"format":"fftools.font-patch.v1","entries":[{"family":"\u0428\u0440\u0438\u0444\u0442 \u0410\u043b\u0438\u0430\u0441 \u0410","fontFile":"fonts/test.ttf","includeRussian":true,"replaceAscii":false,"verticalOffset":0.5,"target":{"asset":"CAB-font","name":"Legacy Font A","pathId":7}},{"family":"\u0428\u0440\u0438\u0444\u0442 \u0410\u043b\u0438\u0430\u0441 \u0411","fontFile":"fonts/alias.ttf","includeRussian":true,"replaceAscii":false,"verticalOffset":0.5,"target":{"asset":"CAB-font","name":"Legacy Font B","pathId":8}}]}"#
            .as_bytes(),
    )
    .unwrap();
    fs::write(
        project.join("translations/translation.index.json"),
        r#"{"format":"fftools.translation.v1","entries":[{"id":"legacy:Legacy.resourceFile:7:a","source":"Hello","translation":"\u041f\u0440\u0438\u0432\u0435\u0442"},{"id":"legacy:Legacy.resourceFile:7:b","source":"Hello","translation":"\u0417\u0434\u0440\u0430\u0432\u0441\u0442\u0432\u0443\u0439\u0442\u0435"},{"id":"legacy:Legacy.resourceFile:7:fallback","source":"Fallback","translation":""}]}"#
            .as_bytes(),
    )
    .unwrap();
    fs::write(
        project.join("tabledata/manifest.json"),
        br#"{"format":"fftools.tabledata-patch.v1","entries":[{"container":"Legacy.resourceFile","file":"patched.json","name":"xdtdatas","pathId":7}]}"#,
    )
    .unwrap();
    fs::write(
        project.join("tabledata/patched.json"),
        br#"{"format":"fftools.tabledata-object.v1","sourceBundle":"Legacy.resourceFile","pathId":7,"value":{"m_Name":"xdtdatas","m_Script":{"__unityType":"pointer","fileId":1,"pathId":2},"m_Table":{"absolutePosix":"/tmp/legacy","absoluteUnc":"\\\\server\\share\\legacy","absoluteWindows":"D:/legacy/content","answer":42,"legacy":"Legacy.resourceFile","slots":[1,{"__unityType":"pointer","fileId":1,"pathId":2},3,"model.nif"]}}}"#,
    )
    .unwrap();

    let main = b"not-a-unity-bundle-main";
    let bundle = b"not-a-unity-bundle-resource";
    fs::write(build.join("main.unity3d"), main).unwrap();
    fs::write(build.join("Legacy.resourceFile"), bundle).unwrap();
    let uuid = "11111111-2222-3333-4444-555555555555";
    let launcher = json!({
        "uuid": uuid,
        "main_file_info": { "hash": sha256_hex(main), "size": main.len() },
        "bundles": {
            "Legacy.resourceFile": {
                "compressed_info": { "hash": sha256_hex(bundle), "size": bundle.len() }
            }
        }
    });
    fs::write(
        build.join(format!("{uuid}.json")),
        json_bytes(&launcher).unwrap(),
    )
    .unwrap();

    cook_ffone_pack(&project, &build, &output, "ru-RU").unwrap();
    let manifest = validate_pack(&output).unwrap();
    assert!(manifest
        .files
        .iter()
        .any(|file| file.kind == ContentKind::Audio));
    assert!(manifest
        .files
        .iter()
        .any(|file| file.kind == ContentKind::Font));
    assert!(manifest
        .files
        .iter()
        .any(|file| file.kind == ContentKind::Localization));
    assert!(manifest
        .files
        .iter()
        .any(|file| file.kind == ContentKind::Table));
    assert!(manifest.files.iter().all(|file| file.path.is_ascii()));
    assert!(manifest
        .files
        .iter()
        .all(|file| !contains_legacy_text(&file.path)));

    for file in &manifest.files {
        let bytes = fs::read(output.join(&file.path)).unwrap();
        if file.path.ends_with(".json") {
            let value: JsonValue = serde_json::from_slice(&bytes).unwrap();
            reject_forbidden_json(&value).unwrap();
        }
    }
    let table_path = manifest
        .files
        .iter()
        .find(|file| file.kind == ContentKind::Table && file.path.starts_with("tables/"))
        .unwrap();
    let table: JsonValue =
        serde_json::from_slice(&fs::read(output.join(&table_path.path)).unwrap()).unwrap();
    assert_eq!(table["tables"][0]["value"]["m_Table"]["answer"], 42);
    assert!(table["tables"][0]["value"]["m_Table"]
        .get("legacy")
        .is_none());
    for key in ["absoluteWindows", "absoluteUnc", "absolutePosix"] {
        assert!(table["tables"][0]["value"]["m_Table"].get(key).is_none());
    }
    assert_eq!(
        table["tables"][0]["value"]["m_Table"]["slots"],
        json!([1, null, 3, null])
    );
    assert!(table["tables"][0]["value"].get("m_Script").is_none());

    let localization_path = manifest
        .files
        .iter()
        .find(|file| file.kind == ContentKind::Localization)
        .unwrap();
    let localization: JsonValue =
        serde_json::from_slice(&fs::read(output.join(&localization_path.path)).unwrap())
            .unwrap();
    assert!(localization["entries"]
        .as_array()
        .unwrap()
        .iter()
        .any(|entry| entry["source"] == "Hello"));
    assert_eq!(localization["entries"].as_array().unwrap().len(), 3);
    assert!(localization["entries"]
        .as_array()
        .unwrap()
        .iter()
        .any(|entry| { entry["source"] == "Fallback" && entry["value"] == "Fallback" }));
    let keys = localization["entries"]
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| entry["key"].as_str().unwrap())
        .collect::<BTreeSet<_>>();
    assert!(keys.contains(
        stable_key(
            "ffone.localization.v1",
            &[b"legacy:Legacy.resourceFile:7:a"]
        )
        .as_str()
    ));
    assert!(keys.contains(
        stable_key(
            "ffone.localization.v1",
            &[b"legacy:Legacy.resourceFile:7:b"]
        )
        .as_str()
    ));

    let catalog_path = manifest
        .files
        .iter()
        .find(|file| file.path.starts_with("catalog/content-index--"))
        .unwrap();
    let catalog: JsonValue =
        serde_json::from_slice(&fs::read(output.join(&catalog_path.path)).unwrap()).unwrap();
    assert_eq!(catalog["profile"], "core-v1");
    assert_eq!(catalog["complete"], false);
    assert_eq!(catalog["nativeOnly"], true);
    let font_assets = catalog["assets"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|asset| asset["kind"] == "font")
        .collect::<Vec<_>>();
    assert_eq!(font_assets.len(), 2);
    let font_names = font_assets
        .iter()
        .map(|asset| asset["name"].as_str().unwrap())
        .collect::<BTreeSet<_>>();
    assert_eq!(
        font_names,
        BTreeSet::from(["Шрифт Алиас А", "Шрифт Алиас Б"])
    );
    assert_eq!(
        font_assets
            .iter()
            .map(|asset| asset["path"].as_str().unwrap())
            .collect::<BTreeSet<_>>()
            .len(),
        1
    );

    let report_path = cook_report_path(&output).unwrap();
    assert!(report_path.is_file());
    assert!(!output.join("reports").exists());
    let report_text = fs::read_to_string(report_path).unwrap();
    assert!(report_text.contains("Legacy.resourceFile"));
    assert!(!report_text.contains(&fixture.0.to_string_lossy().to_string()));
    let report: JsonValue = serde_json::from_str(&report_text).unwrap();
    assert_eq!(report["coverage"]["audioSizeMismatches"], 1);
    assert_eq!(report["coverage"]["localizationFallbacks"], 1);
    assert_eq!(report["mappings"].as_array().unwrap().len(), 7);

    for forbidden in [
        json!({"locator": "\\\\server\\share\\legacy"}),
        json!({"locator": "/tmp/legacy"}),
        json!({"locator": "D:/legacy/content"}),
        json!({"locator": "model.nif"}),
        json!({"pathId": 7}),
    ] {
        assert!(reject_forbidden_json(&forbidden).is_err());
    }

    let first_overlay_digest = manifest
        .provenance
        .sources
        .iter()
        .find(|source| source.label == "authoring-overlay")
        .unwrap()
        .blake3
        .clone();
    let mut changed_ogg = original_ogg;
    changed_ogg[28] ^= 1;
    changed_ogg[22..26].fill(0);
    let changed_crc = ogg_page_crc(&changed_ogg);
    changed_ogg[22..26].copy_from_slice(&changed_crc.to_le_bytes());
    assert!(is_valid_ogg(&changed_ogg));
    fs::write(project.join("audio/voices/hello.ogg"), changed_ogg).unwrap();
    let second_output = fixture.0.join("native-pack-2");
    cook_ffone_pack(&project, &build, &second_output, "ru-RU").unwrap();
    let second_manifest = validate_pack(&second_output).unwrap();
    let second_overlay_digest = second_manifest
        .provenance
        .sources
        .iter()
        .find(|source| source.label == "authoring-overlay")
        .unwrap()
        .blake3
        .clone();
    assert_ne!(first_overlay_digest, second_overlay_digest);
}

#[test]
fn directory_manifest_resolution_requires_exactly_one_uuid_json() {
    let fixture = FixtureRoot::new("manifest-count");
    fs::write(
        fixture.0.join("11111111-2222-3333-4444-555555555555.json"),
        b"{}",
    )
    .unwrap();
    fs::write(
        fixture.0.join("aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee.json"),
        b"{}",
    )
    .unwrap();
    let error = resolve_launcher_manifest(&fixture.0).unwrap_err();
    assert!(error.contains("exactly one UUID-named"));
}

#[test]
fn raw_pack_needs_no_project_and_does_not_read_source_overlays() {
    let fixture = FixtureRoot::new("raw-source");
    let build = fixture.0.join("raw");
    fs::create_dir_all(&build).unwrap();
    let main = b"not-a-unity-bundle";
    fs::write(build.join("main.unity3d"), main).unwrap();
    let uuid = "11111111-2222-3333-4444-555555555555";
    fs::write(
        build.join(format!("{uuid}.json")),
        json_bytes(&json!({
            "uuid": uuid,
            "main_file_info": {"hash": sha256_hex(main), "size": main.len()},
            "bundles": {}
        }))
        .unwrap(),
    )
    .unwrap();
    let output = fixture.0.join("native");
    cook_pack(None, &build, &output, "en-US").unwrap();
    validate_pack(&output).unwrap();
    assert!(!build.join("ffpatch.json").exists());
    // Stray patch configuration in a source must not influence raw export.
    fs::write(build.join("ffpatch.json"), b"invalid ignored json").unwrap();
    let second = fixture.0.join("native-second");
    cook_pack(None, &build, &second, "en-US").unwrap();
    let report: JsonValue = read_json_value(&cook_report_path(&second).unwrap()).unwrap();
    assert!(report["overlayErrors"].as_array().unwrap().is_empty());
    assert_eq!(report["complete"], false); // Bad bundle stays an explicit failure in the report.
    assert_eq!(fs::read_dir(&build).unwrap().count(), 3);
}

#[test]
fn inventory_hash_mismatch_is_fatal_before_output_creation() {
    let fixture = FixtureRoot::new("hash-mismatch");
    let project = fixture.0.join("project.ffclient");
    let build = fixture.0.join("build");
    let output = fixture.0.join("native-pack");
    fs::create_dir_all(&project).unwrap();
    fs::create_dir_all(&build).unwrap();
    fs::write(project.join("ffpatch.json"), b"{}").unwrap();
    fs::write(build.join("main.unity3d"), b"main").unwrap();
    let uuid = "11111111-2222-3333-4444-555555555555";
    let launcher = json!({
        "uuid": uuid,
        "main_file_info": { "hash": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", "size": 4 },
        "bundles": {}
    });
    fs::write(
        build.join(format!("{uuid}.json")),
        json_bytes(&launcher).unwrap(),
    )
    .unwrap();
    let error = cook_ffone_pack(&project, &build, &output, "ru-RU").unwrap_err();
    assert!(error.contains("SHA-256 mismatch"));
    assert!(!output.exists());
    assert!(!cook_report_path(&output).unwrap().exists());
}
