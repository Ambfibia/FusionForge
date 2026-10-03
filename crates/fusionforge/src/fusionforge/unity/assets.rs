use super::*;

#[derive(Debug, Clone)]
pub struct AssetRef {
    pub asset_path: String,
    pub guid: [u8; 16],
    pub type_id: i32,
    pub file_path: String,
}

#[derive(Debug, Clone)]
pub struct Asset {
    pub name: String,
    pub data: Vec<u8>,
    pub format: u32,
    pub metadata_size: u32,
    pub file_size: u32,
    pub data_offset: u32,
    pub long_object_ids: bool,
    pub tree: TypeMetadata,
    pub objects: BTreeMap<i64, ObjectInfo>,
    pub asset_refs: Vec<AssetRef>,
}

pub(super) fn stored_path_id_for_asset(asset: &Asset, path_id: i64) -> i64 {
    if asset.long_object_ids || asset.format >= 14 {
        path_id
    } else {
        i64::from(path_id as i32)
    }
}

impl Asset {
    /// Academy's format-7 long IDs pack the external index in the low 16
    /// bits of the first PPtr word and the object-ID tag in its high 16 bits.
    /// Keep the raw words in Pointer so serialization remains byte exact.
    pub fn pointer_file_index(&self, pointer: &Pointer) -> Option<usize> {
        if self.format == 7 && self.long_object_ids {
            Some((pointer.file_id as u32 & 0xffff) as usize)
        } else {
            usize::try_from(pointer.file_id).ok()
        }
    }

    pub fn empty_with_metadata(name: impl Into<String>, format: u32) -> Result<Self, String> {
        Ok(Self {
            name: name.into(),
            data: Vec::new(),
            format,
            metadata_size: 0,
            file_size: 0,
            data_offset: 0,
            long_object_ids: (7..=13).contains(&format),
            tree: TypeMetadata::default_metadata()?.clone(),
            objects: BTreeMap::new(),
            asset_refs: vec![AssetRef {
                asset_path: String::new(),
                guid: [0; 16],
                type_id: 0,
                file_path: String::new(),
            }],
        })
    }

    pub fn empty_object_value_for_class(&self, class_id: i32) -> Result<UnityValue, String> {
        let tree = self
            .tree
            .type_trees
            .get(&class_id)
            .ok_or_else(|| format!("TypeTree for classId {class_id} was not found"))?;
        empty_value_for_type(tree)
    }

    pub fn serialize_object_value_for_class(
        &self,
        asset_index: usize,
        class_id: i32,
        value: &UnityValue,
    ) -> Result<Vec<u8>, String> {
        let tree = self
            .tree
            .type_trees
            .get(&class_id)
            .ok_or_else(|| format!("TypeTree for classId {class_id} was not found"))?;
        self.serialize_object_value_with_tree(asset_index, tree, value)
    }

    pub fn from_path(path: &Path) -> Result<Self, String> {
        let data = fs::read(path).map_err(|err| format!("{}: {err}", path.display()))?;
        Self::from_bytes(path.file_name().unwrap_or_default().to_string_lossy().into_owned(), data)
    }

    pub fn from_bytes(name: String, data: Vec<u8>) -> Result<Self, String> {
        let mut asset = Self {
            name,
            data,
            format: 0,
            metadata_size: 0,
            file_size: 0,
            data_offset: 0,
            long_object_ids: false,
            tree: TypeMetadata::default(),
            objects: BTreeMap::new(),
            asset_refs: Vec::new(),
        };
        asset.load()?;
        Ok(asset)
    }

    pub(super) fn load(&mut self) -> Result<(), String> {
        if self.name.ends_with(".resource") {
            return Ok(());
        }
        let mut reader = BinaryReader::from_slice(&self.data, Endian::Big);
        self.metadata_size = reader.read_u32()?;
        self.file_size = reader.read_u32()?;
        self.format = reader.read_u32()?;
        self.data_offset = reader.read_u32()?;

        if self.format >= 9 {
            let endianness = reader.read_u32()?;
            if endianness == 0 {
                reader.endian = Endian::Little;
            }
        } else if self.format == 6 || self.format == 7 {
            reader.endian = Endian::Little;
        }

        self.tree =
            TypeMetadata::load(&mut reader, self.format, self.file_size, self.metadata_size)?;

        if (7..=13).contains(&self.format) {
            self.long_object_ids = reader.read_u32()? != 0;
        }

        let object_count = reader.read_u32()?;
        for _ in 0..object_count {
            if self.format >= 14 {
                reader.align4()?;
            }
            let object = self.load_object_info(&mut reader)?;
            self.objects
                .insert(self.effective_path_id(object.path_id), object);
        }

        if self.format >= 11 {
            let adds = reader.read_u32()?;
            for _ in 0..adds {
                if self.format >= 14 {
                    reader.align4()?;
                }
                let _id = self.read_id(&mut reader)?;
                let _value = reader.read_i32()?;
            }
        }

        self.asset_refs.push(AssetRef {
            asset_path: String::new(),
            guid: [0; 16],
            type_id: 0,
            file_path: self.name.clone(),
        });
        if self.format >= 6 {
            let ref_count = reader.read_u32()?;
            for _ in 0..ref_count {
                let asset_path = reader.read_cstring()?;
                let guid = reader
                    .read_exact_vec(16)?
                    .try_into()
                    .map_err(|_| "bad asset ref guid".to_string())?;
                let type_id = reader.read_i32()?;
                let file_path = reader.read_cstring()?;
                self.asset_refs.push(AssetRef {
                    asset_path,
                    guid,
                    type_id,
                    file_path,
                });
            }
        }

        Ok(())
    }

    pub(super) fn load_object_info(&self, reader: &mut BinaryReader) -> Result<ObjectInfo, String> {
        let path_id = if self.long_object_ids {
            reader.read_u64()? as i64
        } else {
            self.read_id(reader)?
        };
        let data_offset = reader.read_u32()?.saturating_add(self.data_offset);
        let size = reader.read_u32()?;
        let (type_id, class_id) = if self.format < 17 {
            let type_id = reader.read_i32()?;
            let class_id = reader.read_i16()? as i32;
            (type_id, class_id)
        } else {
            let type_index = reader.read_i32()? as usize;
            let class_id = *self.tree.class_ids.get(type_index).unwrap_or(&0);
            (class_id, class_id)
        };
        if self.format <= 10 {
            let _destroyed = reader.read_i16()?;
        }
        if (11..=16).contains(&self.format) {
            let _unk0 = reader.read_i16()?;
        }
        if (15..=16).contains(&self.format) {
            let _unk1 = reader.read_u8()?;
        }
        Ok(ObjectInfo {
            path_id,
            data_offset,
            size,
            type_id,
            class_id,
        })
    }

    pub fn read_id(&self, reader: &mut BinaryReader) -> Result<i64, String> {
        if self.format >= 14 {
            reader.read_i64()
        } else {
            reader.read_i32().map(i64::from)
        }
    }

    pub(super) fn effective_path_id(&self, path_id: i64) -> i64 {
        if self.long_object_ids {
            path_id & 0xffff_ffff
        } else {
            path_id
        }
    }

    pub fn object_type_name(&self, object: &ObjectInfo) -> String {
        if object.type_id > 0 {
            class_name(object.type_id)
        } else if object.class_id == 114 {
            "MonoBehaviour".to_string()
        } else if object.class_id != 0 {
            class_name(object.class_id)
        } else {
            object.type_id.to_string()
        }
    }

    pub(super) fn type_tree_for(&self, object: &ObjectInfo) -> Result<&TypeTree, String> {
        if object.type_id < 0 {
            if let Some(tree) = self.tree.type_trees.get(&object.type_id) {
                return Ok(tree);
            }
            if let Some(tree) = self.tree.type_trees.get(&object.class_id) {
                return Ok(tree);
            }
            return TypeMetadata::default_metadata()?
                .type_trees
                .get(&object.class_id)
                .ok_or_else(|| format!("{} absent from structs.dat", object.class_id));
        }
        if let Some(tree) = self.tree.type_trees.get(&object.type_id) {
            return Ok(tree);
        }
        if let Some(tree) = self.tree.type_trees.get(&object.class_id) {
            return Ok(tree);
        }
        TypeMetadata::default_metadata()?
            .type_trees
            .get(&object.class_id)
            .ok_or_else(|| format!("{} absent from structs.dat", object.class_id))
    }

    pub fn object_type_tree(&self, object: &ObjectInfo) -> Result<&TypeTree, String> {
        self.type_tree_for(object)
    }

    pub fn object_raw_data(&self, object: &ObjectInfo) -> Result<&[u8], String> {
        let start = object.data_offset as usize;
        let end = start.saturating_add(object.size as usize);
        self.data
            .get(start..end)
            .ok_or_else(|| format!("{}#{} out of range object data", self.name, object.path_id))
    }

    pub fn read_object(
        &self,
        asset_index: usize,
        object: &ObjectInfo,
    ) -> Result<UnityValue, String> {
        self.read_object_with_size(asset_index, object)
            .map(|result| result.value)
    }

    pub fn read_object_with_size(
        &self,
        asset_index: usize,
        object: &ObjectInfo,
    ) -> Result<ReadObjectResult, String> {
        let data = self.object_raw_data(object)?;
        let mut reader = BinaryReader::from_slice(data, Endian::Little);
        let value = read_value(self, asset_index, self.type_tree_for(object)?, &mut reader)?;
        Ok(ReadObjectResult {
            value,
            consumed: reader.position() as usize,
            expected: data.len(),
        })
    }

    pub fn read_object_by_path_id(
        &self,
        asset_index: usize,
        path_id: i64,
    ) -> Result<UnityValue, String> {
        let object = self
            .objects
            .get(&path_id)
            .ok_or_else(|| format!("{}#{} not found", self.name, path_id))?;
        self.read_object(asset_index, object)
    }

    pub fn serialize_object_value(
        &self,
        asset_index: usize,
        object: &ObjectInfo,
        value: &UnityValue,
    ) -> Result<Vec<u8>, String> {
        let mut writer = BinaryWriter::new(Endian::Little);
        write_value(
            self,
            asset_index,
            self.type_tree_for(object)?,
            &mut writer,
            value,
        )?;
        Ok(writer.into_inner())
    }

    pub fn serialize_object_value_with_tree(
        &self,
        asset_index: usize,
        tree: &TypeTree,
        value: &UnityValue,
    ) -> Result<Vec<u8>, String> {
        let mut writer = BinaryWriter::new(Endian::Little);
        write_value(self, asset_index, tree, &mut writer, value)?;
        Ok(writer.into_inner())
    }

    pub fn with_serialized_format(&self, format: u32) -> Self {
        let mut asset = self.clone();
        asset.format = format;
        if !(7..=13).contains(&format) {
            asset.long_object_ids = false;
        }
        asset
    }

    pub fn empty_object_value(&self, object: &ObjectInfo) -> Result<UnityValue, String> {
        empty_value_for_type(self.type_tree_for(object)?)
    }

    pub fn patch_object_same_size(
        &mut self,
        path_id: i64,
        asset_index: usize,
        value: &UnityValue,
    ) -> Result<(), String> {
        let object = self
            .objects
            .get(&path_id)
            .cloned()
            .ok_or_else(|| format!("{}#{} not found", self.name, path_id))?;
        let data = self.serialize_object_value(asset_index, &object, value)?;
        if data.len() != object.size as usize {
            return Err(format!(
                "{}#{} serialized to {} bytes, expected exactly {} bytes for in-place patch",
                self.name,
                path_id,
                data.len(),
                object.size
            ));
        }
        let start = object.data_offset as usize;
        let end = start + data.len();
        self.data
            .get_mut(start..end)
            .ok_or_else(|| format!("{}#{} out of range object data", self.name, path_id))?
            .copy_from_slice(&data);
        Ok(())
    }

    pub fn rebuild_with_object_data(
        &self,
        replacements: &BTreeMap<i64, Vec<u8>>,
    ) -> Result<Vec<u8>, String> {
        self.rebuild_with_object_data_filtered(None, replacements)
    }

    pub fn rebuild_from_object_data_as_format(
        &self,
        objects: &[(ObjectInfo, Vec<u8>)],
        output_format: Option<u32>,
    ) -> Result<Vec<u8>, String> {
        self.rebuild_with_object_data_filtered_and_extra_as_format(
            Some(&std::collections::BTreeSet::new()),
            &BTreeMap::new(),
            objects,
            output_format,
        )
    }

    pub fn rebuild_with_object_data_filtered(
        &self,
        keep_path_ids: Option<&std::collections::BTreeSet<i64>>,
        replacements: &BTreeMap<i64, Vec<u8>>,
    ) -> Result<Vec<u8>, String> {
        self.rebuild_with_object_data_filtered_and_extra(keep_path_ids, replacements, &[])
    }

    pub fn rebuild_with_object_data_filtered_and_extra(
        &self,
        keep_path_ids: Option<&std::collections::BTreeSet<i64>>,
        replacements: &BTreeMap<i64, Vec<u8>>,
        extra_objects: &[(ObjectInfo, Vec<u8>)],
    ) -> Result<Vec<u8>, String> {
        self.rebuild_with_object_data_filtered_and_extra_as_format(
            keep_path_ids,
            replacements,
            extra_objects,
            None,
        )
    }

    pub fn rebuild_with_object_data_filtered_and_extra_as_format(
        &self,
        keep_path_ids: Option<&std::collections::BTreeSet<i64>>,
        replacements: &BTreeMap<i64, Vec<u8>>,
        extra_objects: &[(ObjectInfo, Vec<u8>)],
        output_format: Option<u32>,
    ) -> Result<Vec<u8>, String> {
        self.rebuild_with_object_data_filtered_and_extra_remapped_as_format(
            keep_path_ids,
            replacements,
            extra_objects,
            &BTreeMap::new(),
            output_format,
        )
    }

    pub fn rebuild_with_object_data_filtered_and_extra_remapped_as_format(
        &self,
        keep_path_ids: Option<&std::collections::BTreeSet<i64>>,
        replacements: &BTreeMap<i64, Vec<u8>>,
        extra_objects: &[(ObjectInfo, Vec<u8>)],
        output_path_ids: &BTreeMap<i64, i64>,
        output_format: Option<u32>,
    ) -> Result<Vec<u8>, String> {
        if self.name.ends_with(".resource") {
            return Ok(self.data.clone());
        }
        let output_asset = output_format
            .map(|format| self.with_serialized_format(format))
            .unwrap_or_else(|| self.clone());
        if output_asset.format > 7 {
            return Err(format!(
                "asset rebuild currently supports Unity serialized formats <= 7, got {}",
                output_asset.format
            ));
        }

        let mut writer = BinaryWriter::new(Endian::Little);
        writer.write_exact(&[0; 16])?;

        let selected_objects = self
            .objects
            .values()
            .filter(|object| {
                keep_path_ids
                    .map(|ids| {
                        ids.contains(&object.path_id)
                            || ids.contains(&self.effective_path_id(object.path_id))
                    })
                    .unwrap_or(true)
            })
            .collect::<Vec<_>>();
        let mut updated_objects = Vec::with_capacity(selected_objects.len());
        for object in selected_objects {
            writer.align4()?;
            let data_offset = writer.position() as u32;
            let data = replacements
                .get(&object.path_id)
                .or_else(|| replacements.get(&self.effective_path_id(object.path_id)))
                .map(Vec::as_slice)
                .unwrap_or(self.object_raw_data(object)?);
            writer.write_exact(data)?;
            let mut updated = object.clone();
            if let Some(path_id) = output_path_ids
                .get(&updated.path_id)
                .or_else(|| output_path_ids.get(&self.effective_path_id(updated.path_id)))
            {
                updated.path_id = *path_id;
            }
            updated.path_id = stored_path_id_for_asset(&output_asset, updated.path_id);
            updated.data_offset = data_offset;
            updated.size = data.len() as u32;
            updated_objects.push(updated);
        }
        for (object, data) in extra_objects {
            writer.align4()?;
            let data_offset = writer.position() as u32;
            writer.write_exact(data)?;
            let mut updated = object.clone();
            updated.path_id = stored_path_id_for_asset(&output_asset, updated.path_id);
            updated.data_offset = data_offset;
            updated.size = data.len() as u32;
            updated_objects.push(updated);
        }
        updated_objects.sort_by_key(|object| object.path_id);
        for window in updated_objects.windows(2) {
            if window[0].path_id == window[1].path_id {
                return Err(format!(
                    "{} has duplicate serialized path id {}",
                    output_asset.name, window[0].path_id
                ));
            }
        }

        let metadata_offset = writer.position() as u32;
        writer.write_u8(0)?;
        self.tree.write_old(&mut writer)?;

        if output_asset.format == 7 {
            writer.write_u32(u32::from(output_asset.long_object_ids))?;
        }

        writer.write_u32(updated_objects.len() as u32)?;
        for object in &updated_objects {
            write_object_metadata(&output_asset, &mut writer, object)?;
        }

        if output_asset.format >= 6 {
            writer.write_u32(self.asset_refs.len().saturating_sub(1) as u32)?;
            for asset_ref in self.asset_refs.iter().skip(1) {
                write_asset_ref(&mut writer, asset_ref)?;
            }
        }
        writer.write_u8(0)?;

        let file_size = writer.position() as u32;
        let metadata_size = file_size.saturating_sub(metadata_offset);
        let data_offset = 0_u32;
        writer.seek(0)?;
        writer.endian = Endian::Big;
        writer.write_u32(metadata_size)?;
        writer.write_u32(file_size)?;
        writer.write_u32(output_asset.format)?;
        writer.write_u32(data_offset)?;

        Ok(writer.into_inner())
    }

    pub fn rebuild_with_object_values(
        &self,
        asset_index: usize,
        replacements: &BTreeMap<i64, UnityValue>,
    ) -> Result<Vec<u8>, String> {
        let mut data_replacements = BTreeMap::new();
        for (path_id, value) in replacements {
            let object = self
                .objects
                .get(path_id)
                .ok_or_else(|| format!("{}#{} not found", self.name, path_id))?;
            data_replacements.insert(
                *path_id,
                self.serialize_object_value(asset_index, object, value)?,
            );
        }
        self.rebuild_with_object_data(&data_replacements)
    }
}

pub(super) fn write_asset_ref(writer: &mut BinaryWriter, asset_ref: &AssetRef) -> Result<(), String> {
    writer.write_cstring(&asset_ref.asset_path)?;
    writer.write_exact(&asset_ref.guid)?;
    writer.write_i32(asset_ref.type_id)?;
    writer.write_cstring(&asset_ref.file_path)
}

pub(super) fn write_path_id(asset: &Asset, writer: &mut BinaryWriter, path_id: i64) -> Result<(), String> {
    if asset.long_object_ids {
        writer.write_u64(path_id as u64)
    } else if asset.format >= 14 {
        writer.write_i64(path_id)
    } else {
        writer.write_i32(path_id as i32)
    }
}

pub(super) fn write_pointer_path_id(
    asset: &Asset,
    writer: &mut BinaryWriter,
    path_id: i64,
) -> Result<(), String> {
    if asset.format == 7 {
        writer.write_u32(path_id as u32)
    } else if asset.long_object_ids {
        writer.write_u64(path_id as u64)
    } else if asset.format >= 14 {
        writer.write_i64(path_id)
    } else {
        writer.write_i32(path_id as i32)
    }
}

pub(super) fn read_pointer_path_id(asset: &Asset, reader: &mut BinaryReader) -> Result<i64, String> {
    if asset.format == 7 {
        reader.read_u32().map(i64::from)
    } else if asset.long_object_ids {
        reader.read_u64().map(|value| value as i64)
    } else {
        asset.read_id(reader)
    }
}

pub fn archive_ref_asset_name(value: &str) -> Option<String> {
    if value.is_empty() {
        return None;
    }
    let normalized = value.replace('\\', "/");
    if normalized.starts_with("archive:") {
        return normalized
            .rsplit('/')
            .next()
            .filter(|part| !part.is_empty())
            .map(|part| part.to_lowercase());
    }
    Path::new(&normalized)
        .file_name()
        .and_then(|value| value.to_str())
        .map(|value| value.to_lowercase())
}
