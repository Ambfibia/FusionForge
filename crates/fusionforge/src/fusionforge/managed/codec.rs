use super::*;

pub(super) fn metadata_stream_payload(
    metadata: &Metadata,
    original: &[u8],
    stream_name: &str,
) -> Result<Vec<u8>, String> {
    let stream = metadata
        .root
        .find_stream(stream_name)
        .ok_or_else(|| format!("CLR metadata has no {stream_name} stream"))?;
    original
        .get(stream.offset as usize..stream.offset as usize + stream.size as usize)
        .map(|value| value.to_vec())
        .ok_or_else(|| format!("metadata stream {stream_name} is out of range"))
}
