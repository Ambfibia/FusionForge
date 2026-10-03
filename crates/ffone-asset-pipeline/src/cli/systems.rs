use super::*;

pub(super) fn run_tutorial_npc_building_update(args: &[std::ffi::OsString]) -> Result<String, String> {
    if args
        .get(1)
        .is_some_and(|arg| arg == "--help" || arg == "-h")
    {
        return Ok(USAGE.to_owned());
    }
    if args.len() < 10 {
        return Err(format!(
            "update-tutorial-npc-building requires five roots/files, both expected SHA-256 flags, and optional --apply\n\n{USAGE}"
        ));
    }
    let mut expected_installed = None;
    let mut expected_manifest = None;
    let mut apply = false;
    let mut index = 6;
    while index < args.len() {
        let flag = args[index]
            .to_str()
            .ok_or_else(|| "update arguments must be valid UTF-8".to_owned())?;
        index += 1;
        match flag {
            "--apply" => {
                if apply {
                    return Err("--apply may only be supplied once".to_owned());
                }
                apply = true;
            }
            "--expected-installed-sha256" | "--expected-manifest-sha256" => {
                let value = args
                    .get(index)
                    .and_then(|value| value.to_str())
                    .ok_or_else(|| format!("{flag} requires a UTF-8 digest"))?
                    .to_owned();
                index += 1;
                let slot = if flag == "--expected-installed-sha256" {
                    &mut expected_installed
                } else {
                    &mut expected_manifest
                };
                if slot.replace(value).is_some() {
                    return Err(format!("{flag} may only be supplied once"));
                }
            }
            other => return Err(format!("unknown update argument {other:?}\n\n{USAGE}")),
        }
    }
    let report = update_tutorial_npc_building(
        &TutorialNpcBuildingUpdateOptions::new(
            PathBuf::from(&args[1]),
            PathBuf::from(&args[2]),
            PathBuf::from(&args[3]),
            PathBuf::from(&args[4]),
            PathBuf::from(&args[5]),
            expected_installed.ok_or("--expected-installed-sha256 is required")?,
            expected_manifest.ok_or("--expected-manifest-sha256 is required")?,
        )
        .with_apply(apply),
    )
    .map_err(|error| error.to_string())?;
    serde_json::to_string_pretty(&report)
        .map_err(|error| format!("failed to serialize npc_building update report: {error}"))
}
