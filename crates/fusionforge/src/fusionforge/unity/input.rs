use super::*;

pub(super) fn read_value(
    asset: &Asset,
    asset_index: usize,
    tree: &TypeTree,
    reader: &mut BinaryReader,
) -> Result<UnityValue, String> {
    let mut align = false;
    let t = tree.type_name.as_str();
    let first_child = tree.children.first();
    let result = match t {
        "bool" => UnityValue::Bool(reader.read_bool()?),
        "SInt8" => UnityValue::Int(reader.read_i8()? as i64),
        "UInt8" | "char" => UnityValue::UInt(reader.read_u8()? as u64),
        "SInt16" => UnityValue::Int(reader.read_i16()? as i64),
        "UInt16" => UnityValue::UInt(reader.read_u16()? as u64),
        "SInt64" => UnityValue::Int(reader.read_i64()?),
        "UInt64" => UnityValue::UInt(reader.read_u64()?),
        "UInt32" | "unsigned int" => UnityValue::UInt(reader.read_u32()? as u64),
        "SInt32" | "int" => UnityValue::Int(reader.read_i32()? as i64),
        "float" => UnityValue::Float(reader.read_f32()? as f64),
        "double" => UnityValue::Float(reader.read_f64()?),
        "string" => {
            let size = reader.read_u32()? as usize;
            let value = reader.read_string_bytes(size)?;
            align = tree.children.first().is_some_and(TypeTree::post_align);
            UnityValue::String(value)
        }
        _ => {
            let array_tree = if tree.is_array { Some(tree) } else { None };
            if t.starts_with("PPtr<") {
                let file_id = reader.read_i32()?;
                let path_id = read_pointer_path_id(asset, reader)?;
                UnityValue::Pointer(Pointer {
                    source_asset: asset_index,
                    file_id,
                    path_id,
                })
            } else if let Some(array_parent) = array_tree.or_else(|| {
                (tree.children.len() == 1).then_some(()).and_then(|_| {
                    first_child.filter(|child| child.is_array && child.name == "Array")
                })
            }) {
                align = array_parent.post_align();
                let size = reader.read_u32()? as usize;
                let item_tree = array_parent
                    .children
                    .get(1)
                    .ok_or_else(|| format!("array {} has no item child", tree.name))?;
                if matches!(item_tree.type_name.as_str(), "char" | "UInt8") {
                    UnityValue::Bytes(reader.read_exact_vec(size)?)
                } else {
                    if size > reader.remaining() {
                        return Err(format!(
                            "array {} wants {} item(s), only {} byte(s) remain",
                            tree.name,
                            size,
                            reader.remaining()
                        ));
                    }
                    let mut values = Vec::with_capacity(size);
                    for _ in 0..size {
                        values.push(read_value(asset, asset_index, item_tree, reader)?);
                    }
                    UnityValue::Array(values)
                }
            } else if t == "pair" {
                if tree.children.len() != 2 {
                    return Err("pair type without two children".to_string());
                }
                UnityValue::Pair(
                    Box::new(read_value(asset, asset_index, &tree.children[0], reader)?),
                    Box::new(read_value(asset, asset_index, &tree.children[1], reader)?),
                )
            } else {
                let mut object = BTreeMap::new();
                for child in &tree.children {
                    object.insert(
                        child.name.clone(),
                        read_value(asset, asset_index, child, reader)?,
                    );
                }
                UnityValue::Object(object)
            }
        }
    };

    if align || tree.post_align() {
        reader.align4()?;
    }

    Ok(result)
}

pub fn collect_archive_dependencies(env: &UnityEnvironment) -> Vec<String> {
    let mut deps = Vec::new();
    for asset in &env.assets {
        for asset_ref in asset.asset_refs.iter().skip(1) {
            if let Some(archive) = archive_name_from_ref(&asset_ref.file_path)
                .or_else(|| archive_name_from_ref(&asset_ref.asset_path))
            {
                if !deps.contains(&archive) {
                    deps.push(archive);
                }
            }
        }
    }
    deps.sort();
    deps
}

pub fn read_packed_bits(bits: &UnityValue) -> Vec<u32> {
    let Some(object) = bits.as_object() else {
        return Vec::new();
    };
    let count = object
        .get("m_NumItems")
        .and_then(UnityValue::as_i64)
        .unwrap_or(0);
    let bit_size = object
        .get("m_BitSize")
        .and_then(UnityValue::as_i64)
        .unwrap_or(0);
    let Some(data) = object.get("m_Data").and_then(UnityValue::as_bytes) else {
        return Vec::new();
    };
    if count <= 0 || bit_size <= 0 {
        return Vec::new();
    }
    let mut reader = BitReader::new(data, bit_size as u32);
    (0..count).map(|_| reader.read()).collect()
}

pub fn read_packed_floats(bits: &UnityValue) -> Vec<f64> {
    let Some(object) = bits.as_object() else {
        return Vec::new();
    };
    let count = object
        .get("m_NumItems")
        .and_then(UnityValue::as_i64)
        .unwrap_or(0);
    let bit_size = object
        .get("m_BitSize")
        .and_then(UnityValue::as_i64)
        .unwrap_or(0);
    if count <= 0 || bit_size <= 0 {
        return Vec::new();
    }
    let items = read_packed_bits(bits);
    let max_value = ((1_u64 << bit_size.min(32)) - 1).max(1) as f64;
    let range = object
        .get("m_Range")
        .and_then(UnityValue::as_f64)
        .unwrap_or(0.0)
        / max_value;
    let start = object
        .get("m_Start")
        .and_then(UnityValue::as_f64)
        .unwrap_or(0.0);
    items
        .into_iter()
        .map(|value| value as f64 * range + start)
        .collect()
}
