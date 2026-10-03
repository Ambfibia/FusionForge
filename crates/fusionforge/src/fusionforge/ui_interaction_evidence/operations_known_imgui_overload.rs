use super::*;

pub(super) fn exact_method_for_row(context: &AssemblyContext, row: u32) -> Result<ExactManagedMethod, String> {
    let method = context
        .metadata
        .method_defs
        .get(row.saturating_sub(1) as usize)
        .ok_or_else(|| format!("MethodDef row {row} is out of range"))?;
    let type_row = *context
        .method_owners
        .get(row as usize)
        .ok_or_else(|| format!("MethodDef row {row} has no declaring type"))?;
    if type_row == 0 {
        return Err(format!("MethodDef row {row} has no declaring type"));
    }
    let declaring_type = context
        .type_names
        .get(type_row as usize)
        .cloned()
        .unwrap_or_else(|| format!("<TypeDef {type_row}>"));
    let name = context
        .metadata
        .strings
        .get(method.name)
        .map_err(|error| format!("invalid MethodDef name heap index: {error}"))?
        .to_string();
    let (return_type, parameter_types) = method_signature(&context.metadata, method)
        .ok_or_else(|| format!("MethodDef row {row} has an unreadable signature"))?;
    Ok(ExactManagedMethod {
        declaring_type,
        type_token: metadata_token(0x02, type_row),
        name,
        parameter_types,
        return_type,
        method_token: metadata_token(0x06, row),
        rva: method.rva,
    })
}

pub(super) fn method_signature(metadata: &Metadata, method: &MethodDefRow) -> Option<(String, Vec<String>)> {
    let blob = metadata.blobs.get(method.signature).ok()?;
    let signature = MethodSig::parse_blob(blob).ok()?;
    Some((
        format_type_sig(metadata, &signature.return_type),
        signature
            .params
            .iter()
            .map(|value| format_type_sig(metadata, value))
            .collect(),
    ))
}

pub(super) fn member_ref_owner_full_name(
    context: &AssemblyContext,
    class: &clrmeta::CodedIndex,
) -> Option<String> {
    match class.table? {
        TableId::TypeRef => type_ref_full_name(&context.metadata, class.row),
        TableId::TypeDef => context
            .type_names
            .get(class.row as usize)
            .filter(|value| !value.is_empty())
            .cloned(),
        _ => None,
    }
}

pub(super) fn type_ref_full_name(metadata: &Metadata, index: u32) -> Option<String> {
    let row = metadata.get_type_ref(index)?;
    let name = metadata.strings.get(row.type_name).ok()?;
    let namespace = if row.type_namespace == 0 {
        ""
    } else {
        metadata.strings.get(row.type_namespace).ok()?
    };
    Some(if namespace.is_empty() {
        name.to_string()
    } else {
        format!("{namespace}.{name}")
    })
}

pub(super) fn known_imgui_overload(member: &ManagedMemberIdentity) -> bool {
    let owner = member.declaring_type.as_str();
    let name = member.name.as_str();
    let ret = member.return_type.as_str();
    let params = member
        .parameter_types
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>();
    let exact = |expected: &[&str]| params.as_slice() == expected;
    let content = |value: &str| {
        matches!(
            value,
            "System.String" | "UnityEngine.GUIContent" | "UnityEngine.Texture"
        )
    };
    let options = "UnityEngine.GUILayoutOption[]";

    match (owner, name, ret) {
        ("UnityEngine.GUI", "Button" | "RepeatButton", "System.Boolean") => {
            (params.len() == 2 && params[0] == "UnityEngine.Rect" && content(params[1]))
                || (params.len() == 3
                    && params[0] == "UnityEngine.Rect"
                    && content(params[1])
                    && params[2] == "UnityEngine.GUIStyle")
        }
        ("UnityEngine.GUILayout", "Button" | "RepeatButton", "System.Boolean") => {
            (params.len() == 2 && content(params[0]) && params[1] == options)
                || (params.len() == 3
                    && content(params[0])
                    && params[1] == "UnityEngine.GUIStyle"
                    && params[2] == options)
        }
        ("UnityEngine.GUI", "Toggle", "System.Boolean") => {
            (params.len() == 3
                && params[0] == "UnityEngine.Rect"
                && params[1] == "System.Boolean"
                && content(params[2]))
                || (params.len() == 4
                    && params[0] == "UnityEngine.Rect"
                    && params[1] == "System.Boolean"
                    && content(params[2])
                    && params[3] == "UnityEngine.GUIStyle")
        }
        ("UnityEngine.GUILayout", "Toggle", "System.Boolean") => {
            (params.len() == 3
                && params[0] == "System.Boolean"
                && content(params[1])
                && params[2] == options)
                || (params.len() == 4
                    && params[0] == "System.Boolean"
                    && content(params[1])
                    && params[2] == "UnityEngine.GUIStyle"
                    && params[3] == options)
        }
        ("UnityEngine.GUI", "Toolbar", "System.Int32") => {
            params.len() >= 3
                && params.len() <= 4
                && params[0] == "UnityEngine.Rect"
                && params[1] == "System.Int32"
                && matches!(
                    params[2],
                    "System.String[]" | "UnityEngine.GUIContent[]" | "UnityEngine.Texture[]"
                )
                && (params.len() == 3 || params[3] == "UnityEngine.GUIStyle")
        }
        ("UnityEngine.GUILayout", "Toolbar", "System.Int32") => {
            params.len() >= 3
                && params.len() <= 4
                && params[0] == "System.Int32"
                && matches!(
                    params[1],
                    "System.String[]" | "UnityEngine.GUIContent[]" | "UnityEngine.Texture[]"
                )
                && params[params.len() - 1] == options
                && (params.len() == 3 || params[2] == "UnityEngine.GUIStyle")
        }
        ("UnityEngine.GUI", "SelectionGrid", "System.Int32") => {
            params.len() >= 4
                && params.len() <= 5
                && params[0] == "UnityEngine.Rect"
                && params[1] == "System.Int32"
                && matches!(
                    params[2],
                    "System.String[]" | "UnityEngine.GUIContent[]" | "UnityEngine.Texture[]"
                )
                && params[3] == "System.Int32"
                && (params.len() == 4 || params[4] == "UnityEngine.GUIStyle")
        }
        ("UnityEngine.GUILayout", "SelectionGrid", "System.Int32") => {
            params.len() >= 4
                && params.len() <= 5
                && params[0] == "System.Int32"
                && matches!(
                    params[1],
                    "System.String[]" | "UnityEngine.GUIContent[]" | "UnityEngine.Texture[]"
                )
                && params[2] == "System.Int32"
                && params[params.len() - 1] == options
                && (params.len() == 4 || params[3] == "UnityEngine.GUIStyle")
        }
        ("UnityEngine.GUI", "HorizontalSlider" | "VerticalSlider", "System.Single") => {
            exact(&[
                "UnityEngine.Rect",
                "System.Single",
                "System.Single",
                "System.Single",
            ]) || exact(&[
                "UnityEngine.Rect",
                "System.Single",
                "System.Single",
                "System.Single",
                "UnityEngine.GUIStyle",
                "UnityEngine.GUIStyle",
            ])
        }
        ("UnityEngine.GUILayout", "HorizontalSlider" | "VerticalSlider", "System.Single") => {
            exact(&["System.Single", "System.Single", "System.Single", options])
                || exact(&[
                    "System.Single",
                    "System.Single",
                    "System.Single",
                    "UnityEngine.GUIStyle",
                    "UnityEngine.GUIStyle",
                    options,
                ])
        }
        ("UnityEngine.GUI", "HorizontalScrollbar" | "VerticalScrollbar", "System.Single") => {
            exact(&[
                "UnityEngine.Rect",
                "System.Single",
                "System.Single",
                "System.Single",
                "System.Single",
            ]) || exact(&[
                "UnityEngine.Rect",
                "System.Single",
                "System.Single",
                "System.Single",
                "System.Single",
                "UnityEngine.GUIStyle",
            ])
        }
        ("UnityEngine.GUILayout", "HorizontalScrollbar" | "VerticalScrollbar", "System.Single") => {
            exact(&[
                "System.Single",
                "System.Single",
                "System.Single",
                "System.Single",
                options,
            ]) || exact(&[
                "System.Single",
                "System.Single",
                "System.Single",
                "System.Single",
                "UnityEngine.GUIStyle",
                options,
            ])
        }
        ("UnityEngine.GUI", "TextField" | "PasswordField" | "TextArea", "System.String") => {
            params.len() >= 2
                && params.len() <= 4
                && params[0] == "UnityEngine.Rect"
                && params[1] == "System.String"
                && params[2..].iter().all(|value| {
                    matches!(
                        *value,
                        "System.Int32" | "System.Char" | "UnityEngine.GUIStyle"
                    )
                })
        }
        ("UnityEngine.GUILayout", "TextField" | "PasswordField" | "TextArea", "System.String") => {
            params.len() >= 2
                && params.len() <= 5
                && params[0] == "System.String"
                && params[params.len() - 1] == options
                && params[1..params.len() - 1].iter().all(|value| {
                    matches!(
                        *value,
                        "System.Int32" | "System.Char" | "UnityEngine.GUIStyle"
                    )
                })
        }
        ("UnityEngine.GUI", "BeginScrollView", "UnityEngine.Vector2") => {
            exact(&[
                "UnityEngine.Rect",
                "UnityEngine.Vector2",
                "UnityEngine.Rect",
            ]) || exact(&[
                "UnityEngine.Rect",
                "UnityEngine.Vector2",
                "UnityEngine.Rect",
                "System.Boolean",
                "System.Boolean",
            ]) || exact(&[
                "UnityEngine.Rect",
                "UnityEngine.Vector2",
                "UnityEngine.Rect",
                "UnityEngine.GUIStyle",
                "UnityEngine.GUIStyle",
            ]) || exact(&[
                "UnityEngine.Rect",
                "UnityEngine.Vector2",
                "UnityEngine.Rect",
                "System.Boolean",
                "System.Boolean",
                "UnityEngine.GUIStyle",
                "UnityEngine.GUIStyle",
            ])
        }
        ("UnityEngine.GUILayout", "BeginScrollView", "UnityEngine.Vector2") => {
            exact(&["UnityEngine.Vector2", options])
                || exact(&[
                    "UnityEngine.Vector2",
                    "System.Boolean",
                    "System.Boolean",
                    options,
                ])
                || exact(&[
                    "UnityEngine.Vector2",
                    "UnityEngine.GUIStyle",
                    "UnityEngine.GUIStyle",
                    options,
                ])
                || exact(&[
                    "UnityEngine.Vector2",
                    "System.Boolean",
                    "System.Boolean",
                    "UnityEngine.GUIStyle",
                    "UnityEngine.GUIStyle",
                    options,
                ])
        }
        ("UnityEngine.GUI" | "UnityEngine.GUILayout", "EndScrollView", "System.Void") => {
            params.is_empty()
        }
        ("UnityEngine.GUI", "BeginGroup", "System.Void") => {
            exact(&["UnityEngine.Rect"])
                || exact(&["UnityEngine.Rect", "System.String"])
                || exact(&["UnityEngine.Rect", "System.String", "UnityEngine.GUIStyle"])
                || exact(&["UnityEngine.Rect", "UnityEngine.GUIContent"])
                || exact(&[
                    "UnityEngine.Rect",
                    "UnityEngine.GUIContent",
                    "UnityEngine.GUIStyle",
                ])
        }
        ("UnityEngine.GUI", "EndGroup", "System.Void") => params.is_empty(),
        ("UnityEngine.GUILayout", "BeginHorizontal" | "BeginVertical", "System.Void") => {
            exact(&[options])
                || exact(&["UnityEngine.GUIStyle", options])
                || (params.len() == 3
                    && content(params[0])
                    && params[1] == "UnityEngine.GUIStyle"
                    && params[2] == options)
        }
        ("UnityEngine.GUILayout", "EndHorizontal" | "EndVertical" | "EndArea", "System.Void") => {
            params.is_empty()
        }
        ("UnityEngine.GUILayout", "BeginArea", "System.Void") => {
            exact(&["UnityEngine.Rect"])
                || (params.len() >= 2
                    && params.len() <= 3
                    && params[0] == "UnityEngine.Rect"
                    && content(params[1])
                    && (params.len() == 2 || params[2] == "UnityEngine.GUIStyle"))
        }
        (
            "UnityEngine.GUI" | "UnityEngine.GUILayout",
            "Window" | "ModalWindow",
            "UnityEngine.Rect",
        ) => {
            params.len() >= 4
                && params[0] == "System.Int32"
                && params[1] == "UnityEngine.Rect"
                && params[2] == "UnityEngine.GUI/WindowFunction"
                && content(params[3])
        }
        ("UnityEngine.GUI", "DragWindow", "System.Void") => {
            params.is_empty() || exact(&["UnityEngine.Rect"])
        }
        ("UnityEngine.GUI", "FocusControl" | "SetNextControlName", "System.Void") => {
            exact(&["System.String"])
        }
        ("UnityEngine.GUI", "set_enabled" | "set_changed", "System.Void") => {
            exact(&["System.Boolean"])
        }
        ("UnityEngine.GUI", "set_depth", "System.Void") => exact(&["System.Int32"]),
        ("UnityEngine.GUI", "set_skin", "System.Void") => exact(&["UnityEngine.GUISkin"]),
        (
            "UnityEngine.GUI",
            "set_color" | "set_backgroundColor" | "set_contentColor",
            "System.Void",
        ) => exact(&["UnityEngine.Color"]),
        ("UnityEngine.GUI", "set_matrix", "System.Void") => exact(&["UnityEngine.Matrix4x4"]),
        ("UnityEngine.GUI", "set_tooltip", "System.Void") => exact(&["System.String"]),
        ("UnityEngine.GUI" | "UnityEngine.GUILayout", "Label" | "Box", "System.Void") => {
            !params.is_empty()
                && ((owner == "UnityEngine.GUI" && params[0] == "UnityEngine.Rect")
                    || owner == "UnityEngine.GUILayout")
        }
        ("UnityEngine.GUI", "DrawTexture" | "DrawTextureWithTexCoords", "System.Void") => {
            params.len() >= 2
                && params[0] == "UnityEngine.Rect"
                && params[1] == "UnityEngine.Texture"
        }
        ("UnityEngine.Event", "get_current", "UnityEngine.Event")
        | ("UnityEngine.Event", "Use", "System.Void") => params.is_empty(),
        ("UnityEngine.Event", "GetTypeForControl", "UnityEngine.EventType") => {
            exact(&["System.Int32"])
        }
        (
            "UnityEngine.Event",
            "get_type" | "get_rawType" | "get_keyCode" | "get_button" | "get_mousePosition"
            | "get_delta" | "get_modifiers" | "get_character" | "get_clickCount",
            _,
        ) => params.is_empty(),
        ("UnityEngine.GUIUtility", "GetControlID", "System.Int32") => {
            exact(&["UnityEngine.FocusType"])
                || exact(&["System.Int32", "UnityEngine.FocusType"])
                || exact(&["UnityEngine.FocusType", "UnityEngine.Rect"])
                || exact(&["System.Int32", "UnityEngine.FocusType", "UnityEngine.Rect"])
                || exact(&["UnityEngine.GUIContent", "UnityEngine.FocusType"])
                || exact(&[
                    "UnityEngine.GUIContent",
                    "UnityEngine.FocusType",
                    "UnityEngine.Rect",
                ])
        }
        ("UnityEngine.GUIUtility", "ExitGUI", "System.Void") => params.is_empty(),
        ("UnityEngine.GUIUtility", "get_hotControl" | "get_keyboardControl", "System.Int32") => {
            params.is_empty()
        }
        ("UnityEngine.GUIUtility", "set_hotControl" | "set_keyboardControl", "System.Void") => {
            exact(&["System.Int32"])
        }
        _ => false,
    }
}

pub(super) fn classify_api(member: &ManagedMemberIdentity) -> ClassifiedCall {
    use CallCategory as Category;
    use InteractionKind as Kind;
    if matches!(
        member.declaring_type.as_str(),
        "UnityEngine.GUI"
            | "UnityEngine.GUILayout"
            | "UnityEngine.Event"
            | "UnityEngine.GUIUtility"
    ) && !known_imgui_overload(member)
    {
        return ClassifiedCall {
            category: Category::Other,
            interaction: Kind::Other,
            boolean_result: member.return_type == "System.Boolean",
        };
    }
    let returns = |expected: &str| member.return_type == expected;
    let (category, interaction) = match (member.declaring_type.as_str(), member.name.as_str()) {
        ("UnityEngine.GUI" | "UnityEngine.GUILayout", "Button" | "RepeatButton")
            if returns("System.Boolean") =>
        {
            (Category::Control, Kind::Button)
        }
        ("UnityEngine.GUI" | "UnityEngine.GUILayout", "Toggle") if returns("System.Boolean") => {
            (Category::Control, Kind::Toggle)
        }
        ("UnityEngine.GUI" | "UnityEngine.GUILayout", "Toolbar") if returns("System.Int32") => {
            (Category::Control, Kind::Toolbar)
        }
        ("UnityEngine.GUI" | "UnityEngine.GUILayout", "SelectionGrid")
            if returns("System.Int32") =>
        {
            (Category::Control, Kind::SelectionGrid)
        }
        ("UnityEngine.GUI" | "UnityEngine.GUILayout", "HorizontalSlider" | "VerticalSlider")
            if returns("System.Single") =>
        {
            (Category::Control, Kind::Slider)
        }
        (
            "UnityEngine.GUI" | "UnityEngine.GUILayout",
            "HorizontalScrollbar" | "VerticalScrollbar",
        ) if returns("System.Single") => (Category::Control, Kind::Scrollbar),
        ("UnityEngine.GUI" | "UnityEngine.GUILayout", "TextField" | "PasswordField")
            if returns("System.String") =>
        {
            (Category::Control, Kind::TextField)
        }
        ("UnityEngine.GUI" | "UnityEngine.GUILayout", "TextArea") if returns("System.String") => {
            (Category::Control, Kind::TextArea)
        }
        ("UnityEngine.GUI" | "UnityEngine.GUILayout", "BeginScrollView")
            if returns("UnityEngine.Vector2") =>
        {
            (Category::ScrollScope, Kind::ScrollViewBegin)
        }
        ("UnityEngine.GUI" | "UnityEngine.GUILayout", "EndScrollView")
            if returns("System.Void") =>
        {
            (Category::ScrollScope, Kind::ScrollViewEnd)
        }
        ("UnityEngine.GUI", "BeginGroup") if returns("System.Void") => {
            (Category::ScrollScope, Kind::GroupBegin)
        }
        ("UnityEngine.GUI", "EndGroup") if returns("System.Void") => {
            (Category::ScrollScope, Kind::GroupEnd)
        }
        ("UnityEngine.GUILayout", "BeginHorizontal") if returns("System.Void") => {
            (Category::LayoutScope, Kind::HorizontalScopeBegin)
        }
        ("UnityEngine.GUILayout", "EndHorizontal") if returns("System.Void") => {
            (Category::LayoutScope, Kind::HorizontalScopeEnd)
        }
        ("UnityEngine.GUILayout", "BeginVertical") if returns("System.Void") => {
            (Category::LayoutScope, Kind::VerticalScopeBegin)
        }
        ("UnityEngine.GUILayout", "EndVertical") if returns("System.Void") => {
            (Category::LayoutScope, Kind::VerticalScopeEnd)
        }
        ("UnityEngine.GUILayout", "BeginArea") if returns("System.Void") => {
            (Category::LayoutScope, Kind::AreaBegin)
        }
        ("UnityEngine.GUILayout", "EndArea") if returns("System.Void") => {
            (Category::LayoutScope, Kind::AreaEnd)
        }
        ("UnityEngine.GUI" | "UnityEngine.GUILayout", "Window" | "ModalWindow")
            if returns("UnityEngine.Rect") =>
        {
            (Category::Control, Kind::Window)
        }
        ("UnityEngine.GUI", "DragWindow") if returns("System.Void") => {
            (Category::Control, Kind::DragWindow)
        }
        ("UnityEngine.GUI", "FocusControl") if returns("System.Void") => {
            (Category::Focus, Kind::FocusControl)
        }
        ("UnityEngine.GUI", "SetNextControlName") if returns("System.Void") => {
            (Category::Focus, Kind::NextControlName)
        }
        ("UnityEngine.GUI", "set_enabled") if returns("System.Void") => {
            (Category::GuiState, Kind::GuiEnabledWrite)
        }
        ("UnityEngine.GUI", "set_changed") if returns("System.Void") => {
            (Category::GuiState, Kind::GuiChangedWrite)
        }
        (
            "UnityEngine.GUI",
            "set_color"
            | "set_backgroundColor"
            | "set_contentColor"
            | "set_matrix"
            | "set_depth"
            | "set_skin"
            | "set_tooltip",
        ) if returns("System.Void") => (Category::GuiState, Kind::GuiVisualStateWrite),
        (
            "UnityEngine.GUI" | "UnityEngine.GUILayout",
            "Label" | "Box" | "DrawTexture" | "DrawTextureWithTexCoords",
        ) if returns("System.Void") => (Category::Draw, Kind::Draw),
        ("UnityEngine.Event", "get_current") if returns("UnityEngine.Event") => {
            (Category::EventState, Kind::EventCurrent)
        }
        ("UnityEngine.Event", "Use") if returns("System.Void") => {
            (Category::EventState, Kind::EventUse)
        }
        ("UnityEngine.Event", "GetTypeForControl") if returns("UnityEngine.EventType") => {
            (Category::EventState, Kind::EventTypeForControl)
        }
        (
            "UnityEngine.Event",
            "get_type" | "get_rawType" | "get_keyCode" | "get_button" | "get_mousePosition"
            | "get_delta" | "get_modifiers" | "get_character" | "get_clickCount",
        ) => (Category::EventState, Kind::EventRead),
        ("UnityEngine.GUIUtility", "GetControlID") if returns("System.Int32") => {
            (Category::Focus, Kind::ControlId)
        }
        ("UnityEngine.GUIUtility", "ExitGUI") if returns("System.Void") => {
            (Category::EventState, Kind::ExitGui)
        }
        ("UnityEngine.GUIUtility", "get_hotControl" | "set_hotControl") => {
            (Category::Focus, Kind::HotControl)
        }
        ("UnityEngine.GUIUtility", "get_keyboardControl" | "set_keyboardControl") => {
            (Category::Focus, Kind::KeyboardControl)
        }
        (
            "UnityEngine.Input",
            "GetKey" | "GetKeyDown" | "GetKeyUp" | "GetButton" | "GetButtonDown" | "GetButtonUp",
        ) if returns("System.Boolean") => (Category::Input, Kind::KeyPoll),
        ("UnityEngine.Input", "GetMouseButton" | "GetMouseButtonDown" | "GetMouseButtonUp")
            if returns("System.Boolean") =>
        {
            (Category::Input, Kind::MousePoll)
        }
        ("UnityEngine.Input", "get_mousePosition") if returns("UnityEngine.Vector3") => {
            (Category::Input, Kind::MousePoll)
        }
        ("UnityEngine.Input", "GetAxis" | "GetAxisRaw") if returns("System.Single") => {
            (Category::Input, Kind::AxisPoll)
        }
        (
            "UnityEngine.Time",
            "get_time"
            | "get_deltaTime"
            | "get_unscaledTime"
            | "get_unscaledDeltaTime"
            | "get_realtimeSinceStartup",
        ) if returns("System.Single") => (Category::Time, Kind::TimeRead),
        ("cnEvent", "SendPacket") => (Category::Packet, Kind::PacketSend),
        ("cnEvent", "SendEvent") => (Category::LocalEvent, Kind::LocalEventSend),
        ("SoundUtil", "Playsound" | "ButtonSound") => (Category::Audio, Kind::AudioSideEffect),
        ("PopupControll", "Popup" | "Open" | "Close" | "Show" | "Hide") => {
            (Category::Popup, Kind::PopupSideEffect)
        }
        _ => (Category::Other, Kind::Other),
    };
    ClassifiedCall {
        category,
        interaction,
        boolean_result: member.return_type == "System.Boolean",
    }
}

pub(super) fn build_type_names(metadata: &Metadata) -> Vec<String> {
    let mut names = vec![String::new(); metadata.type_defs.len() + 1];
    let nested = metadata
        .nested_classes
        .iter()
        .map(|row| (row.nested_class, row.enclosing_class))
        .collect::<HashMap<_, _>>();
    for index in 1..=metadata.type_defs.len() as u32 {
        names[index as usize] = type_full_name(metadata, &nested, index);
    }
    names
}

pub(super) fn type_full_name(metadata: &Metadata, nested: &HashMap<u32, u32>, index: u32) -> String {
    let Some(row) = metadata.get_type_def(index) else {
        return format!("<TypeDef {index}>");
    };
    let name = metadata
        .strings
        .get(row.type_name)
        .unwrap_or("")
        .to_string();
    if let Some(enclosing) = nested.get(&index).copied() {
        let parent = type_full_name(metadata, nested, enclosing);
        return if parent.is_empty() {
            name
        } else {
            format!("{parent}/{name}")
        };
    }
    let namespace = if row.type_namespace == 0 {
        ""
    } else {
        metadata.strings.get(row.type_namespace).unwrap_or("")
    };
    if namespace.is_empty() {
        name
    } else {
        format!("{namespace}.{name}")
    }
}

pub(super) fn build_method_owners(metadata: &Metadata) -> Vec<u32> {
    let mut owners = vec![0; metadata.method_defs.len() + 1];
    for type_row in 1..=metadata.type_defs.len() as u32 {
        for (method_row, _) in metadata.get_type_methods(type_row) {
            owners[method_row as usize] = type_row;
        }
    }
    owners
}

pub(super) fn build_field_owners(metadata: &Metadata) -> Vec<u32> {
    let mut owners = vec![0; metadata.fields.len() + 1];
    for type_row in 1..=metadata.type_defs.len() as u32 {
        for (field_row, _) in metadata.get_type_fields(type_row) {
            owners[field_row as usize] = type_row;
        }
    }
    owners
}

pub(super) fn format_type_sig(metadata: &Metadata, sig: &TypeSig) -> String {
    match sig {
        TypeSig::Primitive(kind) => primitive_type_name(*kind).to_string(),
        TypeSig::Class(token) | TypeSig::ValueType(token) => {
            let coded = clrmeta::CodedIndex::decode(clrmeta::CodedIndexKind::TypeDefOrRef, *token);
            metadata
                .resolve_type(&coded)
                .map(|value| value.full_name())
                .unwrap_or_else(|| format!("<type {token}>"))
        }
        TypeSig::SzArray(inner) => format!("{}[]", format_type_sig(metadata, inner)),
        TypeSig::Array { element_type, .. } => {
            format!("{}[]", format_type_sig(metadata, element_type))
        }
        TypeSig::Ptr(inner) => format!("{}*", format_type_sig(metadata, inner)),
        TypeSig::ByRef(inner) => format!("{}&", format_type_sig(metadata, inner)),
        TypeSig::GenericInst {
            type_ref,
            type_args,
            ..
        } => {
            let coded =
                clrmeta::CodedIndex::decode(clrmeta::CodedIndexKind::TypeDefOrRef, *type_ref);
            let base = metadata
                .resolve_type(&coded)
                .map(|value| value.full_name())
                .unwrap_or_else(|| format!("<type {type_ref}>"));
            let args = type_args
                .iter()
                .map(|arg| format_type_sig(metadata, arg))
                .collect::<Vec<_>>()
                .join(",");
            format!("{base}<{args}>")
        }
        TypeSig::Var(index) => format!("!{index}"),
        TypeSig::MVar(index) => format!("!!{index}"),
        TypeSig::FnPtr(_) => "method".to_string(),
        TypeSig::Modified { inner, .. } | TypeSig::Pinned(inner) => {
            format_type_sig(metadata, inner)
        }
        _ => "<type>".to_string(),
    }
}

pub(super) fn primitive_type_name(kind: ElementType) -> &'static str {
    match kind {
        ElementType::Void => "System.Void",
        ElementType::Boolean => "System.Boolean",
        ElementType::Char => "System.Char",
        ElementType::I1 => "System.SByte",
        ElementType::U1 => "System.Byte",
        ElementType::I2 => "System.Int16",
        ElementType::U2 => "System.UInt16",
        ElementType::I4 => "System.Int32",
        ElementType::U4 => "System.UInt32",
        ElementType::I8 => "System.Int64",
        ElementType::U8 => "System.UInt64",
        ElementType::R4 => "System.Single",
        ElementType::R8 => "System.Double",
        ElementType::String => "System.String",
        ElementType::TypedByRef => "System.TypedReference",
        ElementType::IntPtr => "System.IntPtr",
        ElementType::UIntPtr => "System.UIntPtr",
        ElementType::Object => "System.Object",
        _ => kind.name(),
    }
}

pub(super) fn normalize_sha256(value: &str) -> Result<String, String> {
    if value.len() != 64 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err("SHA-256 must contain exactly 64 hexadecimal digits".to_string());
    }
    Ok(value.to_ascii_uppercase())
}

pub(super) fn sha256_hex_lower(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

pub(super) fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:X}", hasher.finalize())
}

pub(super) fn metadata_token(table: u8, row: u32) -> String {
    format!("0x{:08X}", ((table as u32) << 24) | row)
}

pub(super) fn site_id(method_row: u32, il_offset: usize) -> String {
    format!("M{:08X}_IL_{il_offset:04X}", 0x0600_0000u32 | method_row)
}

pub(super) fn field_site_id(method_row: u32, il_offset: usize) -> String {
    format!(
        "M{:08X}_FIELD_IL_{il_offset:04X}",
        0x0600_0000u32 | method_row
    )
}
