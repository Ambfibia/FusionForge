//! Bounded bitmap-font evidence. Raw vertex coordinates are not GUI baseline placement.
use crate::fusionforge::unity::UnityValue;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

pub(super) fn inspect(font: &UnityValue, text: &str, raw: &[u8]) -> Result<Value, String> {
    if text.is_empty() || text.chars().count() > 256 || text.contains(['\n', '\r']) {
        return Err("--font-text expects 1..256 scalars on one line".into());
    }
    let glyphs = font
        .get("m_CharacterRects")
        .and_then(UnityValue::as_array)
        .ok_or("Font has no bitmap character rectangles")?;
    let mut selected = Vec::new();
    for character in text.chars().collect::<BTreeSet<_>>() {
        let matches = glyphs
            .iter()
            .filter(|g| g.get("index").and_then(UnityValue::as_i64) == Some(character as i64))
            .collect::<Vec<_>>();
        let [glyph] = matches.as_slice() else {
            return Err(format!(
                "missing or ambiguous bitmap glyph U+{:04X}",
                character as u32
            ));
        };
        selected.push(
            json!({"character":character.to_string(),"serializedMetrics":glyph.to_json_sample()}),
        );
    }
    Ok(json!({
        "rawObjectSha256":format!("{:x}", Sha256::digest(raw)),
        "lineSpacing":font.get("m_LineSpacing").ok_or("Font has no line spacing")?.to_json_sample(),
        "coordinateSpace":"serializedFontVertices",
        "guiBaselineMeasured":false,
        "glyphs":selected
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    fn font() -> UnityValue {
        UnityValue::Object(BTreeMap::from([
            ("m_LineSpacing".into(), UnityValue::Float(13.71)),
            (
                "m_CharacterRects".into(),
                UnityValue::Array(vec![UnityValue::Object(BTreeMap::from([
                    ("index".into(), UnityValue::Int(65)),
                    ("width".into(), UnityValue::Float(9.0)),
                ]))]),
            ),
        ]))
    }
    #[test]
    fn exact_glyph_selection_is_bounded_and_does_not_claim_gui_baseline() {
        let output = inspect(&font(), "AAA", b"font").unwrap();
        assert_eq!(output["glyphs"].as_array().unwrap().len(), 1);
        assert_eq!(output["glyphs"][0]["serializedMetrics"]["width"], 9.0);
        assert_eq!(output["guiBaselineMeasured"], false);
        for text in ["", "A\nA", "B", &"A".repeat(257)] {
            assert!(inspect(&font(), text, b"font").is_err());
        }
    }
}
