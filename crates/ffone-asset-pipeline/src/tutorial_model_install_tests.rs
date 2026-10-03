use std::{io::Cursor, path::Path};

use ffone_skinned_model::{
    GpuAnimationEvidence, GpuModelIdentity, GpuRuntimeEvidence, GpuScreenshotEvidence,
};
use image::{DynamicImage, ImageFormat, Rgb, RgbImage};
use serde_json::json;
use tempfile::TempDir;

use super::*;
use crate::{ModelFeatureCounts, SourcePackIdentity};

const MODEL: &str = "models/mob/npc_test/npc_test.glb";
const PUBLISH: &str = "models/mob/npc_test/npc_test.publish.json";
const TEXTURE: &str = "models/mob/npc_test/npc_test.textures/test.png";
const EVIDENCE_JSON: &str = "models/mob/npc_test/npc_test.gpu.json";
const EVIDENCE_PNG: &str = "models/mob/npc_test/npc_test.gpu.png";

struct Fixture {
    _temp: TempDir,
    candidate: PathBuf,
    evidence: PathBuf,
    assets: PathBuf,
}

impl Fixture {
    fn options(&self) -> TutorialModelInstallOptions {
        TutorialModelInstallOptions::new(&self.candidate, &self.assets, "fixture-build")
            .with_evidence_root(&self.evidence)
            .with_model(MODEL)
    }
}

fn fixture() -> Fixture {
    let temp = tempfile::tempdir().unwrap();
    let candidate = temp.path().join("candidate");
    let evidence = temp.path().join("evidence");
    let assets = temp.path().join("assets");
    fs::create_dir_all(&candidate).unwrap();
    fs::create_dir_all(&evidence).unwrap();
    fs::create_dir_all(assets.join("characters/npc")).unwrap();

    let texture = png_bytes();
    write(&candidate, TEXTURE, &texture);
    let glb = fixture_glb();
    write(&candidate, MODEL, &glb);

    let mut counts = ModelFeatureCounts::default();
    counts.nodes = 2;
    counts.mesh_parts = 1;
    counts.material_slots = 1;
    let contract = ModelPublishContract {
        schema: crate::MODEL_PUBLISH_SCHEMA.to_owned(),
        legacy_name: "npc_test".to_owned(),
        root_node: "npc_test".to_owned(),
        family: "mob".to_owned(),
        semantic_directories: vec!["npc_test".to_owned()],
        output_glb: MODEL.to_owned(),
        glb_blake3: blake3::hash(&glb).to_hex().to_string(),
        source: counts.clone(),
        published: counts,
        unresolved_source_features: Vec::new(),
    };
    let semantic_hash = sha256(b"semantic-scope");
    let publish = json!({
        "schema": LOGICAL_MODEL_PUBLISH_REPORT_SCHEMA,
        "status": "staged-incomplete",
        "publishable": false,
        "contract": contract,
        "semanticProof": {
            "status": "native-model-matches-redecoded-glb",
            "matched": true,
            "source": { "hierarchySha256": semantic_hash.clone() },
            "emitted": { "hierarchySha256": semantic_hash }
        },
        "materialPublish": {
            "status": "native-data-complete-runtime-validation-pending",
            "textures": [{
                "uri": "npc_test.textures/test.png",
                "byteLength": texture.len(),
                "sha256": sha256(&texture),
                "sourceMipCount": 1,
                "mipLevels": [{
                    "uri": "npc_test.textures/test.png",
                    "pngByteLength": texture.len(),
                    "pngSha256": sha256(&texture)
                }]
            }]
        },
        "reportPath": PUBLISH
    });
    write_json(&candidate, PUBLISH, &publish);

    let batch = json!({
        "schema": LOGICAL_MODEL_BATCH_REPORT_SCHEMA,
        "status": "complete-structural-audit-passed-gpu-pending",
        "structuralAuditPassed": true,
        "coordinateStatus": "artifact-space-proven-runtime-pending",
        "coordinateArtifactSpaceProven": 1,
        "coordinateRuntimeSpawnPolicyPending": 1,
        "coordinateMismatches": 0,
        "skinningBasisParityMaxError": null,
        "currentPoseBindIdentityDeviationMax": null,
        "gpuGatePending": true,
        "candidatePublishable": false,
        "counts": {
            "plannedSources": 1,
            "sources": 1,
            "blockedSources": 0,
            "families": 1,
            "semanticDirectories": 1,
            "files": 3,
            "glbs": 1,
            "pngs": 1,
            "publishReports": 1
        },
        "models": [{
            "source": "mob/npc_test/npc_test.source.json",
            "sourceSha256": sha256(b"offline-source-document"),
            "family": "mob",
            "semanticDirectories": ["npc_test"],
            "logicalName": "npc_test",
            "outputGlb": MODEL,
            "coordinateStatus": "artifact-space-proven-runtime-pending",
            "runtimeSpawnPolicy": "pending",
            "skinningBasisParityStatus": "not-applicable-rigid",
            "skinningBasisParityMaxError": null,
            "currentPoseBindIdentityDeviationMax": null
        }],
        "blockers": []
    });
    write_json(&candidate, LOGICAL_MODEL_BATCH_REPORT_FILE, &batch);

    let facts = gpu_model_facts_from_glb(&glb).unwrap();
    let screenshot = png_bytes();
    let decoded = image::load_from_memory_with_format(&screenshot, ImageFormat::Png)
        .unwrap()
        .to_rgb8();
    let foreground =
        screenshot_foreground(decoded.as_raw(), decoded.width(), decoded.height()).unwrap();
    let evidence_document = LogicalModelGpuEvidence {
        schema: GPU_EVIDENCE_SCHEMA.to_owned(),
        status: AutomatedGpuStatus::Passed,
        render_profile: GPU_RENDER_PROFILE.to_owned(),
        visual_parity: VisualParityClaim::NotAsserted,
        model: GpuModelIdentity {
            relative_glb: MODEL.to_owned(),
            true_name: facts.true_name.clone(),
            glb_byte_length: glb.len() as u64,
            glb_sha256: sha256(&glb),
        },
        animation: GpuAnimationEvidence {
            standard_clips_loaded: 0,
            selected_exact_name: None,
            sample_normalized_ppm: None,
            animation_players: 0,
            sampled_players: 0,
        },
        runtime: GpuRuntimeEvidence {
            scene_ready: true,
            outline_mode: SourceOutlineMode::Source,
            mesh_parts: facts.mesh_parts,
            skinned_mesh_parts: facts.skinned_mesh_parts,
            skin_joint_references: facts.skin_joint_references,
            resolved_skin_joint_references: facts.skin_joint_references,
            inverse_bind_matrices: facts.inverse_bind_matrices,
            materials_applied: facts.materials_applied,
            legacy_pass_companions: facts.legacy_pass_companions,
            outline_pass_companions: facts.outline_pass_companions,
            assigned_texture_bindings: facts.assigned_texture_bindings,
            exact_mip_markers: facts.exact_mip_markers,
            exact_mip_chains: facts.exact_mip_chains,
            exact_mip_levels: facts.exact_mip_levels,
            material_errors: 0,
            shader_errors: 0,
        },
        screenshot: GpuScreenshotEvidence {
            relative_png: EVIDENCE_PNG.to_owned(),
            byte_length: screenshot.len() as u64,
            sha256: sha256(&screenshot),
            width: decoded.width(),
            height: decoded.height(),
            foreground_pixels: foreground,
        },
    };
    write_json(
        &evidence,
        EVIDENCE_JSON,
        &serde_json::to_value(evidence_document).unwrap(),
    );
    write(&evidence, EVIDENCE_PNG, &screenshot);

    let existing = b"user-owned-character".to_vec();
    write(&assets, "characters/npc/existing.glb", &existing);
    let manifest = ProjectAssetManifest {
        schema: PROJECT_ASSET_SCHEMA.to_owned(),
        protocol: 1,
        locale: "en".to_owned(),
        source_pack: SourcePackIdentity {
            schema: "fixture.source-pack.v1".to_owned(),
            manifest_blake3: "fixture".to_owned(),
        },
        files: vec![ProjectAssetFile {
            source_path: "user/characters/npc/existing.glb".to_owned(),
            path: "characters/npc/existing.glb".to_owned(),
            kind: ProjectAssetKind::Model,
            bytes: existing.len() as u64,
            blake3: blake3::hash(&existing).to_hex().to_string(),
        }],
    };
    write_json(
        &assets,
        ASSET_MANIFEST_FILE,
        &serde_json::to_value(manifest).unwrap(),
    );

    Fixture {
        _temp: temp,
        candidate,
        evidence,
        assets,
    }
}

fn fixture_glb() -> Vec<u8> {
    let document = json!({
        "asset": { "version": "2.0" },
        "scene": 0,
        "scenes": [{ "name": "npc_test", "nodes": [0] }],
        "nodes": [
            { "name": "npc_test", "children": [1] },
            { "name": "mesh", "mesh": 0 }
        ],
        "meshes": [{ "primitives": [{ "attributes": {}, "material": 0 }] }],
        "accessors": [],
        "materials": [{
            "extras": { "ffone": {
                "passes": [{ "outline": { "mode": "disabled" } }],
                "textureBindings": [{
                    "texture": 0,
                    "uri": "npc_test.textures/test.png",
                    "mipProvenance": { "publishedPolicy": "baseLevelOnly" },
                    "mipLevels": [{ "uri": "npc_test.textures/test.png" }]
                }]
            }}
        }],
        "images": [{ "uri": "npc_test.textures/test.png" }],
        "textures": [{ "source": 0, "extras": { "ffone": {
            "uri": "npc_test.textures/test.png",
            "mipLevels": [{ "uri": "npc_test.textures/test.png" }]
        }}}],
        "extras": { "logicalModelName": "npc_test" }
    });
    let mut json_bytes = serde_json::to_vec(&document).unwrap();
    while json_bytes.len() % 4 != 0 {
        json_bytes.push(b' ');
    }
    let total = 20 + json_bytes.len() + 8;
    let mut glb = Vec::with_capacity(total);
    glb.extend_from_slice(b"glTF");
    glb.extend_from_slice(&2_u32.to_le_bytes());
    glb.extend_from_slice(&(total as u32).to_le_bytes());
    glb.extend_from_slice(&(json_bytes.len() as u32).to_le_bytes());
    glb.extend_from_slice(&0x4e4f_534a_u32.to_le_bytes());
    glb.extend_from_slice(&json_bytes);
    glb.extend_from_slice(&0_u32.to_le_bytes());
    glb.extend_from_slice(&0x004e_4942_u32.to_le_bytes());
    glb
}

fn png_bytes() -> Vec<u8> {
    let mut image = RgbImage::from_pixel(32, 32, Rgb([12, 14, 20]));
    for y in 6..26 {
        for x in 6..26 {
            image.put_pixel(x, y, Rgb([220, 80, 40]));
        }
    }
    let mut output = Cursor::new(Vec::new());
    DynamicImage::ImageRgb8(image)
        .write_to(&mut output, ImageFormat::Png)
        .unwrap();
    output.into_inner()
}

fn write(root: &Path, relative: &str, bytes: &[u8]) {
    let path = relative
        .split('/')
        .fold(root.to_path_buf(), |path, component| path.join(component));
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, bytes).unwrap();
}

fn write_json(root: &Path, relative: &str, value: &Value) {
    let mut bytes = serde_json::to_vec_pretty(value).unwrap();
    bytes.push(b'\n');
    write(root, relative, &bytes);
}

fn snapshot(root: &Path) -> BTreeMap<String, Vec<u8>> {
    index_regular_tree(root)
        .unwrap()
        .into_values()
        .map(|relative| {
            let bytes = read_regular_relative(root, &relative).unwrap();
            (relative, bytes)
        })
        .collect()
}

#[test]
fn selective_install_preserves_unrelated_character_entry() {
    let fixture = fixture();
    let before = fs::read(fixture.assets.join("characters/npc/existing.glb")).unwrap();
    let report = install_tutorial_models(&fixture.options()).unwrap();
    assert_eq!(report.installed_models, 1);
    assert_eq!(report.installed_files, 4);
    assert!(!report.replaced_previous_install);
    assert_eq!(
        fs::read(fixture.assets.join("characters/npc/existing.glb")).unwrap(),
        before
    );
    assert!(
        fixture
            .assets
            .join("tutorial/models/mob/npc_test/npc_test.glb")
            .is_file()
    );
    let manifest: ProjectAssetManifest =
        serde_json::from_slice(&fs::read(fixture.assets.join(ASSET_MANIFEST_FILE)).unwrap())
            .unwrap();
    assert!(manifest.files.iter().any(|entry| {
        entry.path == "characters/npc/existing.glb"
            && entry.source_path == "user/characters/npc/existing.glb"
    }));
    assert_eq!(
        manifest
            .files
            .iter()
            .filter(|entry| owned_path(&entry.path))
            .count(),
        4
    );
}

#[test]
fn bad_gpu_hash_is_rejected_atomically() {
    let fixture = fixture();
    let path = fixture.evidence.join(EVIDENCE_JSON);
    let mut value: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    value["screenshot"]["sha256"] = Value::String("0".repeat(64));
    write_json(&fixture.evidence, EVIDENCE_JSON, &value);
    let before = snapshot(&fixture.assets);
    let error = install_tutorial_models(&fixture.options()).unwrap_err();
    assert!(
        error
            .to_string()
            .contains("GPU screenshot hash/length/path")
    );
    assert_eq!(snapshot(&fixture.assets), before);
}

#[test]
fn bad_publish_hash_is_rejected_atomically() {
    let fixture = fixture();
    let path = fixture.candidate.join(PUBLISH);
    let mut value: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    value["materialPublish"]["textures"][0]["sha256"] = Value::String("0".repeat(64));
    write_json(&fixture.candidate, PUBLISH, &value);
    let before = snapshot(&fixture.assets);
    let error = install_tutorial_models(&fixture.options()).unwrap_err();
    assert!(error.to_string().contains("conflicting hash evidence"));
    assert_eq!(snapshot(&fixture.assets), before);
}

#[test]
fn repeat_install_requires_full_manifest_disk_agreement() {
    let fixture = fixture();
    install_tutorial_models(&fixture.options()).unwrap();
    let installed = fixture
        .assets
        .join("tutorial/models/mob/npc_test/npc_test.glb");
    fs::write(&installed, b"tampered").unwrap();
    let before = snapshot(&fixture.assets);
    let error = install_tutorial_models(&fixture.options()).unwrap_err();
    assert!(
        error
            .to_string()
            .contains("existing tutorial-model file disagrees with manifest")
    );
    assert_eq!(snapshot(&fixture.assets), before);
}

#[test]
fn catalogless_native_installer_tree_is_migrated_after_full_manifest_proof() {
    let fixture = fixture();
    install_tutorial_models(&fixture.options()).unwrap();
    fs::remove_file(fixture.assets.join(TUTORIAL_MODEL_CATALOG_PATH)).unwrap();
    let manifest_path = fixture.assets.join(ASSET_MANIFEST_FILE);
    let mut manifest: ProjectAssetManifest =
        serde_json::from_slice(&fs::read(&manifest_path).unwrap()).unwrap();
    manifest
        .files
        .retain(|entry| entry.path != TUTORIAL_MODEL_CATALOG_PATH);
    fs::write(
        &manifest_path,
        serde_json::to_vec_pretty(&manifest).unwrap(),
    )
    .unwrap();

    let report = install_tutorial_models(&fixture.options()).unwrap();
    assert!(report.replaced_previous_install);
    assert!(fixture.assets.join(TUTORIAL_MODEL_CATALOG_PATH).is_file());
    let migrated: TutorialModelCatalog = serde_json::from_slice(
        &fs::read(fixture.assets.join(TUTORIAL_MODEL_CATALOG_PATH)).unwrap(),
    )
    .unwrap();
    assert_eq!(migrated.schema, TUTORIAL_MODEL_CATALOG_SCHEMA);
    assert_eq!(migrated.installer, INSTALLER_ID);
}

#[test]
fn catalogless_tree_with_foreign_manifest_ownership_is_rejected_atomically() {
    let fixture = fixture();
    install_tutorial_models(&fixture.options()).unwrap();
    fs::remove_file(fixture.assets.join(TUTORIAL_MODEL_CATALOG_PATH)).unwrap();
    let manifest_path = fixture.assets.join(ASSET_MANIFEST_FILE);
    let mut manifest: ProjectAssetManifest =
        serde_json::from_slice(&fs::read(&manifest_path).unwrap()).unwrap();
    manifest
        .files
        .retain(|entry| entry.path != TUTORIAL_MODEL_CATALOG_PATH);
    manifest
        .files
        .iter_mut()
        .find(|entry| entry.path.ends_with("/npc_test.glb"))
        .unwrap()
        .source_path = "foreign/tutorial/npc_test.glb".to_owned();
    fs::write(
        &manifest_path,
        serde_json::to_vec_pretty(&manifest).unwrap(),
    )
    .unwrap();
    let before = snapshot(&fixture.assets);

    let error = install_tutorial_models(&fixture.options()).unwrap_err();
    assert!(error.to_string().contains("foreign ownership"));
    assert_eq!(snapshot(&fixture.assets), before);
}

#[test]
fn duplicate_evidence_pair_across_roots_is_rejected() {
    let fixture = fixture();
    let second = fixture._temp.path().join("evidence-duplicate");
    fs::create_dir_all(&second).unwrap();
    write(
        &second,
        EVIDENCE_JSON,
        &fs::read(fixture.evidence.join(EVIDENCE_JSON)).unwrap(),
    );
    write(
        &second,
        EVIDENCE_PNG,
        &fs::read(fixture.evidence.join(EVIDENCE_PNG)).unwrap(),
    );
    let before = snapshot(&fixture.assets);
    let error = install_tutorial_models(&fixture.options().with_evidence_root(second)).unwrap_err();
    assert!(error.to_string().contains("exactly one GPU JSON+PNG pair"));
    assert_eq!(snapshot(&fixture.assets), before);
}

#[test]
fn duplicate_selection_is_rejected_before_write() {
    let fixture = fixture();
    let before = snapshot(&fixture.assets);
    let error = install_tutorial_models(&fixture.options().with_model(MODEL)).unwrap_err();
    assert!(
        error
            .to_string()
            .contains("duplicate tutorial model selection")
    );
    assert_eq!(snapshot(&fixture.assets), before);
}
