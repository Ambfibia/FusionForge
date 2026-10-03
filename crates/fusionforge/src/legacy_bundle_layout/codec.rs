use super::*;

pub(super) fn audio_payload<'a>(value: &'a UnityValue) -> Result<&'a [u8], String> {
    let mut payload = None::<&[u8]>;
    for key in ["audio data", "m_AudioData"] {
        let Some(field) = value.get(key) else {
            continue;
        };
        let bytes = field
            .as_bytes()
            .ok_or_else(|| format!("AudioClip field '{key}' is not a byte array"))?;
        if let Some(existing) = payload {
            if existing != bytes {
                return Err(
                    "AudioClip has conflicting 'audio data' and 'm_AudioData' payloads".to_string(),
                );
            }
        } else {
            payload = Some(bytes);
        }
    }
    let payload = payload.ok_or_else(|| {
        "AudioClip has neither embedded 'audio data' nor 'm_AudioData'".to_string()
    })?;
    if payload.is_empty() {
        return Err("AudioClip embedded payload is empty".to_string());
    }
    Ok(payload)
}
