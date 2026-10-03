use super::*;

fn candidate(
    category: &str,
    name: &str,
    skinned: bool,
    animated: bool,
    textured: bool,
    multi_pass: bool,
) -> Candidate {
    let facts = LogicalModelGpuFacts {
        true_name: name.to_owned(),
        standard_animation_names: if animated {
            vec!["exact_idle".to_owned()]
        } else {
            Vec::new()
        },
        mesh_parts: 1,
        skinned_mesh_parts: u64::from(skinned),
        skin_joint_references: if skinned { 12 } else { 0 },
        inverse_bind_matrices: if skinned { 12 } else { 0 },
        materials_applied: 1,
        legacy_pass_companions: u64::from(multi_pass),
        outline_pass_companions: u64::from(multi_pass),
        assigned_texture_bindings: u64::from(textured),
        exact_mip_markers: 1,
        exact_mip_chains: 0,
        exact_mip_levels: 0,
    };
    Candidate {
        category: category.to_owned(),
        true_name: name.to_owned(),
        relative_glb: format!("characters/player/equipment/{category}/{name}/{name}.glb"),
        variants: variant_labels(&facts),
        facts,
    }
}

#[test]
fn smoke_selection_covers_every_present_slot_and_observed_variant() {
    let candidates = vec![
        candidate("back", "back_a", false, false, false, false),
        candidate("hat", "hat_a", true, true, true, true),
        candidate("weapon", "weapon_a", false, false, true, false),
    ];
    let (selection, required_variants) =
        select_candidates(EquipmentGpuBatchMode::Smoke, None, &candidates);
    let selected = selection
        .iter()
        .map(|selection| &candidates[selection.index])
        .collect::<Vec<_>>();
    let categories = selected
        .iter()
        .map(|candidate| candidate.category.as_str())
        .collect::<BTreeSet<_>>();
    let variants = selected
        .iter()
        .flat_map(|candidate| candidate.variants.iter().cloned())
        .collect::<BTreeSet<_>>();
    assert_eq!(categories, BTreeSet::from(["back", "hat", "weapon"]));
    assert_eq!(variants, required_variants);
}

#[test]
fn animation_selection_is_exact_published_name_not_an_index() {
    let candidate = candidate("back", "back_animated", true, true, true, true);
    assert_eq!(
        candidate
            .facts
            .standard_animation_names
            .first()
            .map(String::as_str),
        Some("exact_idle")
    );
}

#[test]
fn scope_blockers_keep_missing_mask_and_attachment_parity_explicit() {
    let candidates = vec![
        candidate("hat", "hat_a", false, false, true, true),
        candidate("shirt", "shirt_a", true, false, true, true),
        candidate("mask", "mask_a", true, false, true, true),
        candidate("vehicle", "vehicle_a", false, false, true, false),
    ];
    let blockers = scope_blockers(&candidates);
    assert!(blockers.iter().any(|blocker| {
        blocker.category.as_deref() == Some("glasses")
            && blocker.code == "noPublishedCandidateForEquipmentSlot"
    }));
    assert!(blockers.iter().any(|blocker| {
        blocker.category.as_deref() == Some("mask")
            && blocker.code == "playerSkinnedFaceAssemblyParityPending"
    }));
    assert!(blockers.iter().any(|blocker| {
        blocker.category.as_deref() == Some("hat")
            && blocker.code == "playerSocketAttachmentParityPending"
    }));
    assert!(blockers.iter().any(|blocker| {
        blocker.category.as_deref() == Some("shirt")
            && blocker.code == "playerSkinnedBodyAssemblyParityPending"
    }));
    assert!(blockers.iter().any(|blocker| {
        blocker.category.as_deref() == Some("vehicle")
            && blocker.code == "playerVehicleMountParityPending"
    }));
}

#[test]
fn full_shards_are_disjoint_and_cover_every_candidate() {
    let candidates = (0..11)
        .map(|index| {
            candidate(
                "weapon",
                &format!("weapon_{index:02}"),
                index % 2 == 0,
                false,
                true,
                false,
            )
        })
        .collect::<Vec<_>>();
    let mut covered = BTreeSet::new();
    for shard_index in 0..4 {
        let (selected, _) = select_candidates(
            EquipmentGpuBatchMode::Full,
            Some(EquipmentGpuShard {
                index: shard_index,
                count: 4,
            }),
            &candidates,
        );
        for selection in selected {
            assert!(covered.insert(selection.index));
        }
    }
    assert_eq!(covered, (0..candidates.len()).collect());
}

#[test]
fn preview_failures_are_typed_from_runtime_json_without_guessing() {
    let shader = br#"{"status":"error","error":"exact legacy material metadata error: unsupported exact legacy shader name: Shader/Exact"}"#;
    let shader = classify_preview_failure(shader);
    assert_eq!(shader.code, "runtimeLegacyShaderUnsupported");
    assert_eq!(
        shader.typed_evidence["unsupportedExactLegacyShaderNames"][0],
        "Shader/Exact"
    );

    let clear = br#"{"status":"error","error":"frame limit reached: 60 clear-only GPU captures while pipelines warmed up"}"#;
    assert_eq!(
        classify_preview_failure(clear).code,
        "gpuVisibleSurfaceEvidenceMissing"
    );
}

#[test]
fn route_qualified_taxonomy_uses_the_exact_xdt_route_stem() {
    let qualified = "characters/player/equipment/vehicle/vehicle_policecar/vehicle_hovercarA/vehicle_hovercarA.glb";
    assert!(is_exact_equipment_output_path(
        "vehicle",
        "wear/vehicle_policecar.nif",
        "vehicle_hovercarA",
        qualified,
    ));
    assert!(!is_exact_equipment_output_path(
        "vehicle",
        "wear/vehicle_hovercar1.nif",
        "vehicle_hovercarA",
        qualified,
    ));
}
