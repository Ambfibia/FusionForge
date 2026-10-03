use super::*;

pub(super) fn publish_shared_package(
    package: &DetailPackage,
    package_root: &str,
    shared_files: &mut BTreeMap<String, Vec<u8>>,
) -> Result<(DetailRoutes, u64)> {
    let rooted_member = |source_path: &str| -> Result<String> {
        validate_relative(source_path)?;
        let member = Path::new(source_path)
            .strip_prefix(Path::new(&package.package_relative))
            .map_err(|_| {
                invalid_error(format!(
                    "detail dependency {source_path:?} escaped package {:?}",
                    package.package_relative
                ))
            })?;
        validate_relative_path(member)?;
        Ok(format!("{package_root}/{}", slash_path(member)))
    };

    let mut document = package.document.clone();
    let object = document
        .as_object_mut()
        .ok_or_else(|| invalid_error("detail texture document is not an object"))?;
    let mips = object
        .get_mut("mips")
        .and_then(JsonValue::as_array_mut)
        .ok_or_else(|| invalid_error("detail texture document has no mips"))?;
    let mut rewritten_mip_zero = None;
    for mip in mips {
        let mip_object = mip
            .as_object_mut()
            .ok_or_else(|| invalid_error("detail mip is not an object"))?;
        let level = mip_object
            .get("level")
            .and_then(JsonValue::as_u64)
            .ok_or_else(|| invalid_error("detail mip has no level"))?;
        let original = mip_object
            .get("path")
            .and_then(JsonValue::as_str)
            .ok_or_else(|| invalid_error("detail mip path is absent"))?
            .to_owned();
        let rooted = rooted_member(&original)?;
        mip_object.insert("path".to_owned(), JsonValue::String(rooted.clone()));
        if level == 0 {
            rewritten_mip_zero = Some(rooted);
        }
        if let Some(source) = mip_object
            .get_mut("sourceEncoded")
            .and_then(JsonValue::as_object_mut)
        {
            let original = source
                .get("path")
                .and_then(JsonValue::as_str)
                .ok_or_else(|| invalid_error("detail source mip path is absent"))?
                .to_owned();
            source.insert(
                "path".to_owned(),
                JsonValue::String(rooted_member(&original)?),
            );
        }
    }
    let rewritten_base = if package.base_is_mip_zero {
        rewritten_mip_zero.ok_or_else(|| invalid_error("detail document lost mip zero"))?
    } else {
        rooted_member(&package.base_path)?
    };
    object.insert("path".to_owned(), JsonValue::String(rewritten_base.clone()));
    let document_bytes = pretty_json(&document)?;

    let mut output_bytes = 0_u64;
    let base_member = Path::new(&package.base_path)
        .strip_prefix(Path::new(&package.package_relative))
        .map(slash_path)
        .map_err(|_| invalid_error("detail base escaped its package"))?;
    for file in &package.files {
        if file.member == "texture.json"
            || (package.base_is_mip_zero
                && package.base_path != package.mip_zero_path
                && file.member == base_member)
        {
            continue;
        }
        let route = format!("{package_root}/{}", file.member);
        let bytes = read_file(&file.absolute, "shared detail member")?;
        output_bytes += bytes.len() as u64;
        insert_bytes(shared_files, route, bytes, "shared detail member")?;
    }
    let document_path = format!("{package_root}/texture.json");
    output_bytes += document_bytes.len() as u64;
    insert_bytes(
        shared_files,
        document_path.clone(),
        document_bytes.clone(),
        "shared detail document",
    )?;
    Ok((
        DetailRoutes {
            document_path,
            document_blake3: format!("blake3:{}", hash(&document_bytes)),
            texture_path: rewritten_base,
        },
        output_bytes,
    ))
}

pub(super) fn write_new(path: &Path, bytes: &[u8]) -> Result<()> {
    if path.exists() {
        return invalid(format!(
            "refusing to overwrite staged file: {}",
            path.display()
        ));
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| io_at(parent, error))?;
    }
    fs::write(path, bytes).map_err(|error| io_at(path, error))
}

pub(super) fn write_replace(path: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| io_at(parent, error))?;
    }
    let temporary = path.with_extension(format!(
        "{}.tmp-{}",
        path.extension()
            .and_then(|extension| extension.to_str())
            .unwrap_or("json"),
        std::process::id()
    ));
    if temporary.exists() {
        return invalid(format!(
            "stale report temporary exists: {}",
            temporary.display()
        ));
    }
    fs::write(&temporary, bytes).map_err(|error| io_at(&temporary, error))?;
    if path.exists() {
        fs::remove_file(path).map_err(|error| io_at(path, error))?;
    }
    fs::rename(&temporary, path).map_err(|error| io_at(path, error))
}
