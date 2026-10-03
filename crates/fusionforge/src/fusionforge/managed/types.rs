use super::*;

#[derive(Debug, Clone)]
pub(super) struct CliHeader {
    pub(super) rva: u32,
    pub(super) metadata_rva: u32,
    pub(super) metadata_size: u32,
}

#[derive(Debug, Clone)]
pub(super) struct AssemblyContext {
    pub(super) pe: PE,
    pub(super) metadata: Metadata,
    pub(super) metadata_bytes: Vec<u8>,
    pub(super) cli: CliHeader,
    pub(super) type_names: Vec<String>,
    pub(super) method_names: Vec<String>,
}

#[derive(Debug, Clone)]
pub(super) struct LdstrInstruction {
    pub(super) type_name: String,
    pub(super) method_name: String,
    pub(super) il_offset: usize,
    pub(super) occurrence: usize,
    pub(super) source: String,
    pub(super) operand_rva: u32,
}

#[derive(Debug, Clone)]
pub(super) struct MethodBody {
    pub(super) header: Vec<u8>,
    pub(super) code_rva: u32,
    pub(super) code: Vec<u8>,
}

pub(super) struct UserStringAppender {
    pub(super) data: Vec<u8>,
    pub(super) offsets: HashMap<String, u32>,
}

#[derive(Debug, Clone)]
pub(super) struct BinaryReaderUnicodeTokens {
    pub(super) read_bytes: BTreeSet<u32>,
    pub(super) get_default: BTreeSet<u32>,
    pub(super) get_unicode: BTreeSet<u32>,
    pub(super) get_string: BTreeSet<u32>,
}

#[derive(Debug, Clone)]
pub(super) struct PatchedBinaryReaderUnicodeCode {
    pub(super) code: Vec<u8>,
    pub(super) sites: usize,
}

#[derive(Debug, Clone)]
pub(super) struct PatchedManagedMethod {
    pub(super) method_index: u32,
    pub(super) expected_rva: u32,
    pub(super) body: Vec<u8>,
    pub(super) new_rva: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum IlOpcode {
    Single(u8),
    Extended(u8),
}

#[derive(Debug, Clone)]
pub(super) struct IlInstruction {
    pub(super) offset: usize,
    pub(super) opcode: IlOpcode,
    pub(super) operand_offset: usize,
    pub(super) operand_len: usize,
    pub(super) size: usize,
}

#[derive(Debug, Clone)]
pub(super) struct ResourceLocatorCharacterTemplate {
    pub(super) load_www_field_token: [u8; 4],
    pub(super) hashtable_type_token: [u8; 4],
    pub(super) hashtable_get_item_token: [u8; 4],
    pub(super) asset_bundle_type_token: [u8; 4],
    pub(super) object_inequality_token: [u8; 4],
    pub(super) asset_bundle_contains_token: [u8; 4],
}

impl AssemblyContext {
    pub(super) fn from_path(path: &Path) -> Result<Self, String> {
        let bytes = fs::read(path).map_err(|e| e.to_string())?;
        Self::from_bytes(path, &bytes)
    }
    pub(super) fn from_bytes(path: &Path, bytes: &[u8]) -> Result<Self, String> {
        let pe = PE::parse(bytes).map_err(|err| format!("{}: {err}", path.display()))?;
        let cli = read_cli_header(&pe)?;
        let metadata_bytes = pe
            .read_at_rva(cli.metadata_rva, cli.metadata_size as usize)
            .ok_or_else(|| {
                format!(
                    "{}: metadata RVA 0x{:x} size {} is out of range",
                    path.display(),
                    cli.metadata_rva,
                    cli.metadata_size
                )
            })?;
        let metadata_bytes = metadata_bytes.to_vec();
        let metadata = Metadata::parse(&metadata_bytes)
            .map_err(|err| format!("{}: could not parse CLR metadata: {err}", path.display()))?;
        let type_names = build_type_names(&metadata);
        let method_names = build_method_names(&metadata, &type_names);
        Ok(Self {
            pe,
            metadata,
            metadata_bytes,
            cli,
            type_names,
            method_names,
        })
    }
}

impl UserStringAppender {
    pub(super) fn from_metadata(metadata: &Metadata) -> Self {
        let mut offsets = HashMap::new();
        for (offset, value) in metadata.user_strings.iter() {
            offsets.entry(value).or_insert(offset);
        }
        Self {
            data: metadata.user_strings.data().to_vec(),
            offsets,
        }
    }

    pub(super) fn add(&mut self, value: &str) -> Result<u32, String> {
        if let Some(offset) = self.offsets.get(value).copied() {
            return Ok(offset);
        }
        if self.data.len() > 0x00ff_ffff {
            return Err("#US heap is too large for a user string token".to_string());
        }
        let offset = self.data.len() as u32;
        append_user_string(&mut self.data, value)?;
        self.offsets.insert(value.to_string(), offset);
        Ok(offset)
    }

    pub(super) fn into_bytes(self) -> Vec<u8> {
        self.data
    }
}
