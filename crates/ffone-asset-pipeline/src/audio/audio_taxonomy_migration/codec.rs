use super::*;

pub(super) fn ogg_packet_hash(path: &Path) -> Result<String> {
    let bytes = fs::read(path).map_err(|source| io_at(path, source))?;
    ogg_packet_hash_bytes(&bytes).map_err(|reason| {
        invalid_error(format!(
            "invalid Ogg packet stream at {}: {reason}",
            path.display()
        ))
    })
}

pub(super) fn ogg_packet_hash_bytes(bytes: &[u8]) -> std::result::Result<String, String> {
    let mut cursor = 0_usize;
    let mut pending_packet = Vec::new();
    let mut packet_count = 0_u64;
    let mut hasher = blake3::Hasher::new();
    while cursor < bytes.len() {
        let fixed_end = cursor
            .checked_add(27)
            .ok_or_else(|| "Ogg page header offset overflow".to_owned())?;
        let fixed = bytes
            .get(cursor..fixed_end)
            .ok_or_else(|| "truncated Ogg page header".to_owned())?;
        if &fixed[..4] != b"OggS" || fixed[4] != 0 {
            return Err("invalid Ogg capture pattern or version".to_owned());
        }
        let segment_count = fixed[26] as usize;
        let lacing_end = fixed_end
            .checked_add(segment_count)
            .ok_or_else(|| "Ogg lacing table offset overflow".to_owned())?;
        let lacing = bytes
            .get(fixed_end..lacing_end)
            .ok_or_else(|| "truncated Ogg lacing table".to_owned())?;
        let body_bytes = lacing.iter().try_fold(0_usize, |total, value| {
            total
                .checked_add(*value as usize)
                .ok_or_else(|| "Ogg page body length overflow".to_owned())
        })?;
        let page_end = lacing_end
            .checked_add(body_bytes)
            .ok_or_else(|| "Ogg page body offset overflow".to_owned())?;
        bytes
            .get(lacing_end..page_end)
            .ok_or_else(|| "truncated Ogg page body".to_owned())?;

        let mut body_cursor = lacing_end;
        for segment_bytes in lacing.iter().copied().map(usize::from) {
            let segment_end = body_cursor
                .checked_add(segment_bytes)
                .ok_or_else(|| "Ogg packet segment offset overflow".to_owned())?;
            let segment = bytes
                .get(body_cursor..segment_end)
                .ok_or_else(|| "truncated Ogg packet segment".to_owned())?;
            pending_packet.extend_from_slice(segment);
            body_cursor = segment_end;
            if segment_bytes < 255 {
                // Vorbis comments are non-audio metadata and are routinely
                // rewritten by bundle extractors. Keep identification, codec
                // setup and every audio packet in the identity, but ignore
                // only the 0x03 + "vorbis" comment packet.
                if !pending_packet.starts_with(b"\x03vorbis") {
                    let packet_bytes = u64::try_from(pending_packet.len())
                        .map_err(|_| "Ogg packet length does not fit u64".to_owned())?;
                    hasher.update(&packet_bytes.to_le_bytes());
                    hasher.update(&pending_packet);
                    packet_count += 1;
                }
                pending_packet.clear();
            }
        }
        cursor = page_end;
    }
    if cursor != bytes.len() || !pending_packet.is_empty() || packet_count == 0 {
        return Err("incomplete or empty Ogg packet stream".to_owned());
    }
    Ok(hasher.finalize().to_hex().to_string())
}
