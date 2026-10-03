use super::*;

pub(super) fn diagnose_style_fields(
    diagnostics: &mut Vec<LegacyGuiSkinCandidateDiagnostic>,
    skin_name: &str,
    style_name: &str,
    style: &Value,
) {
    diagnose_pointer_field(
        diagnostics,
        skin_name,
        Some(style_name),
        "m_Font",
        style.get("m_Font"),
    );

    for (_, serialized_state) in STYLE_STATES {
        if diagnose_expected_field(
            diagnostics,
            skin_name,
            Some(style_name),
            serialized_state,
            style.get(*serialized_state),
            "object",
            Value::is_object,
        ) {
            let state = &style[*serialized_state];
            let background = format!("{serialized_state}.m_Background");
            diagnose_pointer_field(
                diagnostics,
                skin_name,
                Some(style_name),
                &background,
                state.get("m_Background"),
            );
            let color = format!("{serialized_state}.m_TextColor");
            if diagnose_expected_field(
                diagnostics,
                skin_name,
                Some(style_name),
                &color,
                state.get("m_TextColor"),
                "object",
                Value::is_object,
            ) {
                for component in ["r", "g", "b", "a"] {
                    let field = format!("{color}.{component}");
                    diagnose_expected_field(
                        diagnostics,
                        skin_name,
                        Some(style_name),
                        &field,
                        state["m_TextColor"].get(component),
                        "number",
                        Value::is_number,
                    );
                }
            }
        }
    }

    for field_name in ["m_Border", "m_Margin", "m_Padding", "m_Overflow"] {
        if diagnose_expected_field(
            diagnostics,
            skin_name,
            Some(style_name),
            field_name,
            style.get(field_name),
            "object",
            Value::is_object,
        ) {
            for side in ["m_Left", "m_Right", "m_Top", "m_Bottom"] {
                let field = format!("{field_name}.{side}");
                diagnose_expected_field(
                    diagnostics,
                    skin_name,
                    Some(style_name),
                    &field,
                    style[field_name].get(side),
                    "integer",
                    is_integer,
                );
            }
        }
    }

    if diagnose_expected_field(
        diagnostics,
        skin_name,
        Some(style_name),
        "m_ContentOffset",
        style.get("m_ContentOffset"),
        "object",
        Value::is_object,
    ) {
        for component in ["x", "y"] {
            let field = format!("m_ContentOffset.{component}");
            diagnose_expected_field(
                diagnostics,
                skin_name,
                Some(style_name),
                &field,
                style["m_ContentOffset"].get(component),
                "number",
                Value::is_number,
            );
        }
    }

    for field in [
        "m_ImagePosition",
        "m_Alignment",
        "m_WordWrap",
        "m_TextClipping",
        "m_StretchWidth",
        "m_StretchHeight",
    ] {
        diagnose_expected_field(
            diagnostics,
            skin_name,
            Some(style_name),
            field,
            style.get(field),
            "integer",
            is_integer,
        );
    }
    for field in ["m_FixedWidth", "m_FixedHeight"] {
        diagnose_expected_field(
            diagnostics,
            skin_name,
            Some(style_name),
            field,
            style.get(field),
            "number",
            Value::is_number,
        );
    }
}

pub(super) fn diagnose_pointer_field(
    diagnostics: &mut Vec<LegacyGuiSkinCandidateDiagnostic>,
    skin_name: &str,
    style_name: Option<&str>,
    field: &str,
    value: Option<&Value>,
) {
    if !diagnose_expected_field(
        diagnostics,
        skin_name,
        style_name,
        field,
        value,
        "PPtr object",
        Value::is_object,
    ) {
        return;
    }
    let pointer = value.expect("validated pointer value");
    for component in ["fileId", "pathId"] {
        let component_field = format!("{field}.{component}");
        diagnose_expected_field(
            diagnostics,
            skin_name,
            style_name,
            &component_field,
            pointer.get(component),
            "integer",
            is_integer,
        );
    }
}

pub(super) fn diagnose_expected_field(
    diagnostics: &mut Vec<LegacyGuiSkinCandidateDiagnostic>,
    skin_name: &str,
    style_name: Option<&str>,
    field: &str,
    value: Option<&Value>,
    expected: &str,
    predicate: fn(&Value) -> bool,
) -> bool {
    match value {
        Some(value) if predicate(value) => true,
        Some(_) => {
            push_candidate_diagnostic(
                diagnostics,
                "invalidField",
                skin_name,
                style_name,
                field,
                &format!(
                    "field is not serialized as {expected}; output value is an unproven compatibility placeholder"
                ),
            );
            false
        }
        None => {
            push_candidate_diagnostic(
                diagnostics,
                "missingField",
                skin_name,
                style_name,
                field,
                &format!(
                    "field is absent; expected {expected}; output value is an unproven compatibility placeholder"
                ),
            );
            false
        }
    }
}

pub(super) fn is_integer(value: &Value) -> bool {
    value.as_i64().is_some() || value.as_u64().is_some()
}

pub(super) fn push_candidate_diagnostic(
    diagnostics: &mut Vec<LegacyGuiSkinCandidateDiagnostic>,
    code: &str,
    skin_name: &str,
    style_name: Option<&str>,
    field: &str,
    message: &str,
) {
    diagnostics.push(LegacyGuiSkinCandidateDiagnostic {
        code: code.to_owned(),
        skin: skin_name.to_owned(),
        style: style_name.map(str::to_owned),
        field: field.to_owned(),
        message: message.to_owned(),
    });
}

pub(super) fn pointer(
    value: &Value,
    source_asset: &str,
    identities: &ObjectIdentityIndex,
) -> LegacyGuiObjectPointer {
    let file_id_is_explicit = value.get("fileId").is_some_and(is_integer);
    let path_id_is_explicit = value.get("pathId").is_some_and(is_integer);
    let file_id = integer_field(value, "fileId");
    let path_id = integer_field(value, "pathId");
    // A zero file ID is local to the serialized asset which owns the PPtr. Positive file IDs
    // require the external-file table of that asset, which generic dump-object-all JSON omits.
    // Resolve only an exact, unique local composite identity; never borrow a same-PathID object
    // from another serialized asset or guess an external target.
    let identity = (file_id_is_explicit
        && path_id_is_explicit
        && file_id == 0
        && path_id != 0
        && !source_asset.is_empty())
    .then(|| identities.get(&(source_asset.to_owned(), path_id)))
    .flatten()
    .and_then(|identity| match identity {
        ObjectIdentityMatch::Unique(identity) => Some(identity),
        ObjectIdentityMatch::Ambiguous => None,
    });
    LegacyGuiObjectPointer {
        file_id,
        path_id,
        asset_name: identity.map(|identity| identity.name.clone()),
        asset_type: identity.map(|identity| identity.asset_type.clone()),
    }
}

pub(super) fn insets(value: &Value) -> LegacyGuiInsets {
    LegacyGuiInsets {
        left: integer_field(value, "m_Left"),
        right: integer_field(value, "m_Right"),
        top: integer_field(value, "m_Top"),
        bottom: integer_field(value, "m_Bottom"),
    }
}

pub(super) fn vector2(value: &Value) -> LegacyGuiVector2 {
    LegacyGuiVector2 {
        x: number_field(value, "x"),
        y: number_field(value, "y"),
    }
}

pub(super) fn color(value: &Value) -> LegacyGuiColor {
    LegacyGuiColor {
        r: number_field(value, "r"),
        g: number_field(value, "g"),
        b: number_field(value, "b"),
        a: number_field(value, "a"),
    }
}

pub(super) fn append_unresolved_pointer_diagnostics(
    skins: &[LegacyGuiSkin],
    diagnostics: &mut Vec<LegacyGuiSkinCandidateDiagnostic>,
) -> usize {
    let mut unresolved = 0;
    for skin in skins {
        unresolved += append_unresolved_pointer_diagnostic(
            &skin.font,
            &skin.name,
            None,
            "m_Font",
            diagnostics,
        );
        for (style_name, style) in &skin.built_in_styles {
            unresolved +=
                append_style_pointer_diagnostics(style, &skin.name, style_name, diagnostics);
        }
        for (index, style) in skin.custom_styles.iter().enumerate() {
            let style_name = if style.name.is_empty() {
                format!("customStyles[{index}]")
            } else {
                style.name.clone()
            };
            unresolved +=
                append_style_pointer_diagnostics(style, &skin.name, &style_name, diagnostics);
        }
    }
    unresolved
}

pub(super) fn append_style_pointer_diagnostics(
    style: &LegacyGuiStyle,
    skin_name: &str,
    style_name: &str,
    diagnostics: &mut Vec<LegacyGuiSkinCandidateDiagnostic>,
) -> usize {
    let mut unresolved = append_unresolved_pointer_diagnostic(
        &style.font,
        skin_name,
        Some(style_name),
        "m_Font",
        diagnostics,
    );
    for (state_name, state) in &style.states {
        let field = format!("states.{state_name}.m_Background");
        unresolved += append_unresolved_pointer_diagnostic(
            &state.background,
            skin_name,
            Some(style_name),
            &field,
            diagnostics,
        );
    }
    unresolved
}

pub(super) fn append_unresolved_pointer_diagnostic(
    pointer: &LegacyGuiObjectPointer,
    skin_name: &str,
    style_name: Option<&str>,
    field: &str,
    diagnostics: &mut Vec<LegacyGuiSkinCandidateDiagnostic>,
) -> usize {
    let unresolved = pointer.path_id != 0
        && (pointer.file_id != 0
            || pointer.asset_name.as_deref().unwrap_or_default().is_empty()
            || pointer.asset_type.as_deref().unwrap_or_default().is_empty());
    if !unresolved {
        return 0;
    }
    let message = if pointer.file_id > 0 {
        "positive fileId target is unresolved because generic dump JSON omits the owning external-file table"
    } else if pointer.file_id < 0 {
        "negative fileId is invalid for candidate pointer resolution"
    } else {
        "local PPtr target is absent or ambiguous within the owning serialized asset"
    };
    push_candidate_diagnostic(
        diagnostics,
        "unresolvedPointer",
        skin_name,
        style_name,
        field,
        message,
    );
    1
}

pub(super) fn string_field(value: &Value, field: &str) -> String {
    value
        .get(field)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned()
}

pub(super) fn integer_field(value: &Value, field: &str) -> i64 {
    value
        .get(field)
        .and_then(|value| {
            value
                .as_i64()
                .or_else(|| value.as_u64().map(|value| value as i64))
        })
        .unwrap_or_default()
}

pub(super) fn number_field(value: &Value, field: &str) -> f64 {
    value.get(field).and_then(Value::as_f64).unwrap_or_default()
}
