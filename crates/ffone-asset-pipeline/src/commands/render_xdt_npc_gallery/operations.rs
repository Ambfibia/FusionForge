use super::*;

pub(in super::super) fn run(raw_args: Vec<OsString>) -> Result<String, String> {
    let options = Options::parse(raw_args)?;
    if options.output.exists() && !options.resume {
        return Err(format!(
            "output already exists (pass --resume or choose a fresh directory): {}",
            options.output.display()
        ));
    }
    fs::create_dir_all(&options.output).map_err(|error| {
        format!(
            "cannot create output directory {}: {error}",
            options.output.display()
        )
    })?;

    let xdt_bytes = read_file(&options.xdt)?;
    let xdt: Value = serde_json::from_slice(&xdt_bytes)
        .map_err(|error| format!("cannot parse XDT {}: {error}", options.xdt.display()))?;
    let registry_path = options.asset_root.join("_runtime/characters.json");
    let registry_bytes = read_file(&registry_path)?;
    let registry: RuntimeRegistry = serde_json::from_slice(&registry_bytes).map_err(|error| {
        format!(
            "cannot parse runtime registry {}: {error}",
            registry_path.display()
        )
    })?;
    if registry.schema != REGISTRY_SCHEMA {
        return Err(format!(
            "runtime registry has schema {:?}, expected {REGISTRY_SCHEMA:?}",
            registry.schema
        ));
    }
    let mut legacy = options
        .legacy_plan
        .as_deref()
        .map(load_legacy_evidence)
        .transpose()?
        .unwrap_or_default();
    for report in &options.blocker_reports {
        load_supplemental_blockers(report, &mut legacy)?;
    }

    let registry_by_stem = index_registry(&registry)?;
    let npc_table = object_field(&xdt, "m_pNpcTable")?;
    let rows = array_field(npc_table, "m_pNpcData")?;
    let meshes = array_field(npc_table, "m_pNpcMeshData")?;
    let names = array_field(npc_table, "m_pNpcStringData")?;

    let texture_catalog =
        RuntimeTextureCatalog::load(&options.asset_root, &options.texture_metadata)?;
    let mut records = Vec::<EntityRecord>::new();
    let mut tasks = BTreeMap::<String, ModelTask>::new();
    let mut rows_with_mesh_index = 0_usize;

    for (row_index, row) in rows.iter().enumerate() {
        let Some(mesh_index) = positive_usize(row, "m_iMesh") else {
            continue;
        };
        rows_with_mesh_index += 1;
        let Some(mesh) = meshes.get(mesh_index) else {
            continue;
        };
        let Some(model_stem) = model_name(mesh.get("m_pstrMMeshModelString")) else {
            continue;
        };
        let npc_number = integer(row, "m_iNpcNumber").unwrap_or_default();
        let name_index = non_negative_usize(row, "m_iNpcName");
        let npc_name = name_index
            .and_then(|index| names.get(index))
            .and_then(|entry| entry.get("m_strName"))
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned();
        let team = integer(row, "m_iTeam").unwrap_or_default();
        let hnpc = integer(row, "m_iHNpc").unwrap_or_default() != 0;
        let npc_class = integer(row, "m_iNpcType").unwrap_or_default();
        let role = if hnpc {
            "hnpc"
        } else if team == 2 {
            "mob"
        } else {
            "npc"
        };
        let table_scale = number(row, "m_fScale").unwrap_or_default();
        let legacy_route = normalized_route(&model_stem);
        let texture = texture_name(mesh.get("m_pstrMTextureString"));
        let texture2 = texture_name(mesh.get("m_pstrMTextureString2"));
        let texture_match = texture.as_deref().map(|name| texture_catalog.resolve(name));
        let texture2_match = texture2
            .as_deref()
            .map(|name| texture_catalog.resolve(name));
        let apply_npc_texture_overrides = !hnpc && npc_class < 100;
        let main_texture = apply_npc_texture_overrides
            .then(|| {
                texture_match
                    .as_ref()
                    .and_then(|resolved| resolved.path.clone())
            })
            .flatten();
        let sub_texture = apply_npc_texture_overrides
            .then(|| {
                texture2_match
                    .as_ref()
                    .and_then(|resolved| resolved.path.clone())
            })
            .flatten();
        let main_sampler = apply_npc_texture_overrides
            .then(|| {
                texture_match
                    .as_ref()
                    .and_then(|resolved| resolved.sampler.clone())
            })
            .flatten();
        let sub_sampler = apply_npc_texture_overrides
            .then(|| {
                texture2_match
                    .as_ref()
                    .and_then(|resolved| resolved.sampler.clone())
            })
            .flatten();
        let texture_override_unresolved = apply_npc_texture_overrides
            && texture_match
                .as_ref()
                .is_some_and(|resolved| resolved.path.is_none() || resolved.sampler.is_none());
        let texture2_override_unresolved = apply_npc_texture_overrides
            && texture2_match
                .as_ref()
                .is_some_and(|resolved| resolved.path.is_none() || resolved.sampler.is_none());
        let key = model_stem.to_lowercase();
        let runtime = registry_by_stem.get(&key).copied();
        let (status, blockers) = if !hnpc && npc_class >= 100 {
            ("hidden_location_marker".to_owned(), Vec::new())
        } else if is_primary_interaction_placeholder(&model_stem) {
            ("primary_interaction_placeholder".to_owned(), Vec::new())
        } else if !table_scale.is_finite() || table_scale <= 0.0 {
            ("invalid_table_scale".to_owned(), Vec::new())
        } else if runtime.is_some() && (texture_override_unresolved || texture2_override_unresolved)
        {
            ("texture_override_unresolved".to_owned(), Vec::new())
        } else if runtime.is_some() {
            ("runtime_available".to_owned(), Vec::new())
        } else if legacy.ready.contains(&legacy_route) {
            ("primary_ready_unpublished".to_owned(), Vec::new())
        } else if let Some(blockers) = legacy.blockers.get(&legacy_route) {
            (
                "primary_blocked".to_owned(),
                blockers.iter().cloned().collect(),
            )
        } else if options.legacy_plan.is_some() {
            ("not_in_primary_catalog".to_owned(), Vec::new())
        } else {
            ("not_in_runtime_registry".to_owned(), Vec::new())
        };

        let animation = runtime.and_then(|model| select_animation(&model.animations));
        let texture_present = texture_match
            .as_ref()
            .map(|resolved| resolved.path.is_some());
        let texture2_present = texture2_match
            .as_ref()
            .map(|resolved| resolved.path.is_some());

        if let Some(model) = runtime.filter(|_| status == "runtime_available") {
            let task_key = appearance_key(
                model,
                main_texture.as_deref(),
                sub_texture.as_deref(),
                main_sampler.as_ref(),
                sub_sampler.as_ref(),
            )?;
            if !tasks.contains_key(&task_key) {
                let relative_base = checked_registry_output_base(&model.id)?;
                let appearance_suffix = short_digest(&task_key);
                let appearance_base = relative_base.join(format!("xdt-{appearance_suffix}"));
                let screenshot = options
                    .output
                    .join("appearances")
                    .join(&appearance_base)
                    .with_extension("png");
                let report = options
                    .output
                    .join("appearances")
                    .join(&appearance_base)
                    .with_extension("report.json");
                let log = options
                    .output
                    .join("appearances")
                    .join(&appearance_base)
                    .with_extension("log.txt");
                tasks.insert(
                    task_key.clone(),
                    ModelTask {
                        key: task_key,
                        model: model.clone(),
                        scale: table_scale,
                        animation: animation.clone(),
                        main_texture: main_texture.clone(),
                        sub_texture: sub_texture.clone(),
                        main_sampler: main_sampler.clone(),
                        sub_sampler: sub_sampler.clone(),
                        relative_screenshot: relative_to(&options.output, &screenshot)?,
                        relative_report: relative_to(&options.output, &report)?,
                        screenshot,
                        report,
                        log,
                    },
                );
            }
        }
        let record_appearance_key = runtime
            .filter(|_| status == "runtime_available")
            .map(|model| {
                appearance_key(
                    model,
                    main_texture.as_deref(),
                    sub_texture.as_deref(),
                    main_sampler.as_ref(),
                    sub_sampler.as_ref(),
                )
            })
            .transpose()?;

        records.push(EntityRecord {
            row_index,
            npc_number,
            npc_name,
            role: role.to_owned(),
            team,
            hnpc,
            npc_class,
            mesh_index,
            model_stem,
            legacy_route,
            table_scale,
            texture,
            texture2,
            texture_present,
            texture2_present,
            texture_path: texture_match
                .as_ref()
                .and_then(|resolved| resolved.path.clone()),
            texture2_path: texture2_match
                .as_ref()
                .and_then(|resolved| resolved.path.clone()),
            texture_resolution: texture_match
                .as_ref()
                .map(|resolved| resolved.status.to_owned()),
            texture2_resolution: texture2_match
                .as_ref()
                .map(|resolved| resolved.status.to_owned()),
            texture_sampler_resolution: texture_match
                .as_ref()
                .map(|resolved| resolved.sampler_status.to_owned()),
            texture2_sampler_resolution: texture2_match
                .as_ref()
                .map(|resolved| resolved.sampler_status.to_owned()),
            registry_id: runtime.map(|model| model.id.clone()),
            registry_category: runtime.map(|model| model.category.clone()),
            logical_name: runtime.map(|model| model.logical_name.clone()),
            glb: runtime.map(|model| model.glb.clone()),
            selected_animation: animation,
            status,
            blockers,
            image: String::new(),
            model_report: None,
            appearance_key: record_appearance_key,
        });
    }

    let render_results = render_models(&options, tasks.values())?;
    let result_by_appearance = render_results
        .iter()
        .map(|result| (result.appearance_key.clone(), result))
        .collect::<BTreeMap<_, _>>();

    create_placeholders(&options.output)?;
    for record in &mut records {
        if let Some(appearance_key) = record.appearance_key.as_deref()
            && let Some(result) = result_by_appearance.get(appearance_key)
        {
            record.status = result.status.clone();
            record.model_report = Some(result.report.clone());
        }
        let image_source = if record.status == "rendered" {
            result_by_appearance
                .get(record.appearance_key.as_deref().unwrap_or_default())
                .map(|result| options.output.join(&result.screenshot))
                .ok_or_else(|| "rendered record lost its model screenshot".to_owned())?
        } else {
            options
                .output
                .join("placeholders")
                .join(format!("{}.png", record.status))
        };
        ensure_placeholder_for_status(&image_source, &record.status)?;
        let entity_path = options
            .output
            .join("entities")
            .join(&record.role)
            .join(format!(
                "row-{row:04}_npc-{npc:04}.png",
                row = record.row_index,
                npc = record.npc_number
            ));
        link_or_copy(&image_source, &entity_path, options.resume)?;
        record.image = relative_to(&options.output, &entity_path)?;
    }

    let status_counts = count_by(records.iter().map(|record| record.status.as_str()));
    let role_counts = count_by(records.iter().map(|record| record.role.as_str()));
    let missing_models = build_missing_model_report(&records);
    let texture_resolutions = build_texture_resolution_report(&records, &texture_catalog);
    let animation_coverage = build_animation_coverage_report(&render_results);
    let gpu_issues = build_gpu_issue_report(&options.output, &render_results);
    let unique_runtime_models = render_results
        .iter()
        .map(|result| result.registry_id.as_str())
        .collect::<BTreeSet<_>>()
        .len();
    let rendered_appearances = render_results
        .iter()
        .filter(|result| result.status == "rendered")
        .count();
    let render_failures = render_results
        .iter()
        .filter(|result| result.status == "render_failed")
        .count();
    let limited_models = render_results
        .iter()
        .filter(|result| result.status == "not_rendered_limit")
        .count();

    let inputs = json!({
        "xdt": input_proof("patched", &options.xdt, &xdt_bytes),
        "runtimeRegistry": input_proof("native-runtime", &registry_path, &registry_bytes),
        "legacyPlan": options.legacy_plan.as_deref().map(|path| {
            let bytes = fs::read(path).unwrap_or_default();
            input_proof("primary", path, &bytes)
        }),
        "blockerReports": options.blocker_reports.iter().map(|path| {
            let bytes = fs::read(path).unwrap_or_default();
            input_proof("primary-derived", path, &bytes)
        }).collect::<Vec<_>>(),
        "textureMetadata": options.texture_metadata.iter().map(|path| {
            let bytes = fs::read(path).unwrap_or_default();
            input_proof("primary", path, &bytes)
        }).collect::<Vec<_>>(),
    });
    let summary = json!({
        "schema": SCHEMA,
        "status": if render_failures == 0 { "generated" } else { "generated_with_render_failures" },
        "inputs": inputs,
        "command": options.command,
        "counts": {
            "xdtRows": rows.len(),
            "rowsWithPositiveMeshIndex": rows_with_mesh_index,
            "eligibleRows": records.len(),
            "excludedRows": rows.len().saturating_sub(records.len()),
            "uniqueRuntimeModels": unique_runtime_models,
            "uniqueRuntimeAppearances": tasks.len(),
            "renderedAppearances": rendered_appearances,
            "renderFailures": render_failures,
            "limitedModels": limited_models,
            "uniqueMissingModels": missing_models.len(),
            "textureResolutionGroups": texture_resolutions.len(),
            "animationCoverageIssues": animation_coverage.len(),
            "gpuIssueModels": gpu_issues.len(),
        },
        "statusCounts": status_counts,
        "roleCounts": role_counts,
        "presentation": {
            "cameraView": "reverse",
            "blankCameraRetry": "same",
            "characterRuntimeHalfTurn": true,
            "intent": "front-facing audit view after the exact gameplay character half-turn",
        },
        "notes": [
            "Each eligible XDT row has its own entities/<role> PNG path.",
            "Rows with the same native model and resolved XDT main/sub textures hard-link one deduplicated appearance render.",
            "Every real GPU appearance uses the front-facing reverse camera after the exact gameplay character half-turn; clear-frame warmup retries preserve that side.",
            "NpcMoveController classes 100 and above are hidden location markers in primary Retrobution and receive an explicit placeholder instead of the ObjectNPC1 cube.",
            "Visible ObjectNPC1 rows are explicit primary interaction placeholders: the source is one skinned mesh with a 1x1 texture and table scale 0.01, not the named world prop.",
            "Missing or blocked routes use color-coded diagnostic placeholders, never guessed models.",
            "For visible non-HNPC rows, XDT Texture1 binds the exact main material and Texture2 the exact sub material. Malformed legacy names use a typed shader fallback only when FusionEffect plus Texture2 proves the sub role (Fusion Flapjack acceptance case), matching NpcMoveController.SetupNPC semantics.",
        ],
    });
    write_json(&options.output.join("summary.json"), &summary)?;
    write_json(
        &options.output.join("manifest.json"),
        &json!({"schema": SCHEMA, "entities": records, "modelRenders": render_results}),
    )?;
    write_json(&options.output.join("missing-models.json"), &missing_models)?;
    write_json(
        &options.output.join("texture-resolution.json"),
        &json!({
            "schema": SCHEMA,
            "status": "resolved_against_native_runtime_texture_catalog",
            "primarySources": texture_catalog.source_proofs,
            "primarySelection": {
                "trueName": "spawn11_green",
                "path": "map/shared/effects/textures/spawn11_green/primary.png",
                "authority": "primary NpcTexture.resourceFile texture/spawn11_green.dds; already accepted by the tutorial runtime"
            },
            "textures": texture_resolutions,
        }),
    )?;
    if let Some(path) = &options.texture_catalog_output {
        let runtime_texture_catalog =
            build_runtime_texture_catalog(&records, &texture_catalog, &options.asset_root)?;
        write_json(path, &runtime_texture_catalog)?;
    }
    write_json(
        &options.output.join("animation-coverage.json"),
        &json!({"schema": SCHEMA, "issues": animation_coverage}),
    )?;
    write_json(
        &options.output.join("gpu-issues.json"),
        &json!({"schema": SCHEMA, "issues": gpu_issues}),
    )?;
    write_html(&options.output.join("index.html"), &summary, &records)?;

    Ok(format!(
        "NPC gallery generated: eligible-rows={} rendered-rows={} unresolved-rows={} unique-runtime-models={} unique-appearances={} rendered-appearances={} render-failures={} output={}",
        records.len(),
        status_counts.get("rendered").copied().unwrap_or_default(),
        records
            .len()
            .saturating_sub(status_counts.get("rendered").copied().unwrap_or_default()),
        unique_runtime_models,
        tasks.len(),
        rendered_appearances,
        render_failures,
        options.output.display()
    ))
}

pub(super) fn build_gpu_issue_report(output: &Path, renders: &[ModelRenderRecord]) -> Vec<Value> {
    let mut issues = Vec::new();
    for render in renders {
        let path = output.join(&render.report);
        let Ok(bytes) = fs::read(&path) else {
            if render.status == "render_failed" {
                issues.push(json!({
                    "code": "gpu_report_missing",
                    "registryId": render.registry_id,
                    "glb": render.glb,
                    "report": render.report,
                }));
            }
            continue;
        };
        let Ok(report) = serde_json::from_slice::<Value>(&bytes) else {
            issues.push(json!({
                "code": "gpu_report_invalid_json",
                "registryId": render.registry_id,
                "glb": render.glb,
                "report": render.report,
            }));
            continue;
        };
        let status = report
            .get("status")
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        let material_errors = report
            .get("materialErrors")
            .and_then(Value::as_u64)
            .unwrap_or_default();
        let shader_errors = report
            .get("shaderErrors")
            .and_then(Value::as_u64)
            .unwrap_or_default();
        if status != "success" || material_errors > 0 || shader_errors > 0 {
            issues.push(json!({
                "code": "gpu_acceptance_failed",
                "registryId": render.registry_id,
                "logicalName": render.logical_name,
                "glb": render.glb,
                "status": status,
                "error": report.get("error"),
                "materialErrors": material_errors,
                "shaderErrors": shader_errors,
                "meshes": report.get("meshes"),
                "skinnedMeshes": report.get("skinnedMeshes"),
                "animations": report.get("animations"),
                "animationPlayers": report.get("animationPlayers"),
                "report": render.report,
                "log": render.log,
            }));
        }
    }
    issues
}

pub(super) fn create_placeholders(output: &Path) -> Result<(), String> {
    for status in [
        "hidden_location_marker",
        "primary_interaction_placeholder",
        "texture_override_unresolved",
        "invalid_table_scale",
        "primary_ready_unpublished",
        "primary_blocked",
        "not_in_primary_catalog",
        "not_in_runtime_registry",
        "not_rendered",
        "not_rendered_limit",
        "render_failed",
    ] {
        let path = output.join("placeholders").join(format!("{status}.png"));
        ensure_placeholder_for_status(&path, status)?;
    }
    Ok(())
}

pub(super) fn appearance_key(
    model: &RuntimeModel,
    main_texture: Option<&str>,
    sub_texture: Option<&str>,
    main_sampler: Option<&NativeSampler>,
    sub_sampler: Option<&NativeSampler>,
) -> Result<String, String> {
    Ok(format!(
        "{}|glbBlake3={}|main={}|sub={}|mainSampler={}|subSampler={}",
        model.id.to_lowercase(),
        model.glb_blake3.to_lowercase(),
        main_texture.unwrap_or("-"),
        sub_texture.unwrap_or("-"),
        serde_json::to_string(&main_sampler)
            .map_err(|error| format!("cannot serialize main sampler identity: {error}"))?,
        serde_json::to_string(&sub_sampler)
            .map_err(|error| format!("cannot serialize sub sampler identity: {error}"))?,
    ))
}

pub(super) fn short_digest(value: &str) -> String {
    sha256_hex(value.as_bytes())[..12].to_owned()
}

pub(super) fn is_primary_interaction_placeholder(model_stem: &str) -> bool {
    model_stem.eq_ignore_ascii_case("ObjectNPC1") || model_stem.eq_ignore_ascii_case("RXcom")
}

pub(super) fn array_field<'a>(value: &'a Value, field: &str) -> Result<&'a [Value], String> {
    value
        .get(field)
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .ok_or_else(|| format!("XDT field {field:?} is missing or not an array"))
}

pub(super) fn integer(value: &Value, field: &str) -> Option<i64> {
    value.get(field)?.as_i64()
}

pub(super) fn number(value: &Value, field: &str) -> Option<f64> {
    value.get(field)?.as_f64()
}

pub(super) fn non_negative_usize(value: &Value, field: &str) -> Option<usize> {
    usize::try_from(integer(value, field)?).ok()
}

pub(super) fn positive_usize(value: &Value, field: &str) -> Option<usize> {
    non_negative_usize(value, field).filter(|index| *index > 0)
}

pub(super) fn count_by<'a>(values: impl Iterator<Item = &'a str>) -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::new();
    for value in values {
        *counts.entry(value.to_owned()).or_default() += 1;
    }
    counts
}

pub(super) fn input_proof(alias: &str, path: &Path, bytes: &[u8]) -> Value {
    json!({
        "sourceAlias": alias,
        "path": path.to_string_lossy().replace('\\', "/"),
        "bytes": bytes.len(),
        "sha256": sha256_hex(bytes),
    })
}

pub(super) fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

pub(super) fn is_sha256(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

pub(super) fn is_blake3(value: &str) -> bool {
    is_sha256(value)
}

pub(super) fn canonical_file(path: &Path) -> Result<PathBuf, String> {
    let canonical = fs::canonicalize(path)
        .map_err(|error| format!("cannot resolve {}: {error}", path.display()))?;
    if !canonical.is_file() {
        return Err(format!("{} is not a file", canonical.display()));
    }
    Ok(canonical)
}

pub(super) fn relative_to(root: &Path, path: &Path) -> Result<String, String> {
    path.strip_prefix(root)
        .map(|relative| relative.to_string_lossy().replace('\\', "/"))
        .map_err(|_| {
            format!(
                "path {} is outside output root {}",
                path.display(),
                root.display()
            )
        })
}

pub(super) fn set_once<T>(slot: &mut Option<T>, value: T, flag: &str) -> Result<(), String> {
    if slot.replace(value).is_some() {
        Err(format!("{flag} may only be provided once"))
    } else {
        Ok(())
    }
}

pub(super) fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}
