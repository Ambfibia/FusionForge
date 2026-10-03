//! Guarded semantic edits; untouched JSON spans retain their exact original bytes.
use serde::Deserialize;
use serde_json::{value::RawValue, Value};
use std::collections::BTreeMap;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct Edit {
    pub path: String,
    #[serde(default, deserialize_with = "present_value")]
    pub before: Option<Value>,
    pub after: Value,
}

// A missing property means insertion; an explicit JSON null is a real preimage.
fn present_value<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<Value>, D::Error> {
    Value::deserialize(d).map(Some)
}

fn route(document: &Value, pointer: &str) -> Result<String, String> {
    if let Some(tail) = pointer.strip_prefix("/@tables/") {
        let (name, tail) = tail.split_once('/').unwrap_or((tail, ""));
        let key = name.replace("~1", "/").replace("~0", "~");
        let base = if document.get(&key).is_some() {
            String::new()
        } else {
            let matches = document["tables"]
                .as_array()
                .ok_or("missing table-set")?
                .iter()
                .enumerate()
                .filter(|(_, v)| v["value"].get(&key).is_some())
                .map(|(i, _)| i)
                .collect::<Vec<_>>();
            if matches.len() != 1 {
                return Err(format!("expected one table owning {key}"));
            }
            format!("/tables/{}/value", matches[0])
        };
        Ok(format!(
            "{base}/{name}{}",
            if tail.is_empty() {
                String::new()
            } else {
                format!("/{tail}")
            }
        ))
    } else if pointer.starts_with('/') {
        Ok(pointer.into())
    } else {
        Err("JSON edit path must be a pointer".into())
    }
}

pub(super) fn apply(original: &[u8], edits: &[Edit]) -> Result<Vec<u8>, String> {
    let bom = original.starts_with(b"\xef\xbb\xbf");
    let bytes = if bom { &original[3..] } else { original };
    let text = std::str::from_utf8(bytes).map_err(|e| e.to_string())?;
    let before: Value = serde_json::from_str(text).map_err(|e| e.to_string())?;
    let mut after = before.clone();
    for edit in edits {
        let pointer = route(&after, &edit.path)?;
        let current = after.pointer(&pointer);
        if current == Some(&edit.after) {
            continue;
        }
        if current != edit.before.as_ref() {
            return Err(format!("preimage differs at {}", edit.path));
        }
        if let Some(value) = after.pointer_mut(&pointer) {
            *value = edit.after.clone();
        } else {
            let (parent, key) = pointer.rsplit_once('/').ok_or("edit has no parent")?;
            let parent = after.pointer_mut(parent).ok_or("missing edit parent")?;
            match parent {
                Value::Object(map) => {
                    map.insert(
                        key.replace("~1", "/").replace("~0", "~"),
                        edit.after.clone(),
                    );
                }
                Value::Array(rows) if key.parse::<usize>().ok() == Some(rows.len()) => {
                    rows.push(edit.after.clone());
                }
                _ => return Err(format!("cannot append at {}", edit.path)),
            }
        }
    }
    rewrite(original, &after)
}

pub(super) fn rewrite(original: &[u8], after: &Value) -> Result<Vec<u8>, String> {
    let bom = original.starts_with(b"\xef\xbb\xbf");
    let bytes = if bom { &original[3..] } else { original };
    let text = std::str::from_utf8(bytes).map_err(|e| e.to_string())?;
    let before: Value = serde_json::from_str(text).map_err(|e| e.to_string())?;
    let mut differences = Vec::new();
    changed(&before, &after, String::new(), &mut differences);
    let mut replacements = Vec::new();
    let mut spans = Vec::new();
    locations(
        text,
        &differences
            .iter()
            .enumerate()
            .map(|(i, (p, _))| (i, p.as_str()))
            .collect::<Vec<_>>(),
        &mut spans,
    )?;
    for (index, raw) in spans {
        let value = differences[index].1;
        let start = raw.as_ptr() as usize - text.as_ptr() as usize;
        let mut replacement = if value.is_array() || value.is_object() {
            serde_json::to_string_pretty(value)
        } else {
            serde_json::to_string(value)
        }
        .map_err(|e| e.to_string())?;
        let line_start = text[..start].rfind('\n').map_or(0, |i| i + 1);
        let indentation = text[line_start..].chars().take_while(|c| *c == ' ').count();
        replacement = replacement.replace('\n', &format!("\n{}", " ".repeat(indentation)));
        if text.contains("\r\n") {
            replacement = replacement.replace('\n', "\r\n");
        }
        replacements.push((start, start + raw.len(), replacement));
    }
    replacements.sort_by_key(|(start, _, _)| *start);
    let mut result = text.to_owned();
    for (start, end, replacement) in replacements.into_iter().rev() {
        result.replace_range(start..end, &replacement);
    }
    let verified: Value = serde_json::from_str(&result).map_err(|e| e.to_string())?;
    if verified != *after {
        return Err("JSON span patch failed semantic verification".into());
    }
    let mut bytes = if bom {
        b"\xef\xbb\xbf".to_vec()
    } else {
        Vec::new()
    };
    bytes.extend_from_slice(result.as_bytes());
    Ok(bytes)
}

fn changed<'a>(before: &Value, after: &'a Value, path: String, out: &mut Vec<(String, &'a Value)>) {
    if before == after {
        return;
    }
    match (before, after) {
        (Value::Object(a), Value::Object(b))
            if a.len() == b.len() && a.keys().all(|k| b.contains_key(k)) =>
        {
            for (key, v) in b {
                changed(
                    &a[key],
                    v,
                    format!("{path}/{}", key.replace('~', "~0").replace('/', "~1")),
                    out,
                );
            }
        }
        (Value::Array(a), Value::Array(b)) if a.len() == b.len() => {
            for (i, (a, b)) in a.iter().zip(b).enumerate() {
                changed(a, b, format!("{path}/{i}"), out);
            }
        }
        _ => out.push((path, after)),
    }
}

// Parse each shared ancestor once, rather than reparsing a large table for every edit.
fn locations<'a>(
    text: &'a str,
    pointers: &[(usize, &str)],
    out: &mut Vec<(usize, &'a str)>,
) -> Result<(), String> {
    let mut groups = BTreeMap::<&str, Vec<(usize, &str)>>::new();
    for &(index, pointer) in pointers {
        if pointer.is_empty() {
            out.push((index, text));
            continue;
        }
        let tail = pointer.strip_prefix('/').ok_or("invalid pointer")?;
        let split = tail.find('/').unwrap_or(tail.len());
        groups
            .entry(&tail[..split])
            .or_default()
            .push((index, &tail[split..]));
    }
    if groups.is_empty() {
        return Ok(());
    }
    if text.trim_start().starts_with('{') {
        let values: BTreeMap<String, &RawValue> =
            serde_json::from_str(text).map_err(|e| e.to_string())?;
        for (key, children) in groups {
            let value = values
                .get(&key.replace("~1", "/").replace("~0", "~"))
                .ok_or("missing JSON property")?;
            locations(value.get(), &children, out)?;
        }
    } else {
        let values: Vec<&RawValue> = serde_json::from_str(text).map_err(|e| e.to_string())?;
        for (key, children) in groups {
            let value = values
                .get(key.parse::<usize>().map_err(|e| e.to_string())?)
                .ok_or("missing JSON row")?;
            locations(value.get(), &children, out)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
