use super::*;

pub(super) fn cp1251_aliases() -> BTreeMap<u32, u32> {
    let mut result = BTreeMap::new();
    for index in 0..0x10 {
        result.insert(0x460 + index, 0x410 + index);
    }
    result.insert(0x470, 0x401);
    for index in 0..0x20 {
        result.insert(0xc0 + index, 0x410 + index);
        result.insert(0xe0 + index, 0x430 + index);
    }
    result.insert(0xa8, 0x401);
    result.insert(0xb8, 0x451);
    result.insert(0xb9, 0x401);
    for index in 0..7 {
        result.insert(0xa1 + index, 0x410 + index);
    }
    for index in 7..0x10 {
        result.insert(0xa9 + index - 7, 0x410 + index);
    }
    result.insert(0xb2, 0x44f);
    result
}

pub(super) fn font_summary(value: &UnityValue) -> FontSummary {
    let rects = value
        .get("m_CharacterRects")
        .and_then(UnityValue::as_array)
        .unwrap_or(&[]);
    let codes = rects
        .iter()
        .filter_map(|rect| {
            rect.get("index")
                .and_then(UnityValue::as_i64)
                .map(|value| value as u32)
        })
        .collect::<BTreeSet<_>>();
    FontSummary {
        name: object_name(value),
        has_cyrillic: RUSSIAN_CODES.iter().all(|code| codes.contains(code)),
        line_spacing: value
            .get("m_LineSpacing")
            .and_then(UnityValue::as_f64)
            .unwrap_or(0.0),
    }
}

pub(super) fn build_font_redirect_map(font_info: &HashMap<i64, FontSummary>) -> BTreeMap<i64, i64> {
    let mut result = BTreeMap::new();
    for (path_id, info) in font_info {
        if info.has_cyrillic {
            continue;
        }
        let fallback_name = match info.name.as_str() {
            "ChaletBook-Regular Small 1" => "ChaletBook-Regular Small",
            "JEFFE___72" => "JEFFE___40",
            _ => &info.name,
        };
        let candidate = font_info
            .iter()
            .filter(|(_, candidate)| candidate.has_cyrillic && candidate.name == fallback_name)
            .min_by(|(_, a), (_, b)| {
                (a.line_spacing - info.line_spacing)
                    .abs()
                    .partial_cmp(&(b.line_spacing - info.line_spacing).abs())
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
        if let Some((target, _)) = candidate {
            result.insert(*path_id, *target);
        }
    }
    result
}

pub(super) fn redirect_font_pointers(
    value: &mut UnityValue,
    font_map: &BTreeMap<i64, i64>,
    path: &str,
    changes: &mut Vec<(String, i64, i64)>,
) {
    match value {
        UnityValue::Pointer(pointer) if pointer.file_id == 0 => {
            if let Some(new_path_id) = font_map.get(&pointer.path_id) {
                let old = pointer.path_id;
                pointer.path_id = *new_path_id;
                changes.push((path.to_string(), old, *new_path_id));
            }
        }
        UnityValue::Object(fields) => {
            for (key, child) in fields {
                let child_path = if path.is_empty() {
                    key.clone()
                } else {
                    format!("{path}.{key}")
                };
                redirect_font_pointers(child, font_map, &child_path, changes);
            }
        }
        UnityValue::Array(items) => {
            for (index, child) in items.iter_mut().enumerate() {
                redirect_font_pointers(child, font_map, &format!("{path}[{index}]"), changes);
            }
        }
        UnityValue::Pair(left, right) => {
            redirect_font_pointers(left, font_map, &format!("{path}(0)"), changes);
            redirect_font_pointers(right, font_map, &format!("{path}(1)"), changes);
        }
        _ => {}
    }
}

pub(super) fn looks_like_gui_skin(value: &UnityValue) -> bool {
    matches!(
        value.get("m_Font"),
        Some(UnityValue::Pointer(pointer)) if !pointer.is_null()
    ) && value.get("m_textField").is_some()
        && value.get("m_Settings").is_some()
        && value.get("m_box").is_some()
}

pub(super) fn assign_default_font_to_null_gui_styles(
    value: &mut UnityValue,
    default_font: &UnityValue,
    path: &str,
    changes: &mut Vec<String>,
) {
    match value {
        UnityValue::Object(fields) => {
            let is_null_style_font = matches!(
                fields.get("m_Font"),
                Some(UnityValue::Pointer(pointer)) if pointer.is_null()
            );
            if is_null_style_font
                && fields.contains_key("m_Normal")
                && fields.contains_key("m_TextClipping")
            {
                fields.insert("m_Font".to_string(), default_font.clone());
                changes.push(path.to_string());
            }

            for (key, child) in fields.iter_mut() {
                if key == "m_Font" {
                    continue;
                }
                let child_path = if path.is_empty() {
                    key.clone()
                } else {
                    format!("{path}.{key}")
                };
                assign_default_font_to_null_gui_styles(child, default_font, &child_path, changes);
            }
        }
        UnityValue::Array(items) => {
            for (index, child) in items.iter_mut().enumerate() {
                assign_default_font_to_null_gui_styles(
                    child,
                    default_font,
                    &format!("{path}[{index}]"),
                    changes,
                );
            }
        }
        UnityValue::Pair(left, right) => {
            assign_default_font_to_null_gui_styles(
                left,
                default_font,
                &format!("{path}(0)"),
                changes,
            );
            assign_default_font_to_null_gui_styles(
                right,
                default_font,
                &format!("{path}(1)"),
                changes,
            );
        }
        _ => {}
    }
}

pub(super) fn rect_pixels(
    rect: &UnityValue,
    texture_width: usize,
    texture_height: usize,
) -> (usize, usize, usize, usize) {
    let uv = rect.get("uv");
    let x = uv
        .and_then(|v| v.get("x"))
        .and_then(UnityValue::as_f64)
        .unwrap_or(0.0)
        * texture_width as f64;
    let w = uv
        .and_then(|v| v.get("width"))
        .and_then(UnityValue::as_f64)
        .unwrap_or(0.0)
        * texture_width as f64;
    let h = uv
        .and_then(|v| v.get("height"))
        .and_then(UnityValue::as_f64)
        .unwrap_or(0.0)
        * texture_height as f64;
    let y = texture_height as f64
        - (uv
            .and_then(|v| v.get("y"))
            .and_then(UnityValue::as_f64)
            .unwrap_or(0.0)
            * texture_height as f64
            + h);
    (
        x.round().max(0.0) as usize,
        y.round().max(0.0).min(texture_height as f64) as usize,
        w.round() as usize,
        h.round() as usize,
    )
}

pub(super) fn set_rect_uv(
    rect: &mut UnityValue,
    x: usize,
    y: usize,
    width: usize,
    height: usize,
    atlas_width: usize,
    atlas_height: usize,
) {
    if let Some(uv) = rect.get_mut("uv").and_then(UnityValue::as_object_mut) {
        uv.insert(
            "x".to_string(),
            UnityValue::Float(x as f64 / atlas_width as f64),
        );
        uv.insert(
            "y".to_string(),
            UnityValue::Float((atlas_height - y - height) as f64 / atlas_height as f64),
        );
        uv.insert(
            "width".to_string(),
            UnityValue::Float(width as f64 / atlas_width as f64),
        );
        uv.insert(
            "height".to_string(),
            UnityValue::Float(height as f64 / atlas_height as f64),
        );
    }
}

pub(super) fn make_character_rect(
    glyph: &GlyphBitmap,
    x: usize,
    y: usize,
    atlas_width: usize,
    atlas_height: usize,
    line_spacing: f64,
    vertical_offset: f64,
) -> UnityValue {
    let mut uv = BTreeMap::new();
    uv.insert(
        "x".to_string(),
        UnityValue::Float(x as f64 / atlas_width as f64),
    );
    uv.insert(
        "y".to_string(),
        UnityValue::Float((atlas_height - y - glyph.height) as f64 / atlas_height as f64),
    );
    uv.insert(
        "width".to_string(),
        UnityValue::Float(glyph.width as f64 / atlas_width as f64),
    );
    uv.insert(
        "height".to_string(),
        UnityValue::Float(glyph.height as f64 / atlas_height as f64),
    );
    let vert_x = glyph.origin_x.max(0) as f64;
    let ink_right = vert_x + glyph.width as f64;
    let advance = (glyph.advance as f64).max(ink_right);
    let mut vert = BTreeMap::new();
    vert.insert("x".to_string(), UnityValue::Float(vert_x));
    vert.insert(
        "y".to_string(),
        UnityValue::Float(-0.0_f64.max(line_spacing - glyph.origin_y as f64) + vertical_offset),
    );
    vert.insert("width".to_string(), UnityValue::Float(glyph.width as f64));
    vert.insert(
        "height".to_string(),
        UnityValue::Float(-(glyph.height as f64)),
    );
    UnityValue::Object(BTreeMap::from([
        ("index".to_string(), UnityValue::Int(glyph.code as i64)),
        ("uv".to_string(), UnityValue::Object(uv)),
        ("vert".to_string(), UnityValue::Object(vert)),
        ("width".to_string(), UnityValue::Float(advance)),
    ]))
}

pub(super) fn try_pack_glyphs(
    glyphs: &[GlyphBitmap],
    start_y: usize,
    atlas_width: usize,
    atlas_height: usize,
    padding: usize,
) -> Option<BTreeMap<u32, (usize, usize)>> {
    let mut positions = BTreeMap::new();
    let mut x = padding;
    let mut y = start_y;
    let mut row_height = 0usize;
    for glyph in glyphs {
        if glyph.width == 0 || glyph.height == 0 {
            positions.insert(glyph.code, (x, y));
            continue;
        }
        if glyph.width + padding * 2 > atlas_width {
            return None;
        }
        if x + glyph.width + padding > atlas_width {
            x = padding;
            y += row_height + padding;
            row_height = 0;
        }
        if y + glyph.height + padding > atlas_height {
            return None;
        }
        positions.insert(glyph.code, (x, y));
        x += glyph.width + padding;
        row_height = row_height.max(glyph.height);
    }
    Some(positions)
}

pub(super) fn next_power_of_two(value: usize) -> usize {
    value.max(1).next_power_of_two()
}

pub(super) fn build_ttf_font_map(font_dir: Option<&Path>) -> Result<HashMap<String, PathBuf>, String> {
    let mut result = HashMap::new();
    let Some(font_dir) = font_dir else {
        return Ok(result);
    };
    for entry in fs::read_dir(font_dir).map_err(|err| format!("{}: {err}", font_dir.display()))? {
        let path = entry.map_err(|err| err.to_string())?.path();
        let ext = path
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        if ext == "ttf" || ext == "otf" {
            let stem = path
                .file_stem()
                .and_then(|value| value.to_str())
                .unwrap_or("");
            result.insert(normalize_font_family(stem), path);
        }
    }
    Ok(result)
}

pub(super) fn normalize_font_family(name: &str) -> String {
    let mut value = name.trim().to_string();
    for suffix in [" Small 1", " Small"] {
        if value.ends_with(suffix) {
            value.truncate(value.len() - suffix.len());
        }
    }
    while value
        .chars()
        .last()
        .is_some_and(|ch| ch.is_ascii_digit() || ch == '_' || ch == '-' || ch == ' ')
    {
        value.pop();
    }
    if value.to_ascii_uppercase().starts_with("JEFFE") {
        "JEFFE".to_string()
    } else {
        value
    }
}

pub(super) fn extract_ttf_family_name(path: &Path) -> Result<String, String> {
    let data = fs::read(path).map_err(|err| format!("{}: {err}", path.display()))?;
    let mut reader = BinaryReader::from_slice(&data, Endian::Big);
    reader.skip(4)?;
    let table_count = reader.read_u16()? as usize;
    reader.skip(6)?;
    let mut name_offset = None;
    let mut name_length = 0usize;
    for _ in 0..table_count {
        let tag = reader.read_exact_vec(4)?;
        let _checksum = reader.read_u32()?;
        let offset = reader.read_u32()? as usize;
        let length = reader.read_u32()? as usize;
        if tag == b"name" {
            name_offset = Some(offset);
            name_length = length;
        }
    }
    let offset =
        name_offset.ok_or_else(|| format!("TTF name table was not found: {}", path.display()))?;
    let table = data
        .get(offset..offset + name_length)
        .ok_or_else(|| "TTF name table is out of range".to_string())?;
    let mut reader = BinaryReader::from_slice(table, Endian::Big);
    let _format = reader.read_u16()?;
    let count = reader.read_u16()? as usize;
    let string_offset = reader.read_u16()? as usize;
    let mut candidates = Vec::new();
    for _ in 0..count {
        let platform_id = reader.read_u16()?;
        let encoding_id = reader.read_u16()?;
        let language_id = reader.read_u16()?;
        let name_id = reader.read_u16()?;
        let length = reader.read_u16()? as usize;
        let offset = reader.read_u16()? as usize;
        if name_id != 1 && name_id != 4 {
            continue;
        }
        let Some(bytes) = table.get(string_offset + offset..string_offset + offset + length) else {
            continue;
        };
        let value = if platform_id == 0 || platform_id == 3 {
            let units = bytes
                .chunks_exact(2)
                .map(|chunk| u16::from_be_bytes([chunk[0], chunk[1]]))
                .collect::<Vec<_>>();
            String::from_utf16_lossy(&units)
        } else {
            String::from_utf8_lossy(bytes).to_string()
        };
        if value.trim().is_empty() {
            continue;
        }
        let mut score = 0;
        if name_id == 1 {
            score += 100;
        }
        if platform_id == 3 {
            score += 50;
        }
        if language_id == 0x409 || language_id == 0 {
            score += 10;
        }
        if encoding_id == 1 || encoding_id == 10 {
            score += 5;
        }
        candidates.push((score, value));
    }
    candidates
        .into_iter()
        .max_by_key(|item| item.0)
        .map(|item| item.1)
        .ok_or_else(|| format!("TTF family name was not found: {}", path.display()))
}

pub(super) fn option_value(args: &[String], name: &str) -> Option<String> {
    args.windows(2)
        .find(|pair| pair[0] == name)
        .map(|pair| pair[1].clone())
}

pub(super) fn has_flag(args: &[String], name: &str) -> bool {
    args.iter().any(|arg| arg == name)
}

pub(super) fn has_text(value: Option<&JsonValue>) -> bool {
    value
        .and_then(JsonValue::as_str)
        .is_some_and(|value| !value.is_empty())
}

pub(super) fn entry_i64(entry: &JsonValue, key: &str) -> Result<i64, String> {
    entry
        .get(key)
        .and_then(JsonValue::as_i64)
        .ok_or_else(|| format!("entry has no integer {key}"))
}
