use super::*;

#[allow(clippy::too_many_arguments)]
pub(super) fn audit_publish_report(
    output_root: &Path,
    report_path: &Path,
    report_relative: &str,
    glb_relative: &str,
    glb_hash: &str,
    glb_bytes: &[u8],
    logical_name: Option<&str>,
    actual: &LogicalModelFileCounts,
    pngs: &BTreeSet<String>,
    violations: &mut Vec<LogicalModelTreeViolation>,
) -> Result<()> {
    let bytes = fs::read(report_path).map_err(|error| io_at(report_path, error))?;
    let report: Value = match serde_json::from_slice(&bytes) {
        Ok(value) => value,
        Err(error) => {
            push_violation(
                violations,
                "invalid_publish_report_json",
                report_relative,
                error.to_string(),
            );
            return Ok(());
        }
    };
    if report.get("schema").and_then(Value::as_str) != Some(PUBLISH_REPORT_SCHEMA) {
        push_violation(
            violations,
            "invalid_publish_report_schema",
            report_relative,
            format!("report schema must be {PUBLISH_REPORT_SCHEMA:?}"),
        );
    }
    if report.get("status").and_then(Value::as_str) != Some("staged-incomplete") {
        push_violation(
            violations,
            "invalid_publish_report_status",
            report_relative,
            "report status must remain staged-incomplete until the GPU gate passes",
        );
    }
    if report.get("publishable").and_then(Value::as_bool) != Some(false) {
        push_violation(
            violations,
            "gpu_gate_bypassed",
            report_relative,
            "publishable must remain false until a separate Bevy GPU/runtime parity gate passes",
        );
    }
    if report.get("reportPath").and_then(Value::as_str) != Some(report_relative) {
        push_violation(
            violations,
            "report_path_mismatch",
            report_relative,
            "reportPath does not identify this exact sidecar",
        );
    }
    let contract = report.get("contract");
    if contract
        .and_then(|value| value.get("schema"))
        .and_then(Value::as_str)
        != Some(PUBLISH_CONTRACT_SCHEMA)
    {
        push_violation(
            violations,
            "invalid_publish_contract_schema",
            report_relative,
            format!("contract schema must be {PUBLISH_CONTRACT_SCHEMA:?}"),
        );
    }
    let report_coordinates = report
        .get("coordinateContract")
        .cloned()
        .and_then(|value| serde_json::from_value::<NativeCoordinateContract>(value).ok());
    let glb_coordinates = parse_glb(glb_bytes)
        .ok()
        .and_then(|parsed| {
            parsed
                .document
                .pointer("/extras/nativeCoordinateContract")
                .cloned()
        })
        .and_then(|value| serde_json::from_value::<NativeCoordinateContract>(value).ok());
    if report_coordinates.as_ref() != Some(&exact_native_coordinate_contract())
        || report_coordinates != glb_coordinates
    {
        push_violation(
            violations,
            "coordinate_report_mismatch",
            report_relative,
            "coordinateContract must exactly match the audited GLB and forbid recenter/rescale",
        );
    }
    audit_coordinate_report(&report, glb_bytes, report_relative, actual, violations);
    if contract
        .and_then(|value| aliased_field(value, "outputGlb", "output_glb"))
        .and_then(Value::as_str)
        != Some(glb_relative)
    {
        push_violation(
            violations,
            "contract_glb_path_mismatch",
            report_relative,
            "contract.outputGlb does not identify the adjacent audited GLB",
        );
    }
    if contract
        .and_then(|value| aliased_field(value, "glbBlake3", "glb_blake3"))
        .and_then(Value::as_str)
        != Some(glb_hash)
    {
        push_violation(
            violations,
            "contract_glb_hash_mismatch",
            report_relative,
            "contract.glbBlake3 does not match the GLB bytes",
        );
    }
    if let Some(name) = logical_name {
        for (camel, snake) in [("legacyName", "legacy_name"), ("rootNode", "root_node")] {
            if contract
                .and_then(|value| aliased_field(value, camel, snake))
                .and_then(Value::as_str)
                != Some(name)
            {
                push_violation(
                    violations,
                    "contract_logical_name_mismatch",
                    report_relative,
                    format!("contract.{snake} differs from GLB true logical name {name:?}"),
                );
            }
        }
    }
    let source = contract.and_then(|value| value.get("source"));
    let published = contract.and_then(|value| value.get("published"));
    if source.is_none() || source != published {
        push_violation(
            violations,
            "contract_feature_loss",
            report_relative,
            "contract source and published feature counts differ",
        );
    }
    if contract
        .and_then(|value| {
            aliased_field(
                value,
                "unresolvedSourceFeatures",
                "unresolved_source_features",
            )
        })
        .and_then(Value::as_array)
        .is_none_or(|values| !values.is_empty())
    {
        push_violation(
            violations,
            "unresolved_source_features",
            report_relative,
            "contract has missing or non-empty unresolvedSourceFeatures",
        );
    }
    if let Some(published) = published {
        let mismatches = feature_count_fields(&actual.features)
            .into_iter()
            .filter_map(|(camel, snake, expected)| {
                (aliased_field(published, camel, snake).and_then(Value::as_u64) != Some(expected))
                    .then_some(format!("{snake}: expected {expected}"))
            })
            .collect::<Vec<_>>();
        if !mismatches.is_empty() {
            push_violation(
                violations,
                "contract_actual_count_mismatch",
                report_relative,
                format!(
                    "contract published counts differ from audited GLB counts: {}",
                    mismatches.join(", ")
                ),
            );
        }
    }

    audit_semantic_proof(&report, report_relative, glb_bytes, violations);

    let material = report.get("materialPublish");
    if material
        .and_then(|value| value.get("status"))
        .and_then(Value::as_str)
        != Some("native-data-complete-runtime-validation-pending")
    {
        push_violation(
            violations,
            "invalid_material_publish_status",
            report_relative,
            "materialPublish.status must record native data completeness with runtime validation pending",
        );
    }
    for (field, expected) in [
        ("materialCount", actual.materials),
        ("textureCount", actual.textures),
        ("samplerCount", actual.samplers),
    ] {
        if material
            .and_then(|value| value.get(field))
            .and_then(Value::as_u64)
            != Some(expected)
        {
            push_violation(
                violations,
                "material_report_count_mismatch",
                report_relative,
                format!("materialPublish.{field} does not equal audited count {expected}"),
            );
        }
    }
    for (texture_index, texture) in material
        .and_then(|value| value.get("textures"))
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .enumerate()
    {
        let Some(uri) = texture.get("uri").and_then(Value::as_str) else {
            push_violation(
                violations,
                "invalid_report_texture",
                report_relative,
                format!("material texture report {texture_index} has no URI"),
            );
            continue;
        };
        let Some(relative) = safe_image_relative(glb_relative, uri) else {
            push_violation(
                violations,
                "unsafe_report_texture_uri",
                report_relative,
                format!("material texture report {texture_index} URI is unsafe"),
            );
            continue;
        };
        if !pngs.contains(&relative) {
            continue;
        }
        let path = output_root.join(Path::new(&relative));
        let bytes = fs::read(&path).map_err(|error| io_at(&path, error))?;
        let sha256 = format!("{:x}", Sha256::digest(&bytes));
        if texture.get("byteLength").and_then(Value::as_u64) != Some(to_u64(bytes.len()))
            || texture.get("sha256").and_then(Value::as_str) != Some(sha256.as_str())
        {
            push_violation(
                violations,
                "report_texture_hash_mismatch",
                report_relative,
                format!("texture report {texture_index} byte length/SHA-256 does not match PNG"),
            );
        }
        if let Some((width, height)) = png_dimensions(&bytes) {
            if texture.get("width").and_then(Value::as_u64) != Some(u64::from(width))
                || texture.get("height").and_then(Value::as_u64) != Some(u64::from(height))
            {
                push_violation(
                    violations,
                    "report_texture_dimension_mismatch",
                    report_relative,
                    format!("texture report {texture_index} dimensions do not match PNG"),
                );
            }
        }
        audit_report_mip_pngs(
            output_root,
            report_relative,
            glb_relative,
            texture_index,
            texture,
            pngs,
            violations,
        )?;
    }
    Ok(())
}

pub(super) fn audit_coordinate_report(
    report: &Value,
    glb_bytes: &[u8],
    report_relative: &str,
    actual: &LogicalModelFileCounts,
    violations: &mut Vec<LogicalModelTreeViolation>,
) {
    let coordinate: CoordinateAuditReport = match report
        .get("coordinateAudit")
        .cloned()
        .map(serde_json::from_value)
    {
        Some(Ok(value)) => value,
        Some(Err(error)) => {
            push_violation(
                violations,
                "invalid_coordinate_audit",
                report_relative,
                format!("coordinateAudit is not strict typed evidence: {error}"),
            );
            return;
        }
        None => {
            push_violation(
                violations,
                "missing_coordinate_audit",
                report_relative,
                "publish report has no coordinateAudit",
            );
            return;
        }
    };
    let parsed = parse_glb(glb_bytes).ok();
    let root = parsed.as_ref().and_then(|parsed| {
        let document = &parsed.document;
        let scene = document.get("scene").and_then(Value::as_u64).unwrap_or(0) as usize;
        let root = document
            .get("scenes")?
            .get(scene)?
            .get("nodes")?
            .as_array()?
            .first()?
            .as_u64()? as usize;
        document.get("nodes")?.get(root)
    });
    let root_matches = root.is_some_and(|root| {
        json_f64_array::<3>(root.get("translation")) == Some(coordinate.root_translation)
            && json_f64_array::<4>(root.get("rotation")) == Some(coordinate.root_rotation)
            && json_f64_array::<3>(root.get("scale")) == Some(coordinate.root_scale)
    });
    let bounds_valid = valid_coordinate_bounds(coordinate.raw_local_bounds)
        && valid_coordinate_bounds(coordinate.static_rest_world_bounds);
    let skinning_basis_valid = if actual.skins == 0 {
        coordinate.skinning_basis_parity_status == "not-applicable-rigid"
            && coordinate.skinning_basis_parity_max_error.is_none()
            && coordinate
                .current_pose_bind_identity_deviation_max
                .is_none()
            && coordinate
                .current_pose_bind_identity_deviation_worst
                .is_none()
    } else {
        coordinate.skinning_basis_parity_status
            == "unity-to-native-h-conjugation-algebraically-proven"
            && coordinate
                .skinning_basis_parity_max_error
                .is_some_and(|value| {
                    value.is_finite() && value >= 0.0 && value <= SKINNING_BASIS_PARITY_TOLERANCE
                })
            && valid_current_pose_bind_deviation(&coordinate)
    };
    if coordinate.status != "artifact-space-proven-runtime-spawn-policy-pending"
        || coordinate.runtime_spawn_policy != "runtime-archetype-dependent-pending"
        || coordinate.runtime_spawn_policy_note.trim().is_empty()
        || coordinate.unit_scale != 1.0
        || coordinate.auto_centered
        || coordinate.auto_scaled
        || coordinate.source_triangle_winding != "unity-source-indices-in-h-reflected-native-space"
        || coordinate.published_triangle_winding
            != "gltf-counter-clockwise-per-triangle-normal-aligned"
        || !coordinate.winding_swap_applied_per_triangle
        || coordinate.inverse_bind_matrices != actual.features.inverse_bind_matrices
        || !root_matches
        || !bounds_valid
        || !skinning_basis_valid
    {
        push_violation(
            violations,
            "coordinate_audit_mismatch",
            report_relative,
            "coordinateAudit does not prove exact artifact root/unit/bounds/winding/IBM/H-basis facts with current-pose bind deviation descriptive-only and runtime spawn semantics explicitly pending",
        );
    }
}

pub(super) fn audit_semantic_proof(
    report: &Value,
    report_relative: &str,
    glb_bytes: &[u8],
    violations: &mut Vec<LogicalModelTreeViolation>,
) {
    let Some(value) = report.get("semanticProof") else {
        push_violation(
            violations,
            "missing_semantic_roundtrip_proof",
            report_relative,
            "publish report has no value-level NativeModel/GLB semantic proof",
        );
        return;
    };
    let proof: SemanticRoundtripProof = match serde_json::from_value(value.clone()) {
        Ok(proof) => proof,
        Err(error) => {
            push_violation(
                violations,
                "invalid_semantic_roundtrip_proof",
                report_relative,
                format!("semanticProof is not the strict typed proof schema: {error}"),
            );
            return;
        }
    };
    if proof.schema != SEMANTIC_ROUNDTRIP_PROOF_SCHEMA
        || proof.status != "native-model-matches-redecoded-glb"
        || !proof.matched
        || proof.source != proof.emitted
        || proof.covered_scopes.len() != 6
    {
        push_violation(
            violations,
            "semantic_roundtrip_not_proven",
            report_relative,
            "semanticProof does not assert six matching value-level sections",
        );
    }
    match semantic_digests_from_glb(glb_bytes) {
        Ok(actual) if actual == proof.emitted => {}
        Ok(_) => push_violation(
            violations,
            "semantic_digest_mismatch",
            report_relative,
            "semanticProof emitted digests differ from values re-decoded from the audited GLB",
        ),
        Err(error) => push_violation(
            violations,
            "semantic_proof_redecode_failed",
            report_relative,
            format!("could not independently re-decode GLB semantics: {error}"),
        ),
    }
}

pub(super) fn validate_true_name(
    name: &str,
    label: &str,
    relative: &str,
    violations: &mut Vec<LogicalModelTreeViolation>,
) {
    if name.trim().is_empty()
        || name.chars().any(char::is_control)
        || name.contains(['/', '\\'])
        || has_generated_identity(name)
    {
        push_violation(
            violations,
            "generated_or_unsafe_name",
            relative,
            format!("{label} name {name:?} is not an exact safe source m_Name"),
        );
    }
}

pub(super) fn tree_error<T>(message: impl Into<String>) -> Result<T> {
    Err(PipelineError::ModelAudit(message.into()))
}
