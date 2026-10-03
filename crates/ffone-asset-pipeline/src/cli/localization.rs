use super::*;

pub(super) fn run_localized_voice_install(args: &[std::ffi::OsString]) -> Result<String, String> {
    if args
        .get(1)
        .is_some_and(|arg| arg == "--help" || arg == "-h")
    {
        return Ok(USAGE.to_owned());
    }
    if args.len() < 12 {
        return Err(format!(
            "install-localized-voice requires <ASSET_ROOT> <RU_SOURCE_ROOT> <SOURCE_BUILD> --cook-report <JSON> --cook-pack <DIR> --english-root <DIR> --generated-root <DIR> [--source-catalog <V1_OR_V2_OR_V3_CATALOG_JSON>] [--preflight-only]\n\n{USAGE}"
        ));
    }
    let source_build = args[3]
        .to_str()
        .ok_or_else(|| "SOURCE_BUILD must be valid UTF-8".to_owned())?;
    let mut values = std::collections::BTreeMap::<String, PathBuf>::new();
    let mut preflight_only = false;
    let mut index = 4;
    while index < args.len() {
        let flag = args[index]
            .to_str()
            .ok_or_else(|| "voice install flag must be valid UTF-8".to_owned())?
            .to_owned();
        if flag == "--preflight-only" {
            if preflight_only {
                return Err("duplicate --preflight-only flag".to_owned());
            }
            preflight_only = true;
            index += 1;
            continue;
        }
        if !matches!(
            flag.as_str(),
            "--cook-report"
                | "--cook-pack"
                | "--english-root"
                | "--generated-root"
                | "--source-catalog"
        ) {
            return Err(format!(
                "unknown install-localized-voice flag {flag:?}\n\n{USAGE}"
            ));
        }
        let value = args
            .get(index + 1)
            .ok_or_else(|| format!("missing value after {flag}"))?;
        if values.insert(flag.clone(), PathBuf::from(value)).is_some() {
            return Err(format!("duplicate install-localized-voice flag {flag:?}"));
        }
        index += 2;
    }
    let required = |flag: &str| {
        values
            .get(flag)
            .cloned()
            .ok_or_else(|| format!("missing required install-localized-voice flag {flag}"))
    };
    let mut options = StrictVoiceInstallOptions::new(
        PathBuf::from(&args[1]),
        PathBuf::from(&args[2]),
        source_build,
        required("--cook-report")?,
        required("--cook-pack")?,
        required("--english-root")?,
        required("--generated-root")?,
    );
    if let Some(source_catalog) = values.get("--source-catalog") {
        options = options.with_source_catalog(source_catalog);
    }
    if preflight_only {
        options = options.preflight_only();
    }
    let report = install_strict_localized_voice(&options).map_err(|error| error.to_string())?;
    Ok(format!(
        "{} strict voice v3: runtimeFiles={} bytes={}, enVoice={}, ruVoice={}, localizedKeys={}, ambiguousEnglishOnly={}, ambiguousQuarantineFiles={}, ignoredMalformedFiles={}, unresolvedEnglish={}, identicalLocales={}; catalog={}, generatedReport={}, manifestFiles={}",
        if report.committed {
            "installed"
        } else {
            "preflight passed for"
        },
        report.installed_audio_files,
        report.installed_audio_bytes,
        report.english_voice_files,
        report.russian_voice_files,
        report.localized_voice_keys,
        report.english_only_ambiguous_keys,
        report.quarantined_russian_files,
        report.ignored_russian_files,
        report.unresolved_english_keys,
        report.identical_locale_payloads,
        report.catalog_path,
        report.generated_report_path,
        report.manifest_files,
    ))
}
