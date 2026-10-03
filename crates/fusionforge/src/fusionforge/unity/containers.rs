use super::*;

#[derive(Debug, Clone)]
pub struct ObjectInfo {
    pub path_id: i64,
    pub data_offset: u32,
    pub size: u32,
    pub type_id: i32,
    pub class_id: i32,
}

pub struct ReadObjectResult {
    pub value: UnityValue,
    pub consumed: usize,
    pub expected: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub enum UnityValue {
    Bool(bool),
    Int(i64),
    UInt(u64),
    Float(f64),
    String(String),
    Bytes(Vec<u8>),
    Array(Vec<UnityValue>),
    Object(BTreeMap<String, UnityValue>),
    Pair(Box<UnityValue>, Box<UnityValue>),
    Pointer(Pointer),
}

impl UnityValue {
    pub fn as_object(&self) -> Option<&BTreeMap<String, UnityValue>> {
        match self {
            UnityValue::Object(value) => Some(value),
            _ => None,
        }
    }

    pub fn as_object_mut(&mut self) -> Option<&mut BTreeMap<String, UnityValue>> {
        match self {
            UnityValue::Object(value) => Some(value),
            _ => None,
        }
    }

    pub fn get(&self, key: &str) -> Option<&UnityValue> {
        self.as_object()?.get(key)
    }

    pub fn get_mut(&mut self, key: &str) -> Option<&mut UnityValue> {
        self.as_object_mut()?.get_mut(key)
    }

    pub fn as_array(&self) -> Option<&[UnityValue]> {
        match self {
            UnityValue::Array(value) => Some(value),
            _ => None,
        }
    }

    pub fn as_array_mut(&mut self) -> Option<&mut Vec<UnityValue>> {
        match self {
            UnityValue::Array(value) => Some(value),
            _ => None,
        }
    }

    pub fn as_bytes(&self) -> Option<&[u8]> {
        match self {
            UnityValue::Bytes(value) => Some(value),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            UnityValue::String(value) => Some(value),
            _ => None,
        }
    }

    pub fn as_i64(&self) -> Option<i64> {
        match self {
            UnityValue::Int(value) => Some(*value),
            UnityValue::UInt(value) => i64::try_from(*value).ok(),
            UnityValue::Float(value) => Some(*value as i64),
            UnityValue::Bool(value) => Some(i64::from(*value)),
            _ => None,
        }
    }

    pub fn as_f64(&self) -> Option<f64> {
        match self {
            UnityValue::Float(value) => Some(*value),
            UnityValue::Int(value) => Some(*value as f64),
            UnityValue::UInt(value) => Some(*value as f64),
            UnityValue::Bool(value) => Some(if *value { 1.0 } else { 0.0 }),
            _ => None,
        }
    }

    pub fn as_pointer(&self) -> Option<&Pointer> {
        match self {
            UnityValue::Pointer(value) if !value.is_null() => Some(value),
            _ => None,
        }
    }

    pub fn to_json_sample(&self) -> JsonValue {
        match self {
            UnityValue::Bool(value) => json!(value),
            UnityValue::Int(value) => json!(value),
            UnityValue::UInt(value) => json!(value),
            UnityValue::Float(value) => json!(value),
            UnityValue::String(value) => json!(value),
            UnityValue::Bytes(value) => json!({ "bytes": value.len() }),
            UnityValue::Array(value) => JsonValue::Array(
                value
                    .iter()
                    .take(8)
                    .map(UnityValue::to_json_sample)
                    .collect(),
            ),
            UnityValue::Object(value) => JsonValue::Object(
                value
                    .iter()
                    .take(16)
                    .map(|(key, value)| (key.clone(), value.to_json_sample()))
                    .collect(),
            ),
            UnityValue::Pair(left, right) => json!([left.to_json_sample(), right.to_json_sample()]),
            UnityValue::Pointer(value) => {
                json!({ "fileId": value.file_id, "pathId": value.path_id })
            }
        }
    }
}

pub(super) fn write_object_metadata(
    asset: &Asset,
    writer: &mut BinaryWriter,
    object: &ObjectInfo,
) -> Result<(), String> {
    write_path_id(asset, writer, object.path_id)?;
    writer.write_u32(object.data_offset)?;
    writer.write_u32(object.size)?;
    writer.write_i32(object.type_id)?;
    writer.write_i16(object.class_id as i16)?;
    if asset.format <= 10 {
        writer.write_i16(0)?;
    }
    if (11..=16).contains(&asset.format) {
        writer.write_i16(0)?;
    }
    if (15..=16).contains(&asset.format) {
        writer.write_u8(0)?;
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ObjectKey {
    pub asset: usize,
    pub path_id: i64,
}

pub struct UnityEnvironment {
    pub assets: Vec<Asset>,
    pub(super) by_name: HashMap<String, usize>,
    // Cache lifetime is exactly the loaded source environment, never its recycled address.
    pub(crate) exact_texture_cache: RefCell<BTreeMap<(usize, i64), Result<JsonValue, String>>>,
}

impl UnityEnvironment {
    pub fn from_paths(paths: &[PathBuf]) -> Self {
        let assets = paths
            .iter()
            .filter_map(|path| Asset::from_path(path).ok())
            .collect::<Vec<_>>();
        Self::from_assets(assets)
    }

    pub fn from_dir(session_dir: &Path) -> Self {
        if let Ok(entries) = fs::read_dir(session_dir) {
            let mut paths = entries
                .flatten()
                .map(|entry| entry.path())
                .filter(|path| path.is_file())
                .collect::<Vec<_>>();
            paths.sort();
            return Self::from_paths(&paths);
        }
        Self::from_assets(Vec::new())
    }

    pub fn from_assets(assets: Vec<Asset>) -> Self {
        let mut by_name = HashMap::new();
        for (index, asset) in assets.iter().enumerate() {
            by_name.insert(asset.name.to_lowercase(), index);
        }
        Self {
            assets,
            by_name,
            exact_texture_cache: RefCell::new(BTreeMap::new()),
        }
    }

    pub fn asset_index_by_name(&self, name: &str) -> Option<usize> {
        let short = Path::new(name)
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or(name)
            .to_lowercase();
        self.by_name.get(&short).copied()
    }

    pub(super) fn asset_index_for_ref(&self, asset_ref: &AssetRef) -> Option<usize> {
        let mut candidates = Vec::new();
        if let Some(name) = archive_name_from_ref(&asset_ref.file_path) {
            candidates.push(name);
        }
        if let Some(name) = archive_name_from_ref(&asset_ref.asset_path) {
            candidates.push(name);
        }
        if let Some(name) = archive_ref_asset_name(&asset_ref.file_path) {
            candidates.push(name);
        }
        if let Some(name) = archive_ref_asset_name(&asset_ref.asset_path) {
            candidates.push(name);
        }
        candidates.push(asset_ref.file_path.clone());
        candidates.push(asset_ref.asset_path.clone());
        candidates
            .into_iter()
            .filter(|candidate| !candidate.is_empty())
            .find_map(|candidate| self.asset_index_by_name(&candidate))
    }

    pub fn resolve_pointer(&self, pointer: &Pointer) -> Result<ObjectKey, String> {
        if pointer.is_null() {
            return Err("null pointer".to_string());
        }
        let source = self
            .assets
            .get(pointer.source_asset)
            .ok_or_else(|| "source asset missing".to_string())?;
        if source.format == 7 && source.long_object_ids {
            return self
                .resolve_pointer_strict(pointer)?
                .ok_or_else(|| "null pointer".to_string());
        }
        if source.format == 7 {
            if pointer.file_id != 0 {
                if let Ok(ref_index) = usize::try_from(pointer.file_id) {
                    if let Some(asset_ref) = source.asset_refs.get(ref_index) {
                        let asset_index = self.asset_index_for_ref(asset_ref).ok_or_else(|| {
                            let asset_name = archive_ref_asset_name(&asset_ref.file_path)
                                .or_else(|| archive_ref_asset_name(&asset_ref.asset_path))
                                .unwrap_or_else(|| asset_ref.file_path.clone());
                            format!("No such asset: '{asset_name}'")
                        })?;
                        if self.assets[asset_index]
                            .objects
                            .contains_key(&pointer.path_id)
                        {
                            return Ok(ObjectKey {
                                asset: asset_index,
                                path_id: pointer.path_id,
                            });
                        }
                        return Err(format!(
                            "{}#{} was not found in referenced asset {}",
                            source.name,
                            pointer.path_id,
                            self.asset_name(asset_index)
                        ));
                    }
                }
                if source.objects.contains_key(&pointer.path_id) {
                    return Ok(ObjectKey {
                        asset: pointer.source_asset,
                        path_id: pointer.path_id,
                    });
                }
                for asset_ref in &source.asset_refs {
                    let Some(asset_index) = self.asset_index_for_ref(asset_ref) else {
                        continue;
                    };
                    if self.assets[asset_index]
                        .objects
                        .contains_key(&pointer.path_id)
                    {
                        return Ok(ObjectKey {
                            asset: asset_index,
                            path_id: pointer.path_id,
                        });
                    }
                }
                for (asset_index, asset) in self.assets.iter().enumerate() {
                    if asset.objects.contains_key(&pointer.path_id) {
                        return Ok(ObjectKey {
                            asset: asset_index,
                            path_id: pointer.path_id,
                        });
                    }
                }
                return Err(format!(
                    "{}#{} was not found in loaded assets",
                    source.name, pointer.path_id
                ));
            }

            if source.objects.contains_key(&pointer.path_id) {
                return Ok(ObjectKey {
                    asset: pointer.source_asset,
                    path_id: pointer.path_id,
                });
            }
            let mut missing_refs = Vec::new();
            for asset_ref in &source.asset_refs {
                let Some(asset_index) = self.asset_index_for_ref(asset_ref) else {
                    if let Some(name) = archive_ref_asset_name(&asset_ref.file_path)
                        .or_else(|| archive_ref_asset_name(&asset_ref.asset_path))
                    {
                        missing_refs.push(name);
                    }
                    continue;
                };
                if self.assets[asset_index]
                    .objects
                    .contains_key(&pointer.path_id)
                {
                    return Ok(ObjectKey {
                        asset: asset_index,
                        path_id: pointer.path_id,
                    });
                }
            }
            for (asset_index, asset) in self.assets.iter().enumerate() {
                if asset.objects.contains_key(&pointer.path_id) {
                    return Ok(ObjectKey {
                        asset: asset_index,
                        path_id: pointer.path_id,
                    });
                }
            }
            if let Some(name) = missing_refs.first() {
                return Err(format!("No such asset: '{name}'"));
            }
            return Err(format!(
                "{}#{} was not found in referenced assets",
                source.name, pointer.path_id
            ));
        }
        let asset_index = if pointer.file_id == 0 {
            pointer.source_asset
        } else {
            let asset_ref = source
                .asset_refs
                .get(pointer.file_id as usize)
                .ok_or_else(|| format!("missing asset ref {}", pointer.file_id))?;
            self.asset_index_for_ref(asset_ref).ok_or_else(|| {
                let asset_name = archive_ref_asset_name(&asset_ref.file_path)
                    .or_else(|| archive_ref_asset_name(&asset_ref.asset_path))
                    .unwrap_or_else(|| asset_ref.file_path.clone());
                format!("No such asset: '{asset_name}'")
            })?
        };
        Ok(ObjectKey {
            asset: asset_index,
            path_id: pointer.path_id,
        })
    }

    pub(super) fn strict_asset_indices_for_ref(&self, asset_ref: &AssetRef) -> BTreeSet<usize> {
        let mut candidate_names = BTreeSet::<String>::new();
        let mut add_candidate = |value: String| {
            let normalized = value.replace('\\', "/").to_lowercase();
            if !normalized.is_empty() {
                candidate_names.insert(normalized.clone());
                if let Some(short) = normalized
                    .rsplit('/')
                    .next()
                    .filter(|part| !part.is_empty())
                {
                    candidate_names.insert(short.to_string());
                }
            }
        };
        for value in [&asset_ref.file_path, &asset_ref.asset_path] {
            if let Some(name) = archive_name_from_ref(value) {
                add_candidate(name);
            }
            if let Some(name) = archive_ref_asset_name(value) {
                add_candidate(name);
            }
            add_candidate(value.clone());
        }

        self.assets
            .iter()
            .enumerate()
            .filter_map(|(index, asset)| {
                let normalized = asset.name.replace('\\', "/").to_lowercase();
                let short = normalized
                    .rsplit('/')
                    .next()
                    .filter(|part| !part.is_empty())
                    .unwrap_or(&normalized);
                (candidate_names.contains(&normalized) || candidate_names.contains(short))
                    .then_some(index)
            })
            .collect()
    }

    /// Resolve a PPtr only through the owning serialized asset and its exact
    /// external-file table. This intentionally does not use the format-7 local
    /// or loaded-asset PathID fallbacks retained by `resolve_pointer` for
    /// legacy preview/export compatibility.
    pub fn resolve_pointer_strict(&self, pointer: &Pointer) -> Result<Option<ObjectKey>, String> {
        if pointer.is_null() {
            return Ok(None);
        }
        let source = self.assets.get(pointer.source_asset).ok_or_else(|| {
            format!(
                "PPtr source serialized asset index {} is absent",
                pointer.source_asset
            )
        })?;

        let reference_index = source.pointer_file_index(pointer).ok_or_else(|| {
            format!(
                "{} has invalid negative external fileID {}",
                source.name, pointer.file_id
            )
        })?;
        let target_asset = if reference_index == 0 {
            pointer.source_asset
        } else {
            let asset_ref = source.asset_refs.get(reference_index).ok_or_else(|| {
                format!(
                    "{} external fileID {} is outside its {}-entry table",
                    source.name,
                    pointer.file_id,
                    source.asset_refs.len().saturating_sub(1)
                )
            })?;
            let candidates = self.strict_asset_indices_for_ref(asset_ref);
            match candidates.len() {
                0 => {
                    return Err(format!(
                        "{} external fileID {} ('{}'/'{}') is not loaded",
                        source.name, pointer.file_id, asset_ref.file_path, asset_ref.asset_path
                    ));
                }
                1 => *candidates.iter().next().expect("one strict PPtr candidate"),
                _ => {
                    let names = candidates
                        .iter()
                        .map(|index| self.asset_name(*index).to_string())
                        .collect::<Vec<_>>();
                    return Err(format!(
                        "{} external fileID {} is ambiguous across serialized assets: {}",
                        source.name,
                        pointer.file_id,
                        names.join(", ")
                    ));
                }
            }
        };

        let target = self.assets.get(target_asset).ok_or_else(|| {
            format!("PPtr target serialized asset index {target_asset} is absent")
        })?;
        if !target.objects.contains_key(&pointer.path_id) {
            return Err(format!(
                "{} fileID {} pathID {} is absent from serialized asset {}",
                source.name, pointer.file_id, pointer.path_id, target.name
            ));
        }
        if source.format == 7 && source.long_object_ids {
            let encoded_id = ((u64::from(pointer.file_id as u32 >> 16)) << 32)
                | u64::from(pointer.path_id as u32);
            let object = &target.objects[&pointer.path_id];
            if object.path_id as u64 != encoded_id {
                return Err(format!(
                    "{} packed PPtr object ID {encoded_id:#x} does not match {} object ID {:#x}",
                    source.name, target.name, object.path_id
                ));
            }
        }
        Ok(Some(ObjectKey {
            asset: target_asset,
            path_id: pointer.path_id,
        }))
    }

    pub fn read_object(&self, key: ObjectKey) -> Result<UnityValue, String> {
        let asset = self
            .assets
            .get(key.asset)
            .ok_or_else(|| "asset index out of range".to_string())?;
        let object = asset
            .objects
            .get(&key.path_id)
            .ok_or_else(|| format!("{}#{} not found", asset.name, key.path_id))?;
        asset.read_object(key.asset, object)
    }

    pub fn resolve_value(&self, pointer: &Pointer) -> Result<UnityValue, String> {
        self.read_object(self.resolve_pointer(pointer)?)
    }

    pub fn asset_name(&self, index: usize) -> &str {
        self.assets
            .get(index)
            .map(|asset| asset.name.as_str())
            .unwrap_or("")
    }

    pub fn pointer_key(&self, pointer: &Pointer) -> Option<(String, i64)> {
        self.resolve_pointer(pointer)
            .ok()
            .map(|key| (self.asset_name(key.asset).to_string(), key.path_id))
    }

    pub fn read_streaming_data(&self, info: &UnityValue) -> Option<Vec<u8>> {
        let offset = info.get("offset")?.as_i64()? as usize;
        let size = info.get("size")?.as_i64()? as usize;
        let path = info.get("path")?.as_str()?;
        if path.is_empty() {
            return None;
        }
        let asset_index = self.asset_index_by_name(path)?;
        let asset = self.assets.get(asset_index)?;
        asset
            .data
            .get(offset..offset.saturating_add(size))
            .map(|data| data.to_vec())
    }
}

pub fn object_name(value: &UnityValue) -> String {
    value
        .get("m_Name")
        .or_else(|| value.get("name"))
        .and_then(UnityValue::as_str)
        .unwrap_or_default()
        .to_string()
}

pub fn normalize_bundle_name(value: &str) -> String {
    let mut value = value.to_lowercase();
    for prefix in ["customassetbundle-", "customassetbundle_", "cab-", "cab_"] {
        if let Some(stripped) = value.strip_prefix(prefix) {
            value = stripped.to_string();
            break;
        }
    }
    value
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .collect()
}
