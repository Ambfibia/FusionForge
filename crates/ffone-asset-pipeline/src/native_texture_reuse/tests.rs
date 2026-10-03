use super::*;
use tempfile::TempDir;

fn converted() -> ConvertedModel {
    let source: SourceDocument =
        serde_json::from_value(super::super::tests::fixture()).unwrap();
    convert_source(&source).unwrap()
}

fn indexed() -> (TempDir, PathBuf, PathBuf) {
    let temp = TempDir::new().unwrap();
    let root = temp.path().join("native");
    let source = temp.path().join("source.json");
    fs::write(
        &source,
        serde_json::to_vec(&super::super::tests::fixture()).unwrap(),
    )
    .unwrap();
    let options = LogicalModelPublishOptions::new(&source, "characters", &root)
        .with_semantic_directories(["npcs", "existing"])
        .with_semantic_root_layout();
    publish_logical_model(&options).unwrap();
    let index = temp.path().join("work/index.json");
    assert!(index_native_textures(&root, &index).unwrap() > 0);
    (temp, root, index)
}

#[test]
fn existing_texture_chain_is_reused_with_complete_staged_dependencies() {
    let (temp, _, index) = indexed();
    let source = temp.path().join("source.json");
    let out = temp.path().join("stage");
    let mut options = LogicalModelPublishOptions::new(&source, "characters", &out)
        .with_semantic_directories(["npcs", "new"])
        .with_semantic_root_layout();
    options.reuse_texture_index = Some(index);
    let report = publish_logical_model(&options).unwrap();
    assert!(
        report
            .material_publish
            .textures
            .iter()
            .all(|t| t.uri.contains("existing/"))
    );
    assert!(!out.join("characters/npcs/new/True Hero.textures").exists());
    for texture in &report.material_publish.textures {
        for mip in &texture.mip_levels {
            let path = normalize_relative(
                &Path::new(&report.contract.output_glb)
                    .parent()
                    .unwrap()
                    .join(&mip.uri),
            )
            .unwrap();
            let bytes = fs::read(out.join(path)).unwrap();
            assert_eq!(sha256_hex(&bytes), mip.png_sha256);
        }
    }
    let audit = crate::audit_logical_model_tree(&out).unwrap();
    assert!(audit.passed, "{:?}", audit.violations);
}

#[test]
fn sampling_color_and_lower_mip_differences_prevent_reuse() {
    let model = converted();
    let binding = &model.model.materials[0].texture_bindings[0];
    assert!(same_contract(binding, binding));
    let mut other = binding.clone();
    other.sampler.as_mut().unwrap().descriptor.mip_map_bias += 1.0;
    assert!(!same_contract(binding, &other));
    let mut other = binding.clone();
    other.color_space = if binding.color_space == TextureColorSpace::Srgb {
        TextureColorSpace::Linear
    } else {
        TextureColorSpace::Srgb
    };
    assert!(!same_contract(binding, &other));
    let mut other = binding.clone();
    other
        .mip_levels
        .as_mut()
        .unwrap()
        .last_mut()
        .unwrap()
        .png_sha256 = "different".into();
    assert!(!same_contract(binding, &other));
}

#[test]
fn changed_owner_requires_index_refresh_and_changed_pixels_are_not_reused() {
    let (_temp, root, index) = indexed();
    let mut doc: Index = serde_json::from_slice(&fs::read(&index).unwrap()).unwrap();
    let first = &doc.entries[0];
    let owner = root.join(&first.owner);
    fs::write(&owner, b"changed model").unwrap();
    let mut model = converted();
    assert!(
        reuse_textures(
            &index,
            Path::new("characters/npcs/new/True Hero.glb"),
            &mut model
        )
        .unwrap_err()
        .to_string()
        .contains("stale native texture index")
    );
    // Even matching metadata/hash candidates do not authorize merging
    // different payload bytes (a stale or deliberately altered index).
    for entry in &mut doc.entries {
        entry.owner_sha256 = sha256_hex(b"changed model");
    }
    fs::write(&index, serde_json::to_vec(&doc).unwrap()).unwrap();
    for entry in &doc.entries {
        for mip in entry.binding.mip_levels.as_ref().unwrap() {
            let path = root.join(
                normalize_relative(&entry.owner.parent().unwrap().join(&mip.uri)).unwrap(),
            );
            fs::write(path, b"changed PNG").unwrap();
        }
    }
    let mut model = converted();
    let uris: Vec<_> = model.model.textures.iter().map(|t| t.uri.clone()).collect();
    reuse_textures(
        &index,
        Path::new("characters/npcs/new/True Hero.glb"),
        &mut model,
    )
    .unwrap();
    assert_eq!(
        uris,
        model
            .model
            .textures
            .iter()
            .map(|t| t.uri.clone())
            .collect::<Vec<_>>()
    );
}

#[test]
fn references_cannot_escape_native_root() {
    assert!(normalize_relative(Path::new("../escape.png")).is_err());
    assert!(normalize_relative(Path::new("a/../../escape.png")).is_err());
    assert_eq!(
        normalize_relative(Path::new("a/../shared/p.png")).unwrap(),
        PathBuf::from("shared/p.png")
    );
}

#[test]
fn missing_installed_dependencies_are_reported_and_never_admitted() {
    let (_temp, root, index_path) = indexed();
    let original: Index = serde_json::from_slice(&fs::read(&index_path).unwrap()).unwrap();
    let missing = &original.entries[0];
    let level = &missing.binding.mip_levels.as_ref().unwrap()[0];
    let relative = normalize_relative(&missing.owner.parent().unwrap().join(&level.uri)).unwrap();
    fs::remove_file(root.join(relative)).unwrap();
    index_native_textures(&root, &index_path).unwrap();
    let refreshed: Index = serde_json::from_slice(&fs::read(&index_path).unwrap()).unwrap();
    assert!(!refreshed.excluded.is_empty());
    assert!(refreshed.entries.len() < original.entries.len());
    assert!(refreshed.entries.iter().all(|entry| entry.binding.uri != missing.binding.uri));
}

#[test]
fn identical_static_materials_share_without_dropping_renderer_slots() {
    let mut model = converted();
    model.model.animations.clear();
    let original = model.model.materials.len();
    let primitive_count: usize = model.model.meshes.iter().map(|m| m.primitives.len()).sum();
    model.model.materials.push(model.model.materials[0].clone());
    model
        .material_reports
        .push(model.material_reports[0].clone());
    model.model.meshes[0].primitives[0].material = Some(original as u32);
    reuse_identical_static_materials(&mut model).unwrap();
    assert_eq!(model.model.materials.len(), original);
    assert_eq!(model.model.meshes[0].primitives[0].material, Some(0));
    assert_eq!(
        primitive_count,
        model
            .model
            .meshes
            .iter()
            .map(|m| m.primitives.len())
            .sum::<usize>()
    );
    let mut distinct = model.model.materials[0].clone();
    distinct.render_queue += 1;
    model.model.materials.push(distinct);
    model
        .material_reports
        .push(model.material_reports[0].clone());
    reuse_identical_static_materials(&mut model).unwrap();
    assert_eq!(model.model.materials.len(), original + 1);
}

#[test]
fn animation_ownership_prevents_material_coalescing() {
    let mut model = converted();
    assert!(!model.model.animations.is_empty());
    assert!(
        model
            .model
            .animations
            .iter()
            .any(|a| !a.metadata.events.is_empty()
                || !a.metadata.float_curves.is_empty()
                || !a.metadata.object_curves.is_empty())
    );
    let original = model.model.materials.len();
    model.model.materials.push(model.model.materials[0].clone());
    model
        .material_reports
        .push(model.material_reports[0].clone());
    reuse_identical_static_materials(&mut model).unwrap();
    assert_eq!(model.model.materials.len(), original + 1);
}
