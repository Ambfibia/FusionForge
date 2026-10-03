use super::*;

pub(super) fn default_patch_codes(replace_ascii: bool) -> Vec<u32> {
    let mut codes = Vec::new();
    if replace_ascii {
        codes.extend(0x20..0x7f);
    }
    codes.extend_from_slice(RUSSIAN_CODES);
    codes.extend(cp1251_aliases().keys().copied());
    codes.sort_unstable();
    codes.dedup();
    codes
}

pub fn patch_managed_strings(args: &[String]) -> Result<(), String> {
    let usage =
        "patch-managed-strings <extracted-unityweb-dir> <translation-json> [--allow-missing]";
    let root = required_path(args, 0, usage)?;
    let patch_path = required_path(args, 1, usage)?;
    let options = ManagedApplyOptions {
        allow_missing: has_flag(args, "--allow-missing"),
        backup: has_flag(args, "--backup"),
    };
    let spec = read_json(&patch_path)?;
    let entries = spec
        .get("entries")
        .and_then(JsonValue::as_array)
        .into_iter()
        .flatten()
        .filter(|entry| {
            entry.get("kind").and_then(JsonValue::as_str).unwrap_or("") == managed::MANAGED_KIND
        })
        .filter(|entry| has_text(entry.get("translation")))
        .count();
    if entries == 0 {
        println!("No filled managed translations to apply.");
        return Ok(());
    }
    managed::apply_translations(&root, &patch_path, &options).map(|_| ())
}

pub fn patch_resource_locator_character_bundles(args: &[String]) -> Result<(), String> {
    let usage =
        "patch-resource-locator-character-bundles <extracted-unityweb-dir> <bundle-name>...";
    let root = required_path(args, 0, usage)?;
    if args.len() < 2 {
        return Err(usage.to_string());
    }
    let count = managed::patch_resource_locator_character_bundles(&root, &args[1..])?;
    println!("Added {count} ResourceLocator character bundle entries.");
    Ok(())
}

pub fn patch_binary_reader_unicode_strings(args: &[String]) -> Result<(), String> {
    let usage = "patch-binary-reader-unicode-strings <extracted-unityweb-dir>";
    let root = required_path(args, 0, usage)?;
    let count = managed::patch_binary_reader_unicode_strings(&root)?;
    println!("Patched {count} BinaryReader Unicode string decode site(s).");
    Ok(())
}

pub fn patch_gui_fonts(args: &[String]) -> Result<(), String> {
    let usage = "patch-gui-fonts <input-asset> <output-asset> [--report path] [--ttf-font-dir dir] [--font-manifest path] [--vertical-offset n] [--preserve-ascii-glyphs] [--no-reference-redirect]";
    let input = required_path(args, 0, usage)?;
    let output = required_path(args, 1, usage)?;
    let report_path = option_value(args, "--report").map(PathBuf::from);
    let font_dir = option_value(args, "--ttf-font-dir").map(PathBuf::from);
    let fallback_ttf = option_value(args, "--ttf-font").map(PathBuf::from);
    let font_manifest_path = option_value(args, "--font-manifest").map(PathBuf::from);
    let face_name = option_value(args, "--font-face");
    let preserve_ascii = has_flag(args, "--preserve-ascii-glyphs");
    let no_redirect = has_flag(args, "--no-reference-redirect");
    let vertical_offset =
        option_value(args, "--vertical-offset").and_then(|value| value.parse::<f64>().ok());
    let asset = Asset::from_path(&input)?;
    let asset_index = 0usize;
    let manifest = if let Some(path) = font_manifest_path.as_ref() {
        let json = read_json(path)?;
        if json.get("format").and_then(JsonValue::as_str) != Some(FONT_MANIFEST_FORMAT) {
            return Err(format!(
                "Unsupported font manifest format in {}",
                path.display()
            ));
        }
        font_manifest_entries(&json, path.parent())?
    } else {
        HashMap::new()
    };
    let ttf_by_family = build_ttf_font_map(font_dir.as_deref())?;
    let fallback_face = if let Some(ttf) = fallback_ttf.as_ref() {
        Some(face_name.clone().unwrap_or(extract_ttf_family_name(ttf)?))
    } else {
        None
    };

    let mut replacements = BTreeMap::new();
    let mut ttf_reports = Vec::new();
    let font_path_ids = asset
        .objects
        .iter()
        .filter(|(_, info)| info.type_id == 128)
        .map(|(path_id, _)| *path_id)
        .collect::<Vec<_>>();
    for path_id in font_path_ids {
        let info = asset.objects.get(&path_id).cloned().unwrap();
        let mut font = asset.read_object(asset_index, &info)?;
        let font_name = object_name(&font);
        let family = normalize_font_family(&font_name);
        let manifest_entry = manifest.get(&family);
        if manifest_entry
            .and_then(|entry| entry.get("includeRussian"))
            .and_then(JsonValue::as_bool)
            == Some(false)
        {
            continue;
        }
        let ttf_path = manifest_entry
            .and_then(|entry| entry.get("resolvedFontPath"))
            .and_then(JsonValue::as_str)
            .map(PathBuf::from)
            .or_else(|| ttf_by_family.get(&family).cloned())
            .or_else(|| fallback_ttf.clone());
        let Some(ttf_path) = ttf_path else {
            continue;
        };
        let resolved_face = manifest_entry
            .and_then(|entry| entry.get("fontFace"))
            .and_then(JsonValue::as_str)
            .filter(|value| !value.trim().is_empty())
            .map(str::to_string)
            .or_else(|| {
                if fallback_ttf.as_ref() == Some(&ttf_path) {
                    fallback_face.clone()
                } else {
                    None
                }
            })
            .unwrap_or(extract_ttf_family_name(&ttf_path)?);
        let replace_ascii = manifest_entry
            .and_then(|entry| entry.get("replaceAscii"))
            .and_then(JsonValue::as_bool)
            .unwrap_or(!preserve_ascii);
        let effective_vertical_offset = manifest_entry
            .and_then(|entry| entry.get("verticalOffset"))
            .and_then(JsonValue::as_f64)
            .or(vertical_offset);
        match patch_font_value(
            &asset,
            asset_index,
            &mut font,
            &ttf_path,
            &resolved_face,
            replace_ascii,
            effective_vertical_offset,
        ) {
            Ok(report) => {
                if let Some(texture) = font
                    .as_object_mut()
                    .and_then(|object| object.remove("__fftools_texture_patch"))
                {
                    if let Some(texture_path_id) = font
                        .get("m_Texture")
                        .and_then(UnityValue::as_pointer)
                        .map(|pointer| pointer.path_id)
                    {
                        replacements.insert(texture_path_id, texture);
                    }
                }
                replacements.insert(path_id, font);
                ttf_reports.push(report);
            }
            Err(err) => {
                ttf_reports
                    .push(json!({ "font_path_id": path_id, "font_name": font_name, "error": err }));
            }
        }
    }

    let mut font_info = HashMap::new();
    for (path_id, info) in &asset.objects {
        if info.type_id == 128 {
            let value = replacements
                .get(path_id)
                .cloned()
                .unwrap_or(asset.read_object(asset_index, info)?);
            font_info.insert(*path_id, font_summary(&value));
        }
    }
    let font_map = build_font_redirect_map(&font_info);
    let mut changes = Vec::new();
    if !no_redirect && !font_map.is_empty() {
        let object_ids = asset.objects.keys().copied().collect::<Vec<_>>();
        for path_id in object_ids {
            let info = asset.objects.get(&path_id).cloned().unwrap();
            let mut value = replacements
                .get(&path_id)
                .cloned()
                .unwrap_or(asset.read_object(asset_index, &info)?);
            let mut object_changes = Vec::new();
            redirect_font_pointers(&mut value, &font_map, "", &mut object_changes);
            if !object_changes.is_empty() {
                replacements.insert(path_id, value);
                changes.extend(object_changes.into_iter().map(|change| {
                    json!({ "object_path_id": path_id, "field": change.0, "old_font_path_id": change.1, "new_font_path_id": change.2 })
                }));
            }
        }
    }

    let mut skin_style_font_changes = Vec::new();
    let object_ids = asset.objects.keys().copied().collect::<Vec<_>>();
    for path_id in object_ids {
        let info = asset.objects.get(&path_id).cloned().unwrap();
        let mut value = replacements
            .get(&path_id)
            .cloned()
            .unwrap_or(asset.read_object(asset_index, &info)?);
        if !looks_like_gui_skin(&value) {
            continue;
        }
        let Some(default_font) = value.get("m_Font").cloned() else {
            continue;
        };
        let Some(default_font_pointer) = default_font.as_pointer() else {
            continue;
        };
        let mut object_changes = Vec::new();
        assign_default_font_to_null_gui_styles(&mut value, &default_font, "", &mut object_changes);
        if object_changes.is_empty() {
            continue;
        }
        let skin_name = object_name(&value);
        replacements.insert(path_id, value);
        skin_style_font_changes.extend(object_changes.into_iter().map(|field| {
            json!({
                "object_path_id": path_id,
                "skin_name": skin_name,
                "field": field,
                "font_path_id": default_font_pointer.path_id,
            })
        }));
    }

    let bytes = if replacements.is_empty() {
        asset.data.clone()
    } else {
        asset.rebuild_with_object_values(asset_index, &replacements)?
    };
    fs::write(&output, bytes).map_err(|err| format!("{}: {err}", output.display()))?;
    let report = json!({
        "input_asset": input,
        "output_asset": output,
        "ttf_font_dir": font_dir,
        "replace_ascii_glyphs": !preserve_ascii,
        "ttf_patch": ttf_reports,
        "font_map": font_map,
        "reference_redirects": changes,
        "skin_style_font_assignments": skin_style_font_changes,
    });
    if let Some(path) = report_path {
        write_status(Some(&path), report)?;
    }
    println!("patched GUI fonts in {}", output.display());
    Ok(())
}

pub(super) fn patch_font_value(
    asset: &Asset,
    asset_index: usize,
    font: &mut UnityValue,
    ttf_path: &Path,
    face_name: &str,
    replace_ascii: bool,
    vertical_offset: Option<f64>,
) -> Result<JsonValue, String> {
    let texture_pointer = font
        .get("m_Texture")
        .and_then(UnityValue::as_pointer)
        .ok_or_else(|| "Font has no m_Texture pointer".to_string())?
        .clone();
    if texture_pointer.file_id != 0 {
        return Err("external font texture pointers are not supported in build patch".to_string());
    }
    let texture_info = asset
        .objects
        .get(&texture_pointer.path_id)
        .ok_or_else(|| format!("texture pathId {} was not found", texture_pointer.path_id))?;
    let mut texture = asset.read_object(asset_index, texture_info)?;
    let old_width = texture
        .get("m_Width")
        .and_then(UnityValue::as_i64)
        .unwrap_or(0) as usize;
    let old_height = texture
        .get("m_Height")
        .and_then(UnityValue::as_i64)
        .unwrap_or(0) as usize;
    let logical = texture_to_logical_alpha(&texture)?;
    let line_spacing = font
        .get("m_LineSpacing")
        .and_then(UnityValue::as_f64)
        .unwrap_or(16.0);
    let pixel_height = line_spacing.round().max(1.0) as i32;
    let codes = default_patch_codes(replace_ascii);
    let aliases = cp1251_aliases();
    let glyphs = render_glyphs(ttf_path, face_name, pixel_height, &codes, &aliases)?;
    if glyphs.is_empty() {
        return Err("No glyphs could be rendered from the supplied TTF".to_string());
    }

    let existing_rects = font
        .get("m_CharacterRects")
        .and_then(UnityValue::as_array)
        .ok_or_else(|| "Font has no m_CharacterRects".to_string())?
        .to_vec();
    let patch_codes = codes.iter().copied().collect::<BTreeSet<_>>();
    let mut existing_by_code = BTreeMap::new();
    let mut existing_positions = BTreeMap::new();
    for rect in &existing_rects {
        if let Some(code) = rect
            .get("index")
            .and_then(UnityValue::as_i64)
            .map(|value| value as u32)
        {
            existing_by_code.insert(code, rect.clone());
            existing_positions.insert(code, rect_pixels(rect, old_width, old_height));
        }
    }
    let kept_rects = existing_rects
        .into_iter()
        .filter(|rect| {
            rect.get("index")
                .and_then(UnityValue::as_i64)
                .map(|code| !patch_codes.contains(&(code as u32)))
                .unwrap_or(true)
        })
        .collect::<Vec<_>>();
    let start_y = existing_positions
        .values()
        .map(|(_, y, _, h)| y + h + 2)
        .max()
        .unwrap_or(2);
    let mut atlas_width = next_power_of_two(old_width.max(64));
    let mut atlas_height = next_power_of_two(old_height.max(start_y + 3));
    let mut reused = BTreeMap::new();
    let mut to_pack = Vec::new();
    for glyph in &glyphs {
        if let Some((x, y, w, h)) = existing_positions.get(&glyph.code).copied() {
            if glyph.width <= w && glyph.height <= h {
                reused.insert(glyph.code, (x, y));
                continue;
            }
        }
        to_pack.push(glyph.clone());
    }
    let packed = loop {
        if atlas_width > 4096 || atlas_height > 4096 {
            return Err(format!(
                "Could not pack {} glyphs into a 4096x4096 texture",
                to_pack.len()
            ));
        }
        if let Some(packed) = try_pack_glyphs(&to_pack, start_y, atlas_width, atlas_height, 2) {
            break packed;
        }
        if atlas_height <= atlas_width {
            atlas_height *= 2;
        } else {
            atlas_width *= 2;
        }
    };
    let mut new_logical = vec![0u8; atlas_width * atlas_height];
    for y in 0..old_height {
        let source = y * old_width;
        let target = y * atlas_width;
        new_logical[target..target + old_width]
            .copy_from_slice(&logical[source..source + old_width]);
    }
    let mut rects = kept_rects
        .into_iter()
        .map(|mut rect| {
            if let Some(code) = rect
                .get("index")
                .and_then(UnityValue::as_i64)
                .map(|value| value as u32)
            {
                if let Some((x, y, w, h)) = existing_positions.get(&code).copied() {
                    set_rect_uv(&mut rect, x, y, w, h, atlas_width, atlas_height);
                }
            }
            rect
        })
        .collect::<Vec<_>>();
    for glyph in &glyphs {
        let (x, y) = reused
            .get(&glyph.code)
            .copied()
            .unwrap_or_else(|| packed[&glyph.code]);
        if let Some((old_x, old_y, old_w, old_h)) = existing_positions.get(&glyph.code).copied() {
            for clear_y in old_y..(old_y + old_h).min(atlas_height) {
                let target = clear_y * atlas_width + old_x;
                let clear_width = old_w.min(atlas_width.saturating_sub(old_x));
                new_logical[target..target + clear_width].fill(0);
            }
        }
        for (row_index, row) in glyph.rows.iter().enumerate() {
            let target = (y + row_index) * atlas_width + x;
            new_logical[target..target + glyph.width].copy_from_slice(row);
        }
        rects.push(make_character_rect(
            glyph,
            x,
            y,
            atlas_width,
            atlas_height,
            line_spacing,
            vertical_offset.unwrap_or(0.0),
        ));
    }
    rects.sort_by_key(|rect| rect.get("index").and_then(UnityValue::as_i64).unwrap_or(0));
    *font
        .get_mut("m_CharacterRects")
        .ok_or_else(|| "Font has no m_CharacterRects".to_string())? = UnityValue::Array(rects);
    set_object_field(&mut texture, "m_Width", UnityValue::Int(atlas_width as i64))?;
    set_object_field(
        &mut texture,
        "m_Height",
        UnityValue::Int(atlas_height as i64),
    )?;
    set_object_field(&mut texture, "m_TextureFormat", UnityValue::Int(1))?;
    set_object_field(&mut texture, "m_MipMap", UnityValue::Bool(false))?;
    set_object_field(&mut texture, "m_ImageCount", UnityValue::Int(1))?;
    set_object_field(
        &mut texture,
        "m_CompleteImageSize",
        UnityValue::Int(new_logical.len() as i64),
    )?;
    set_object_field(
        &mut texture,
        "image data",
        UnityValue::Bytes(logical_alpha_to_texture_data(
            &new_logical,
            atlas_width,
            atlas_height,
        )),
    )?;
    set_object_field(font, "__fftools_texture_patch", texture)?;
    Ok(json!({
        "font_name": object_name(font),
        "texture_path_id": texture_pointer.path_id,
        "old_texture_size": [old_width, old_height],
        "new_texture_size": [atlas_width, atlas_height],
        "line_spacing": line_spacing,
        "pixel_height": pixel_height,
        "patched_codes": glyphs.iter().map(|glyph| glyph.code).collect::<Vec<_>>(),
        "cp1251_alias_codes": glyphs.iter().filter(|glyph| glyph.code != glyph.render_code).map(|glyph| glyph.code).collect::<Vec<_>>(),
        "reused_slots": reused.len(),
        "packed_slots": to_pack.len(),
    }))
}
