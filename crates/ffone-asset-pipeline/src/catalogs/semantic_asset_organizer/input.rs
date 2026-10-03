use super::*;

pub(super) fn read_evidence(role: &str, path: &Path, expected_schema: &str) -> Result<EvidenceDocument> {
    let bytes = fs::read(path).map_err(|error| io_at(path, error))?;
    let value: Value = serde_json::from_slice(&bytes).map_err(|source| PipelineError::Json {
        path: path.to_string_lossy().into_owned(),
        source,
    })?;
    let schema = value
        .get("schema")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if schema != expected_schema {
        return semantic_error(format!(
            "{role} schema must be {expected_schema:?}, got {schema:?} at {}",
            path.display()
        ));
    }
    Ok(EvidenceDocument {
        evidence: InputEvidence {
            role: role.to_owned(),
            path: display_path(path),
            schema: schema.to_owned(),
            bytes: bytes.len() as u64,
            blake3: blake3::hash(&bytes).to_hex().to_string(),
        },
        value,
    })
}

pub(super) fn read_logical_plan(
    plan: &Value,
) -> (
    BTreeMap<String, Vec<ReadyRoot>>,
    BTreeMap<String, PlanRouteBlockers>,
    Vec<String>,
) {
    let mut roots = BTreeMap::<String, Vec<ReadyRoot>>::new();
    for item in value_array(plan.get("logicalRoots")) {
        if item.get("status").and_then(Value::as_str) != Some("ready") {
            continue;
        }
        let Some(route) = item
            .pointer("/kfm/normalizedRoute")
            .and_then(Value::as_str)
            .map(normalize_route)
        else {
            continue;
        };
        let Some(true_name) = item
            .pointer("/payload/selfContainedGameObject/rootName")
            .and_then(Value::as_str)
        else {
            continue;
        };
        let proof = item
            .pointer("/payload/selfContainedGameObject/proof")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let source = item
            .pointer("/payload/selfContainedGameObject/rootGameObject")
            .unwrap_or(&Value::Null);
        roots.entry(route.clone()).or_default().push(ReadyRoot {
            route,
            true_name: true_name.to_owned(),
            proof: proof.to_owned(),
            source_root: SerializedObjectIdentity {
                asset_name: source
                    .get("assetName")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_owned(),
                path_id: source
                    .get("pathId")
                    .and_then(Value::as_i64)
                    .unwrap_or_default(),
                object_type: source
                    .get("objectType")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_owned(),
            },
            feature_closure: ModelFeatureClosure {
                status: ModelFeatureClosureStatus::PreserveExactLogicalRootClosure,
                source_mesh_count: u64_pointer(
                    item,
                    "/payload/selfContainedGameObject/meshCount",
                ),
                source_skinned_mesh_renderer_count: u64_pointer(
                    item,
                    "/payload/selfContainedGameObject/skinnedMeshRendererCount",
                ),
                source_bone_count: u64_pointer(
                    item,
                    "/payload/selfContainedGameObject/boneCount",
                ),
                source_animation_component_count: u64_pointer(
                    item,
                    "/payload/selfContainedGameObject/animationComponentCount",
                ),
                source_animation_clip_count: u64_pointer(
                    item,
                    "/payload/selfContainedGameObject/animationClipCount",
                ),
                detail: "GLB publication must preserve the exact proven GameObject subtree, including meshes, skin joints/weights/inverse bind matrices, and legacy animation clips; the semantic organizer never decomposes that closure".to_owned(),
            },
            material_count: item
                .pointer("/payload/selfContainedGameObject/materialCount")
                .and_then(Value::as_u64)
                .unwrap_or_default(),
        });
    }
    let mut route_blockers = BTreeMap::<String, PlanRouteBlockers>::new();
    let mut global = Vec::new();
    for blocker in value_array(plan.get("blockers")) {
        let code = blocker
            .get("code")
            .and_then(Value::as_str)
            .unwrap_or("unknown")
            .to_owned();
        let detail = blocker
            .get("detail")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned();
        let route = blocker
            .get("normalizedRoute")
            .and_then(Value::as_str)
            .or_else(|| {
                value_array(blocker.get("exactRoutes"))
                    .first()
                    .and_then(Value::as_str)
            })
            .map(normalize_route)
            .filter(|route| !route.is_empty());
        if let Some(route) = route {
            let entry = route_blockers.entry(route).or_default();
            entry.codes.insert(code);
            if !detail.is_empty() {
                entry.details.insert(detail);
            }
        } else {
            global.push(format!("{code}: {detail}"));
        }
    }
    (roots, route_blockers, global)
}
