use super::*;

pub fn convert_legacy_gui_skins(
    options: &LegacyGuiSkinConversionOptions,
) -> Result<LegacyGuiSkinConversionReport> {
    let bytes = fs::read(&options.dump_object_all_json)
        .map_err(|source| io_at(&options.dump_object_all_json, source))?;
    let objects: Vec<Value> =
        serde_json::from_slice(&bytes).map_err(|source| PipelineError::Json {
            path: options.dump_object_all_json.display().to_string(),
            source,
        })?;
    let identities = object_identities(&objects);
    let expected = TARGET_GUI_SKINS.iter().copied().collect::<BTreeSet<_>>();
    let mut found = BTreeSet::new();
    let mut source_assets = BTreeSet::new();
    let mut skins = Vec::new();
    let mut diagnostics = Vec::new();

    for object in &objects {
        let value = object.get("value").unwrap_or(&Value::Null);
        let name = object_name(object, value);
        if !expected.contains(name.as_str()) {
            continue;
        }
        require_gui_skin_type(object, &name)?;
        if !found.insert(name.clone()) {
            return Err(PipelineError::InvalidManifest(format!(
                "legacy GUI skin candidate contains more than one GUISkin named {name:?}"
            )));
        }
        diagnose_expected_field(
            &mut diagnostics,
            &name,
            None,
            "value",
            object.get("value"),
            "object",
            Value::is_object,
        );
        diagnose_expected_field(
            &mut diagnostics,
            &name,
            None,
            "pathId",
            object.get("pathId"),
            "integer",
            is_integer,
        );
        let source_asset = string_field(object, "asset");
        if source_asset.is_empty() {
            push_candidate_diagnostic(
                &mut diagnostics,
                "missingOwnerContext",
                &name,
                None,
                "asset",
                "serialized asset name is absent; local PPtr targets cannot be proven",
            );
        } else {
            source_assets.insert(source_asset.clone());
            diagnose_pointer_field(&mut diagnostics, &name, None, "m_Font", value.get("m_Font"));
        }

        let mut built_in_styles = BTreeMap::new();
        for (semantic_name, serialized_name) in BUILTIN_STYLES {
            diagnose_expected_field(
                &mut diagnostics,
                &name,
                Some(semantic_name),
                serialized_name,
                value.get(*serialized_name),
                "object",
                Value::is_object,
            );
            if let Some(style) = value
                .get(*serialized_name)
                .filter(|style| style.is_object())
            {
                diagnose_style_fields(&mut diagnostics, &name, semantic_name, style);
                built_in_styles.insert(
                    (*semantic_name).to_owned(),
                    convert_style(semantic_name, style, &source_asset, &identities),
                );
            }
        }
        diagnose_expected_field(
            &mut diagnostics,
            &name,
            None,
            "customStyles",
            value.get("customStyles"),
            "array",
            Value::is_array,
        );
        let custom_styles =
            convert_custom_styles(&mut diagnostics, &name, value, &source_asset, &identities);
        skins.push(LegacyGuiSkin {
            name,
            path_id: integer_field(object, "pathId"),
            font: pointer(
                value.get("m_Font").unwrap_or(&Value::Null),
                &source_asset,
                &identities,
            ),
            source_asset,
            built_in_styles,
            custom_styles,
        });
    }

    let missing = expected
        .difference(&found.iter().map(String::as_str).collect::<BTreeSet<_>>())
        .copied()
        .collect::<Vec<_>>();
    if !missing.is_empty() {
        return Err(PipelineError::InvalidManifest(format!(
            "legacy GUI skin candidate is missing clean Retrobution GUISkins: {}",
            missing.join(", ")
        )));
    }

    skins.sort_by(|left, right| left.name.cmp(&right.name));
    let unresolved_pointer_count = append_unresolved_pointer_diagnostics(&skins, &mut diagnostics);
    diagnostics.sort();
    let candidate = LegacyGuiSkinCandidate {
        schema: LEGACY_GUI_SKIN_SCHEMA.to_owned(),
        evidence_level: LEGACY_GUI_SKIN_EVIDENCE_LEVEL.to_owned(),
        publication_allowed: false,
        limitations: LEGACY_GUI_SKIN_LIMITATIONS
            .iter()
            .map(|limitation| (*limitation).to_owned())
            .collect(),
        source_build: options.source_build.clone(),
        source_assets: source_assets.into_iter().collect(),
        unresolved_pointer_count,
        diagnostics,
        skins,
    };
    let styles = candidate
        .skins
        .iter()
        .map(|skin| skin.built_in_styles.len() + skin.custom_styles.len())
        .sum();
    let (referenced_textures, referenced_fonts) = referenced_asset_counts(&candidate);
    let encoded = serde_json::to_vec_pretty(&candidate).map_err(|source| PipelineError::Json {
        path: options.output_json.display().to_string(),
        source,
    })?;
    if let Some(parent) = options
        .output_json
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent).map_err(|source| io_at(parent, source))?;
    }
    fs::write(&options.output_json, encoded)
        .map_err(|source| io_at(&options.output_json, source))?;

    Ok(LegacyGuiSkinConversionReport {
        source_build: options.source_build.clone(),
        evidence_level: candidate.evidence_level.clone(),
        publication_allowed: candidate.publication_allowed,
        skins: candidate.skins.len(),
        styles,
        referenced_textures,
        referenced_fonts,
        unresolved_pointer_count: candidate.unresolved_pointer_count,
        diagnostics: candidate.diagnostics.len(),
        output: normalize_path(&options.output_json),
    })
}

pub(super) fn convert_custom_styles(
    diagnostics: &mut Vec<LegacyGuiSkinCandidateDiagnostic>,
    skin_name: &str,
    skin_value: &Value,
    source_asset: &str,
    identities: &ObjectIdentityIndex,
) -> Vec<LegacyGuiStyle> {
    let Some(styles) = skin_value.get("customStyles").and_then(Value::as_array) else {
        return Vec::new();
    };
    let mut converted = Vec::new();
    for (index, style) in styles.iter().enumerate() {
        let entry_field = format!("customStyles[{index}]");
        if !diagnose_expected_field(
            diagnostics,
            skin_name,
            None,
            &entry_field,
            Some(style),
            "object",
            Value::is_object,
        ) {
            continue;
        }
        let style_name = string_field(style, "m_Name");
        let diagnostic_style = if style_name.is_empty() {
            entry_field.clone()
        } else {
            style_name.clone()
        };
        let name_is_string = diagnose_expected_field(
            diagnostics,
            skin_name,
            Some(&diagnostic_style),
            "m_Name",
            style.get("m_Name"),
            "string",
            Value::is_string,
        );
        if name_is_string && style_name.is_empty() {
            push_candidate_diagnostic(
                diagnostics,
                "emptyField",
                skin_name,
                Some(&diagnostic_style),
                "m_Name",
                "custom GUIStyle name is empty; semantic identity is unresolved",
            );
        }
        diagnose_style_fields(diagnostics, skin_name, &diagnostic_style, style);
        converted.push(convert_style(&style_name, style, source_asset, identities));
    }
    converted
}

pub(super) fn convert_style(
    name: &str,
    style: &Value,
    source_asset: &str,
    identities: &ObjectIdentityIndex,
) -> LegacyGuiStyle {
    let states = STYLE_STATES
        .iter()
        .filter_map(|(semantic_name, serialized_name)| {
            style
                .get(*serialized_name)
                .filter(|state| state.is_object())
                .map(|state| {
                    (
                        (*semantic_name).to_owned(),
                        LegacyGuiStyleState {
                            background: pointer(
                                state.get("m_Background").unwrap_or(&Value::Null),
                                source_asset,
                                identities,
                            ),
                            text_color: color(state.get("m_TextColor").unwrap_or(&Value::Null)),
                        },
                    )
                })
        })
        .collect();
    LegacyGuiStyle {
        name: name.to_owned(),
        font: pointer(
            style.get("m_Font").unwrap_or(&Value::Null),
            source_asset,
            identities,
        ),
        states,
        border: insets(style.get("m_Border").unwrap_or(&Value::Null)),
        margin: insets(style.get("m_Margin").unwrap_or(&Value::Null)),
        padding: insets(style.get("m_Padding").unwrap_or(&Value::Null)),
        overflow: insets(style.get("m_Overflow").unwrap_or(&Value::Null)),
        content_offset: vector2(style.get("m_ContentOffset").unwrap_or(&Value::Null)),
        image_position: integer_field(style, "m_ImagePosition"),
        alignment: integer_field(style, "m_Alignment"),
        word_wrap: integer_field(style, "m_WordWrap"),
        text_clipping: integer_field(style, "m_TextClipping"),
        fixed_width: number_field(style, "m_FixedWidth"),
        fixed_height: number_field(style, "m_FixedHeight"),
        stretch_width: integer_field(style, "m_StretchWidth"),
        stretch_height: integer_field(style, "m_StretchHeight"),
    }
}
