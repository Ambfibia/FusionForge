use super::*;

pub(super) fn attach_gameplay_attributes(path: &Path, contract: &JsonValue) -> Result<(), String> {
    let bytes =
        fs::read(path).map_err(|err| format!("could not read {}: {err}", path.display()))?;
    let mut document: JsonValue = serde_json::from_slice(&bytes)
        .map_err(|err| format!("could not parse {}: {err}", path.display()))?;
    let object = document
        .as_object_mut()
        .ok_or_else(|| format!("{} root is not an object", path.display()))?;
    if object
        .insert("gameplayAttributes".to_string(), contract.clone())
        .is_some()
    {
        return Err(format!(
            "{} already contains gameplayAttributes",
            path.display()
        ));
    }
    let mut encoded = serde_json::to_vec_pretty(&document)
        .map_err(|err| format!("could not encode {}: {err}", path.display()))?;
    encoded.push(b'\n');
    let mut file = OpenOptions::new()
        .write(true)
        .truncate(true)
        .open(path)
        .map_err(|err| format!("could not reopen {}: {err}", path.display()))?;
    file.write_all(&encoded)
        .map_err(|err| format!("could not rewrite {}: {err}", path.display()))?;
    file.sync_all()
        .map_err(|err| format!("could not sync {}: {err}", path.display()))
}
