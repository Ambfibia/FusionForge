use super::*;

pub(super) fn write_value(
    asset: &Asset,
    asset_index: usize,
    tree: &TypeTree,
    writer: &mut BinaryWriter,
    value: &UnityValue,
) -> Result<(), String> {
    let mut align = false;
    let t = tree.type_name.as_str();
    let first_child = tree.children.first();
    match t {
        "bool" => writer.write_bool(value.as_i64().unwrap_or(0) != 0)?,
        "SInt8" => writer.write_i8(value.as_i64().unwrap_or(0) as i8)?,
        "UInt8" | "char" => writer.write_u8(value.as_i64().unwrap_or(0) as u8)?,
        "SInt16" => writer.write_i16(value.as_i64().unwrap_or(0) as i16)?,
        "UInt16" => writer.write_u16(value.as_i64().unwrap_or(0) as u16)?,
        "SInt64" => writer.write_i64(value.as_i64().unwrap_or(0))?,
        "UInt64" => writer.write_u64(value.as_i64().unwrap_or(0) as u64)?,
        "UInt32" | "unsigned int" => writer.write_u32(value.as_i64().unwrap_or(0) as u32)?,
        "SInt32" | "int" => writer.write_i32(value.as_i64().unwrap_or(0) as i32)?,
        "float" => writer.write_f32(value.as_f64().unwrap_or(0.0) as f32)?,
        "double" => writer.write_f64(value.as_f64().unwrap_or(0.0))?,
        "string" => {
            let text = value.as_str().unwrap_or_default();
            writer.write_u32(text.len() as u32)?;
            writer.write_string_bytes(text)?;
            align = tree.children.first().is_some_and(TypeTree::post_align);
        }
        _ => {
            let array_tree = if tree.is_array { Some(tree) } else { None };
            if t.starts_with("PPtr<") {
                let pointer = match value {
                    UnityValue::Pointer(pointer) => pointer.clone(),
                    _ => Pointer {
                        source_asset: asset_index,
                        file_id: 0,
                        path_id: 0,
                    },
                };
                writer.write_i32(pointer.file_id)?;
                write_pointer_path_id(asset, writer, pointer.path_id)?;
            } else if let Some(array_parent) = array_tree.or_else(|| {
                (tree.children.len() == 1).then_some(()).and_then(|_| {
                    first_child.filter(|child| child.is_array && child.name == "Array")
                })
            }) {
                align = array_parent.post_align();
                let item_tree = array_parent
                    .children
                    .get(1)
                    .ok_or_else(|| format!("array {} has no item child", tree.name))?;
                if matches!(item_tree.type_name.as_str(), "char" | "UInt8") {
                    let bytes = value
                        .as_bytes()
                        .or_else(|| value.as_str().map(str::as_bytes))
                        .ok_or_else(|| format!("{} expects byte data", tree.name))?;
                    writer.write_u32(bytes.len() as u32)?;
                    writer.write_exact(bytes)?;
                } else {
                    let values = value
                        .as_array()
                        .ok_or_else(|| format!("{} expects an array", tree.name))?;
                    writer.write_u32(values.len() as u32)?;
                    for item in values {
                        write_value(asset, asset_index, item_tree, writer, item)?;
                    }
                }
            } else if t == "pair" {
                if tree.children.len() != 2 {
                    return Err("pair type without two children".to_string());
                }
                match value {
                    UnityValue::Pair(left, right) => {
                        write_value(asset, asset_index, &tree.children[0], writer, left)?;
                        write_value(asset, asset_index, &tree.children[1], writer, right)?;
                    }
                    UnityValue::Array(values) if values.len() >= 2 => {
                        write_value(asset, asset_index, &tree.children[0], writer, &values[0])?;
                        write_value(asset, asset_index, &tree.children[1], writer, &values[1])?;
                    }
                    _ => return Err(format!("{} expects a pair", tree.name)),
                }
            } else {
                if tree.children.len() == 2 {
                    match value {
                        UnityValue::Pair(left, right) => {
                            write_value(asset, asset_index, &tree.children[0], writer, left)?;
                            write_value(asset, asset_index, &tree.children[1], writer, right)?;
                            return Ok(());
                        }
                        UnityValue::Array(values) if values.len() >= 2 => {
                            write_value(asset, asset_index, &tree.children[0], writer, &values[0])?;
                            write_value(asset, asset_index, &tree.children[1], writer, &values[1])?;
                            return Ok(());
                        }
                        _ => {}
                    }
                }
                let object = value.as_object().ok_or_else(|| {
                    let children = tree
                        .children
                        .iter()
                        .map(|child| {
                            format!(
                                "{}:{}{}",
                                child.name,
                                child.type_name,
                                if child.is_array { "[]" } else { "" }
                            )
                        })
                        .collect::<Vec<_>>()
                        .join(",");
                    format!(
                        "{}:{} expects an object; children=[{}]",
                        tree.name, tree.type_name, children
                    )
                })?;
                for child in &tree.children {
                    let child_value = object
                        .get(&child.name)
                        .ok_or_else(|| format!("{} missing child {}", tree.name, child.name))?;
                    write_value(asset, asset_index, child, writer, child_value)?;
                }
            }
        }
    }

    if align || tree.post_align() {
        writer.align4()?;
    }

    Ok(())
}
