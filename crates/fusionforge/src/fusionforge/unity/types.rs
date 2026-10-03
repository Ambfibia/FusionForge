use super::*;

#[derive(Debug, Clone)]
pub struct TypeTree {
    pub children: Vec<TypeTree>,
    pub version: i32,
    pub is_array: bool,
    pub size: i32,
    pub index: u32,
    pub flags: i32,
    pub type_name: String,
    pub name: String,
}

impl TypeTree {
    pub(super) fn new(_format: u32) -> Self {
        Self {
            children: Vec::new(),
            version: 0,
            is_array: false,
            size: 0,
            index: 0,
            flags: 0,
            type_name: "(null)".to_string(),
            name: "(null)".to_string(),
        }
    }

    pub fn post_align(&self) -> bool {
        (self.flags & 0x4000) != 0
    }

    pub(super) fn load(format: u32, reader: &mut BinaryReader) -> Result<Self, String> {
        if format == 10 || format >= 12 {
            Self::load_blob(format, reader)
        } else {
            Self::load_old(format, reader)
        }
    }

    pub(super) fn load_old(format: u32, reader: &mut BinaryReader) -> Result<Self, String> {
        let mut tree = Self::new(format);
        tree.type_name = reader.read_cstring()?;
        tree.name = reader.read_cstring()?;
        tree.size = reader.read_i32()?;
        tree.index = reader.read_i32()? as u32;
        tree.is_array = reader.read_i32()? != 0;
        tree.version = reader.read_i32()?;
        tree.flags = reader.read_i32()?;

        let count = reader.read_u32()?;
        for _ in 0..count {
            tree.children.push(Self::load(format, reader)?);
        }
        Ok(tree)
    }

    pub(super) fn load_blob(format: u32, reader: &mut BinaryReader) -> Result<Self, String> {
        let node_count = reader.read_u32()? as usize;
        let buffer_bytes = reader.read_u32()? as usize;
        let node_data = reader.read_exact_vec(24 * node_count)?;
        let string_data = reader.read_exact_vec(buffer_bytes)?;
        let mut node_reader = BinaryReader::from_slice(&node_data, Endian::Little);
        let mut nodes = Vec::with_capacity(node_count);

        for _ in 0..node_count {
            let version = node_reader.read_i16()? as i32;
            let depth = node_reader.read_u8()?;
            let is_array = node_reader.read_i8()? != 0;
            let type_offset = node_reader.read_i32()?;
            let name_offset = node_reader.read_i32()?;
            let size = node_reader.read_i32()?;
            let index = node_reader.read_u32()?;
            let flags = node_reader.read_i32()?;
            nodes.push(FlatTypeNode {
                depth,
                version,
                is_array,
                type_name: blob_string(type_offset, &string_data),
                name: blob_string(name_offset, &string_data),
                size,
                index,
                flags,
            });
        }

        fn build(format: u32, nodes: &[FlatTypeNode], cursor: &mut usize, depth: u8) -> TypeTree {
            let current = &nodes[*cursor];
            let mut tree = TypeTree {
                children: Vec::new(),
                version: current.version,
                is_array: current.is_array,
                size: current.size,
                index: current.index,
                flags: current.flags,
                type_name: current.type_name.clone(),
                name: current.name.clone(),
            };
            *cursor += 1;
            while *cursor < nodes.len() && nodes[*cursor].depth > depth {
                tree.children
                    .push(build(format, nodes, cursor, nodes[*cursor].depth));
            }
            let _ = format;
            tree
        }

        if nodes.is_empty() {
            return Ok(Self::new(format));
        }
        let mut cursor = 0;
        Ok(build(format, &nodes, &mut cursor, nodes[0].depth))
    }

    pub(super) fn write_old(&self, writer: &mut BinaryWriter) -> Result<(), String> {
        writer.write_cstring(&self.type_name)?;
        writer.write_cstring(&self.name)?;
        writer.write_i32(self.size)?;
        writer.write_i32(self.index as i32)?;
        writer.write_i32(i32::from(self.is_array))?;
        writer.write_i32(self.version)?;
        writer.write_i32(self.flags)?;
        writer.write_u32(self.children.len() as u32)?;
        for child in &self.children {
            child.write_old(writer)?;
        }
        Ok(())
    }
}

#[derive(Debug)]
pub(super) struct FlatTypeNode {
    pub(super) depth: u8,
    pub(super) version: i32,
    pub(super) is_array: bool,
    pub(super) type_name: String,
    pub(super) name: String,
    pub(super) size: i32,
    pub(super) index: u32,
    pub(super) flags: i32,
}

#[derive(Debug, Clone, Default)]
pub struct TypeMetadata {
    pub class_ids: Vec<i32>,
    pub type_trees: HashMap<i32, TypeTree>,
}

impl TypeMetadata {
    pub(super) fn load(
        reader: &mut BinaryReader,
        format: u32,
        file_size: u32,
        metadata_size: u32,
    ) -> Result<Self, String> {
        let mut metadata = Self::default();
        if format > 7 {
            let _generator_version = reader.read_cstring()?;
            let _target_platform = reader.read_u32()?;
        }

        if format >= 13 {
            let has_type_trees = reader.read_bool()?;
            let type_count = reader.read_i32()?;
            for _ in 0..type_count {
                let mut class_id = reader.read_i32()?;
                if format >= 17 {
                    let _unk0 = reader.read_u8()?;
                    let script_id = reader.read_i16()?;
                    if class_id == 114 {
                        class_id = if script_id >= 0 {
                            -2 - script_id as i32
                        } else {
                            -1
                        };
                    }
                }
                metadata.class_ids.push(class_id);
                let hash_size = if class_id < 0 { 0x20 } else { 0x10 };
                reader.skip(hash_size)?;
                if has_type_trees {
                    metadata
                        .type_trees
                        .insert(class_id, TypeTree::load(format, reader)?);
                }
            }
        } else {
            if format == 6 || format == 7 {
                reader.seek(file_size.saturating_sub(metadata_size).saturating_add(1) as u64)?;
            }
            let field_count = reader.read_i32()?;
            for _ in 0..field_count {
                let class_id = reader.read_i32()?;
                metadata
                    .type_trees
                    .insert(class_id, TypeTree::load(format, reader)?);
            }
        }

        Ok(metadata)
    }

    pub fn default_metadata() -> Result<&'static TypeMetadata, String> {
        static DEFAULT: OnceLock<Result<TypeMetadata, String>> = OnceLock::new();
        DEFAULT
            .get_or_init(|| {
                let mut reader = BinaryReader::from_slice(STRUCTS_DAT, Endian::Little);
                TypeMetadata::load(
                    &mut reader,
                    15,
                    STRUCTS_DAT.len() as u32,
                    STRUCTS_DAT.len() as u32,
                )
            })
            .as_ref()
            .map_err(Clone::clone)
    }

    pub(super) fn write_old(&self, writer: &mut BinaryWriter) -> Result<(), String> {
        writer.write_i32(self.type_trees.len() as i32)?;
        for (class_id, tree) in &self.type_trees {
            writer.write_i32(*class_id)?;
            tree.write_old(writer)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Pointer {
    pub source_asset: usize,
    pub file_id: i32,
    pub path_id: i64,
}

impl Pointer {
    pub fn is_null(&self) -> bool {
        // Unity treats m_PathID == 0 as a null PPtr. Old serialized files often
        // leave a stale/non-zero m_FileID in that state.
        self.path_id == 0
    }
}
