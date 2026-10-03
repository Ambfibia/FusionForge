use super::*;

#[derive(Clone, Debug, Deserialize)]
pub(super) struct LauncherBundleInfo {
    pub(super) compressed_info: LauncherFileInfo,
}

#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct BundleCookReport {
    pub(super) bundle: String,
    pub(super) emitted: u64,
    pub(super) errors: Vec<String>,
}

pub(super) fn process_bundle(
    index: usize,
    entry: &InventoryEntry,
    extraction_root: &Path,
    state: &mut CookState,
    report: &mut BundleCookReport,
    mappings: &mut Vec<CookMapping>,
    roots: &ErrorRoots<'_>,
) -> Result<(), String> {
    state.coverage.bundle_files_scanned += 1;
    let bundle_work = extraction_root.join(format!("bundle-{index:05}"));
    fs::create_dir(&bundle_work).map_err(|err| format!("{}: {err}", bundle_work.display()))?;
    let extracted = match extract_bundle(&entry.path, &bundle_work) {
        Ok(value) => value,
        Err(err) => {
            state.coverage.bundle_failures += 1;
            report.errors.push(roots.sanitize(&err));
            fs::remove_dir_all(&bundle_work)
                .map_err(|remove_err| format!("{}: {remove_err}", bundle_work.display()))?;
            return Ok(());
        }
    };
    if extracted.files.is_empty() {
        report
            .errors
            .push("bundle contains no extractable files".to_string());
    }

    let env = UnityEnvironment::from_dir(&bundle_work);
    if env.assets.is_empty() {
        report
            .errors
            .push("bundle contains no readable serialized assets".to_string());
    }
    for (asset_index, asset) in env.assets.iter().enumerate() {
        for info in asset.objects.values() {
            let object_type = asset.object_type_name(info);
            state.coverage.bundle_objects_scanned += 1;
            *state
                .coverage
                .bundle_types
                .entry(object_type.clone())
                .or_default() += 1;
            if !matches!(
                object_type.as_str(),
                "Texture2D" | "Mesh" | "AudioClip" | "Font"
            ) {
                state.coverage.bundle_skipped += 1;
                continue;
            }
            state.coverage.bundle_supported_objects += 1;
            let body = match asset.read_object(asset_index, info) {
                Ok(value) => value,
                Err(err) => {
                    state.coverage.bundle_skipped += 1;
                    report.errors.push(format!(
                        "{}#{} {}: {}",
                        asset.name,
                        info.path_id,
                        object_type,
                        roots.sanitize(&err)
                    ));
                    continue;
                }
            };
            let name = object_name(&body);
            let result = match object_type.as_str() {
                "Texture2D" => cook_texture(&env, &body, &name, state),
                "Mesh" => cook_mesh(&body, &name, state),
                "AudioClip" => cook_audio(&body, &name, state),
                "Font" => cook_font(&body, &name, state),
                _ => unreachable!(),
            };
            match result {
                Ok(emitted) => {
                    state.coverage.bundle_emitted += 1;
                    report.emitted += 1;
                    mappings.push(CookMapping {
                        kind: content_kind_name(match object_type.as_str() {
                            "Texture2D" => ContentKind::Texture,
                            "Mesh" => ContentKind::Mesh,
                            "AudioClip" => ContentKind::Audio,
                            "Font" => ContentKind::Font,
                            _ => unreachable!(),
                        })
                        .to_string(),
                        name: emitted.name,
                        native_key: emitted.key,
                        native_path: emitted.path,
                        source: CookSourceDescriptor {
                            bundle: Some(entry.name.clone()),
                            path_id: Some(info.path_id),
                            context: Some(object_type.clone()),
                            asset: Some(asset.name.clone()),
                            ..CookSourceDescriptor::default()
                        },
                    });
                }
                Err(ObjectCookError::Object(err)) => {
                    state.coverage.bundle_skipped += 1;
                    report.errors.push(format!(
                        "{}#{} {}: {}",
                        asset.name,
                        info.path_id,
                        object_type,
                        roots.sanitize(&err)
                    ));
                }
                Err(ObjectCookError::Output(err)) => return Err(err),
            }
        }
    }
    fs::remove_dir_all(&bundle_work).map_err(|err| format!("{}: {err}", bundle_work.display()))?;
    Ok(())
}
