use super::*;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ManagedPayloadDocument {
    pub(super) bytes: u64,
    pub(super) sha256: String,
    pub(super) materialization: ManagedMaterializationDocument,
}

pub(super) fn decode_il(code: &[u8]) -> Result<Vec<IlInstruction>, String> {
    let mut instructions = Vec::new();
    let mut offset = 0usize;
    while offset < code.len() {
        let opcode_offset = offset;
        let first = code[offset];
        offset += 1;
        let opcode = if first == 0xfe {
            let sub = *code
                .get(offset)
                .ok_or_else(|| format!("truncated extended opcode at IL_{opcode_offset:04X}"))?;
            offset += 1;
            IlOpcode::Extended(sub)
        } else {
            IlOpcode::Single(first)
        };
        let operand_offset = offset;
        let operand_len = match opcode {
            IlOpcode::Single(0x45) => {
                let count = read_u32_le(code, offset)? as usize;
                4usize
                    .checked_add(
                        count
                            .checked_mul(4)
                            .ok_or_else(|| "switch operand overflowed".to_string())?,
                    )
                    .ok_or_else(|| "switch operand overflowed".to_string())?
            }
            IlOpcode::Single(value) => single_operand_len(value).ok_or_else(|| {
                format!("unsupported/reserved IL opcode 0x{value:02X} at IL_{opcode_offset:04X}")
            })?,
            IlOpcode::Extended(value) => extended_operand_len(value).ok_or_else(|| {
                format!("unsupported/reserved IL opcode 0xFE{value:02X} at IL_{opcode_offset:04X}")
            })?,
        };
        offset = offset
            .checked_add(operand_len)
            .ok_or_else(|| "IL operand offset overflowed".to_string())?;
        if offset > code.len() {
            return Err(format!("truncated IL operand at IL_{opcode_offset:04X}"));
        }
        instructions.push(IlInstruction {
            offset: opcode_offset,
            opcode,
            operand_offset,
            operand_len,
            size: offset - opcode_offset,
        });
    }
    Ok(instructions)
}
