use super::*;

pub(super) fn parse_ogg_vorbis_metadata(data: &[u8]) -> Option<OggVorbisMetadata> {
    if data.len() < 27 || &data[..4] != b"OggS" {
        return None;
    }
    let mut cursor = 0usize;
    let mut stream_serial = None;
    let mut sample_rate = None;
    let mut last_granule_position = None;
    while cursor + 27 <= data.len() {
        if &data[cursor..cursor + 4] != b"OggS" {
            return None;
        }
        let page_segments = *data.get(cursor + 26)? as usize;
        let segment_table_start = cursor + 27;
        let segment_table_end = segment_table_start.checked_add(page_segments)?;
        let page_header_end = segment_table_end;
        if page_header_end > data.len() {
            return None;
        }
        let body_len = data[segment_table_start..segment_table_end]
            .iter()
            .map(|value| *value as usize)
            .sum::<usize>();
        let body_start = page_header_end;
        let body_end = body_start.checked_add(body_len)?;
        if body_end > data.len() {
            return None;
        }
        let serial = u32::from_le_bytes(data[cursor + 14..cursor + 18].try_into().ok()?);
        let granule_position = u64::from_le_bytes(data[cursor + 6..cursor + 14].try_into().ok()?);
        if stream_serial.is_none() {
            stream_serial = Some(serial);
        }
        if stream_serial == Some(serial) {
            last_granule_position = Some(granule_position);
            if sample_rate.is_none() && body_len >= 16 {
                let packet = &data[body_start..body_end];
                if packet.first() == Some(&1) && packet.get(1..7) == Some(b"vorbis") {
                    sample_rate = Some(u32::from_le_bytes(packet[12..16].try_into().ok()?));
                }
            }
        }
        cursor = body_end;
    }
    let sample_rate = sample_rate?;
    let granule_position = last_granule_position?;
    if sample_rate == 0 {
        return None;
    }
    Some(OggVorbisMetadata {
        sample_rate,
        duration_seconds: granule_position as f64 / sample_rate as f64,
    })
}

pub(super) fn read_json(path: &Path) -> Result<JsonValue, String> {
    let text = fs::read_to_string(path).map_err(|err| format!("{}: {err}", path.display()))?;
    serde_json::from_str(text.trim_start_matches('\u{feff}'))
        .map_err(|err| format!("{}: {err}", path.display()))
}
