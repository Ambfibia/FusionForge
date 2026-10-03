use super::*;

/// Plans or transactionally applies the short top-level `objects/` layout.
pub fn normalize_object_routes(
    options: &ObjectRouteNormalizationOptions,
) -> Result<ObjectRouteNormalizationReport> {
    let project_root = canonical_directory(&options.project_root, "project root")?;
    let asset_root = canonical_directory(&project_root.join("assets/game"), "asset root")?;
    let map_root = canonical_directory(&asset_root.join("map"), "map root")?;
    let source_root = canonical_directory(&map_root.join("objects"), "legacy object root")?;
    let destination_root = asset_root.join("objects");
    if destination_root.exists() {
        return invalid(format!(
            "top-level object root already exists: {}",
            destination_root.display()
        ));
    }
    let catalog_path = map_root.join("catalog.json");
    let source_catalog_bytes = read_file(&catalog_path, "map catalog")?;
    let catalog: JsonValue =
        serde_json::from_slice(&source_catalog_bytes).map_err(|source| PipelineError::Json {
            path: catalog_path.display().to_string(),
            source,
        })?;
    let plan = build_plan(&asset_root, &source_root, &catalog)?;
    let path_lengths = path_length_report(&plan.file_routes);
    if path_lengths.maximum_after > 180 || path_lengths.maximum_component_after > 64 {
        return invalid(format!(
            "short object route policy failed: maxPath={}, maxComponent={}",
            path_lengths.maximum_after, path_lengths.maximum_component_after
        ));
    }

    let mut result_catalog_blake3 = None;
    let mut verification = None;
    if options.apply {
        let applied = apply_plan(
            &project_root,
            &asset_root,
            &map_root,
            &source_root,
            &destination_root,
            &catalog,
            &plan,
        )?;
        result_catalog_blake3 = Some(applied.0);
        verification = Some(applied.1);
    }

    let report = ObjectRouteNormalizationReport {
        schema: OBJECT_ROUTE_NORMALIZATION_SCHEMA.to_owned(),
        mode: if options.apply { "apply" } else { "plan" }.to_owned(),
        source_alias: "primary".to_owned(),
        source_build: "retrobution-20260613".to_owned(),
        policy: "map/catalog.json retains logical ownership; reusable packages live at objects/<category>/<short-name>, per-object payloads live in models/<short-name>, and set atlases use short names below textures/; exact legacy names and PREFIX/FAMILY evidence remain in catalog/object metadata"
            .to_owned(),
        counts: ObjectRouteNormalizationCounts {
            packages: plan.package_routes.len() as u64,
            members: plan.members as u64,
            textures: plan.textures as u64,
            files: plan.file_routes.len() as u64,
            renamed_member_directories: plan.renamed_members as u64,
        },
        path_lengths,
        source_catalog_blake3: hash(&source_catalog_bytes),
        result_catalog_blake3,
        verification,
        routes: plan.package_routes,
    };
    write_report(&project_root, &asset_root, &options.report_path, &report)?;
    Ok(report)
}
