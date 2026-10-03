use super::*;

pub(super) fn export_exact_texture(args: &[String]) -> Result<(), String> {
    let args = args.to_vec();
    run_large_stack_task(move || {
        let path = required_path(
            &args,
            0,
            "export-exact-texture <asset-or-bundle> <path-id> <out.json>",
        )?;
        let path_id = args
            .get(1)
            .ok_or_else(|| "export-exact-texture requires an exact Texture2D path id".to_string())
            .and_then(|value| parse_i64(value))?;
        let output = required_path(
            &args,
            2,
            "export-exact-texture <asset-or-bundle> <path-id> <out.json>",
        )?;
        if args.len() != 3 {
            return Err(
                "export-exact-texture requires <asset-or-bundle> <path-id> <out.json>".to_string(),
            );
        }
        let loaded = load_input(&path)?;
        let (asset_index, _asset, _info) = find_object(&loaded.env, path_id)?;
        let texture = crate::logical_model_material::exact_texture(
            &loaded.env,
            ObjectKey {
                asset: asset_index,
                path_id,
            },
        )?;
        write_or_print_json(Some(&output), &texture)
    })
}

pub(super) fn export_exact_texture_batch(args: &[String]) -> Result<(), String> {
    let args = args.to_vec();
    run_large_stack_task(move || {
        if args.len() != 3 {
            return Err(
                "export-exact-texture-batch <asset-or-bundle> <selection.json> <fresh-out-dir>"
                    .to_owned(),
            );
        }
        let output = PathBuf::from(&args[2]);
        if output.exists() {
            return Err("texture batch output must be fresh".to_owned());
        }
        let selection: JsonValue =
            serde_json::from_slice(&fs::read(&args[1]).map_err(|e| e.to_string())?)
                .map_err(|e| e.to_string())?;
        let selections = selection
            .as_array()
            .ok_or("texture selection must be an array")?;
        let loaded = load_input(Path::new(&args[0]))?;
        let mut names = HashSet::new();
        let mut textures = Vec::new();
        for selected in selections {
            let name = selected["name"]
                .as_str()
                .ok_or("texture output name missing")?;
            if name.is_empty()
                || !name
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
                || !names.insert(name.to_ascii_lowercase())
            {
                return Err(format!("unsafe or duplicate texture output name {name:?}"));
            }
            let serialized = selected["serializedAsset"]
                .as_str()
                .ok_or("serializedAsset missing")?;
            let path_id = selected["pathId"].as_i64().ok_or("pathId missing")?;
            let matches = loaded
                .env
                .assets
                .iter()
                .enumerate()
                .filter(|(_, asset)| asset.name == serialized)
                .collect::<Vec<_>>();
            let [(asset_index, asset)] = matches.as_slice() else {
                return Err(format!(
                    "serialized asset {serialized:?} is absent or ambiguous"
                ));
            };
            if !asset.objects.contains_key(&path_id) {
                return Err(format!("texture {serialized}:{path_id} is absent"));
            }
            let texture = crate::logical_model_material::exact_texture(
                &loaded.env,
                ObjectKey {
                    asset: *asset_index,
                    path_id,
                },
            )?;
            textures.push((name.to_owned(), texture));
        }
        fs::create_dir_all(&output).map_err(|e| e.to_string())?;
        for (name, texture) in textures {
            let bytes = serde_json::to_vec(&texture).map_err(|e| e.to_string())?;
            fs::write(output.join(format!("{name}.exact.json")), bytes)
                .map_err(|e| e.to_string())?;
        }
        println!("exported {} scoped exact textures", selections.len());
        Ok(())
    })
}

pub(super) fn snapshot_material_texture_refs(
    env: &UnityEnvironment,
    selected: &BTreeSet<(usize, i64)>,
) -> BTreeSet<(usize, i64)> {
    let mut refs = BTreeSet::new();
    for (asset_index, path_id) in selected {
        let Some(asset) = env.assets.get(*asset_index) else {
            continue;
        };
        let Some(info) = asset.objects.get(path_id) else {
            continue;
        };
        if asset.object_type_name(info) != "Material" {
            continue;
        }
        let Ok(body) = asset.read_object(*asset_index, info) else {
            continue;
        };
        for (_label, pointer) in collect_pointers(&body, "") {
            for key in snapshot_pointer_candidate_keys(env, *asset_index, &pointer) {
                if snapshot_key_type(env, key).as_deref() == Some("Texture2D") {
                    refs.insert(key);
                }
            }
        }
    }
    refs
}

pub(super) fn write_snapshot_png_output(
    path: &Path,
    image: &RgbaImage,
    log_writes: bool,
) -> Result<(), String> {
    let mut png = Vec::new();
    PngEncoder::new(&mut png)
        .write_image(
            image.as_raw(),
            image.width(),
            image.height(),
            ColorType::Rgba8.into(),
        )
        .map_err(|err| err.to_string())?;
    write_snapshot_output(path, &png, log_writes)
}

pub(super) fn write_png(path: &Path, image: &RgbaImage) -> Result<(), String> {
    let mut png = Vec::new();
    PngEncoder::new(&mut png)
        .write_image(
            image.as_raw(),
            image.width(),
            image.height(),
            ColorType::Rgba8.into(),
        )
        .map_err(|err| err.to_string())?;
    write_output(path, &png, false)
}

pub(super) fn write_png_output(path: &Path, image: &RgbaImage, dry_run: bool) -> Result<(), String> {
    let mut png = Vec::new();
    PngEncoder::new(&mut png)
        .write_image(
            image.as_raw(),
            image.width(),
            image.height(),
            ColorType::Rgba8.into(),
        )
        .map_err(|err| err.to_string())?;
    write_output(path, &png, dry_run)
}
