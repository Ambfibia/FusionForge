use super::*;

pub(super) fn run_character_runtime_texture_registration(
    args: &[std::ffi::OsString],
) -> Result<String, String> {
    if args.len() != 2 {
        return Err(format!(
            "register-character-runtime-textures requires <ASSET_ROOT>\n\n{USAGE}"
        ));
    }
    let entry = register_character_creation_runtime_textures(PathBuf::from(&args[1]))
        .map_err(|error| error.to_string())?;
    Ok(format!(
        "registered {} ({} bytes, blake3={})",
        entry.path, entry.bytes, entry.blake3
    ))
}
