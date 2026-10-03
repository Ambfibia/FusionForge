use super::*;

pub(super) fn blob_string(offset: i32, local_data: &[u8]) -> String {
    let data = if offset < 0 {
        let offset = (offset & 0x7fff_ffff) as usize;
        STRINGS_DAT.get(offset..).unwrap_or_default()
    } else {
        local_data.get(offset as usize..).unwrap_or_default()
    };
    let end = data
        .iter()
        .position(|value| *value == 0)
        .unwrap_or(data.len());
    String::from_utf8_lossy(&data[..end]).to_string()
}

pub fn empty_value_for_type(tree: &TypeTree) -> Result<UnityValue, String> {
    let first_child = tree.children.first();
    Ok(match tree.type_name.as_str() {
        "bool" => UnityValue::Bool(false),
        "SInt8" | "SInt16" | "SInt32" | "SInt64" | "int" => UnityValue::Int(0),
        "UInt8" | "char" | "UInt16" | "UInt32" | "UInt64" | "unsigned int" => UnityValue::UInt(0),
        "float" | "double" => UnityValue::Float(0.0),
        "string" => UnityValue::String(String::new()),
        value if value.starts_with("PPtr<") => UnityValue::Pointer(Pointer {
            source_asset: 0,
            file_id: 0,
            path_id: 0,
        }),
        "pair" => {
            if tree.children.len() != 2 {
                return Err("pair type without two children".to_string());
            }
            UnityValue::Pair(
                Box::new(empty_value_for_type(&tree.children[0])?),
                Box::new(empty_value_for_type(&tree.children[1])?),
            )
        }
        _ => {
            let array_tree = if tree.is_array { Some(tree) } else { None };
            if let Some(array_parent) = array_tree.or_else(|| {
                (tree.children.len() == 1).then_some(()).and_then(|_| {
                    first_child.filter(|child| child.is_array && child.name == "Array")
                })
            }) {
                let item_tree = array_parent
                    .children
                    .get(1)
                    .ok_or_else(|| format!("array {} has no item child", tree.name))?;
                if matches!(item_tree.type_name.as_str(), "char" | "UInt8") {
                    UnityValue::Bytes(Vec::new())
                } else {
                    UnityValue::Array(Vec::new())
                }
            } else {
                let mut object = BTreeMap::new();
                for child in &tree.children {
                    object.insert(child.name.clone(), empty_value_for_type(child)?);
                }
                UnityValue::Object(object)
            }
        }
    })
}

pub fn coerce_value_for_type(tree: &TypeTree, value: &UnityValue) -> Result<UnityValue, String> {
    let first_child = tree.children.first();
    Ok(match tree.type_name.as_str() {
        "bool" => UnityValue::Bool(value.as_i64().unwrap_or(0) != 0),
        "SInt8" | "SInt16" | "SInt32" | "SInt64" | "int" => {
            UnityValue::Int(value.as_i64().unwrap_or(0))
        }
        "UInt8" | "char" | "UInt16" | "UInt32" | "UInt64" | "unsigned int" => {
            UnityValue::UInt(value.as_i64().unwrap_or(0) as u64)
        }
        "float" | "double" => UnityValue::Float(value.as_f64().unwrap_or(0.0)),
        "string" => UnityValue::String(value.as_str().unwrap_or_default().to_string()),
        value_type if value_type.starts_with("PPtr<") => match value {
            UnityValue::Pointer(pointer) => UnityValue::Pointer(pointer.clone()),
            _ => UnityValue::Pointer(Pointer {
                source_asset: 0,
                file_id: 0,
                path_id: 0,
            }),
        },
        "pair" => {
            if tree.children.len() != 2 {
                return Err("pair type without two children".to_string());
            }
            let (left, right) = match value {
                UnityValue::Pair(left, right) => (left.as_ref(), right.as_ref()),
                UnityValue::Array(values) if values.len() >= 2 => (&values[0], &values[1]),
                _ => {
                    return empty_value_for_type(tree);
                }
            };
            UnityValue::Pair(
                Box::new(coerce_value_for_type(&tree.children[0], left)?),
                Box::new(coerce_value_for_type(&tree.children[1], right)?),
            )
        }
        _ => {
            let array_tree = if tree.is_array { Some(tree) } else { None };
            if let Some(array_parent) = array_tree.or_else(|| {
                (tree.children.len() == 1).then_some(()).and_then(|_| {
                    first_child.filter(|child| child.is_array && child.name == "Array")
                })
            }) {
                let item_tree = array_parent
                    .children
                    .get(1)
                    .ok_or_else(|| format!("array {} has no item child", tree.name))?;
                if matches!(item_tree.type_name.as_str(), "char" | "UInt8") {
                    UnityValue::Bytes(
                        value
                            .as_bytes()
                            .map(ToOwned::to_owned)
                            .or_else(|| value.as_str().map(|text| text.as_bytes().to_vec()))
                            .unwrap_or_default(),
                    )
                } else {
                    let Some(values) = value.as_array() else {
                        return Ok(UnityValue::Array(Vec::new()));
                    };
                    UnityValue::Array(
                        values
                            .iter()
                            .map(|item| coerce_value_for_type(item_tree, item))
                            .collect::<Result<Vec<_>, _>>()?,
                    )
                }
            } else {
                let Some(object) = value.as_object() else {
                    return empty_value_for_type(tree);
                };
                let mut coerced = BTreeMap::new();
                for child in &tree.children {
                    let child_value = if let Some(child_value) = object.get(&child.name) {
                        coerce_value_for_type(child, child_value)?
                    } else {
                        empty_value_for_type(child)?
                    };
                    coerced.insert(child.name.clone(), child_value);
                }
                UnityValue::Object(coerced)
            }
        }
    })
}

pub fn pointer_summary(value: Option<&UnityValue>) -> Option<JsonValue> {
    let pointer = value?.as_pointer()?;
    Some(json!({ "fileId": pointer.file_id, "pathId": pointer.path_id }))
}

pub fn vector(value: Option<&UnityValue>) -> Option<(f64, f64, f64)> {
    let value = value?.as_object()?;
    Some((
        value.get("x")?.as_f64()?,
        value.get("y")?.as_f64()?,
        value.get("z")?.as_f64()?,
    ))
}

pub fn vector2(value: Option<&UnityValue>) -> Option<(f64, f64)> {
    let value = value?.as_object()?;
    Some((value.get("x")?.as_f64()?, value.get("y")?.as_f64()?))
}

pub fn class_name(id: i32) -> String {
    static CLASSES: OnceLock<HashMap<String, String>> = OnceLock::new();
    CLASSES
        .get_or_init(|| serde_json::from_str(CLASSES_JSON).unwrap_or_default())
        .get(&id.to_string())
        .cloned()
        .unwrap_or_else(|| format!("<Unknown #{id}>"))
}

pub fn archive_name_from_ref(value: &str) -> Option<String> {
    if value.is_empty() || value.to_lowercase().starts_with("library/") {
        return None;
    }
    let lower = value.replace('\\', "/").to_lowercase();
    if lower.starts_with("customassetbundle-")
        || lower.starts_with("customassetbundle_")
        || lower.starts_with("cab-")
        || lower.starts_with("cab_")
    {
        return Some(lower);
    }
    if !lower.starts_with("archive:") {
        return None;
    }
    let path = lower.trim_start_matches("archive:").trim_start_matches('/');
    let archive = path.split('/').next().unwrap_or_default();
    if archive.is_empty() || archive.starts_with("library") {
        None
    } else {
        Some(archive.to_string())
    }
}

pub fn value_array(value: Option<&UnityValue>) -> &[UnityValue] {
    value.and_then(UnityValue::as_array).unwrap_or(&[])
}

pub fn pair_name_value(value: &UnityValue) -> Option<(&str, &UnityValue)> {
    match value {
        UnityValue::Pair(left, right) => Some((pair_key_name(left)?, right)),
        UnityValue::Array(items) if items.len() >= 2 => {
            Some((pair_key_name(&items[0])?, &items[1]))
        }
        _ => None,
    }
}

pub fn pair_key_name(value: &UnityValue) -> Option<&str> {
    value
        .as_str()
        .or_else(|| value.get("name").and_then(UnityValue::as_str))
}
