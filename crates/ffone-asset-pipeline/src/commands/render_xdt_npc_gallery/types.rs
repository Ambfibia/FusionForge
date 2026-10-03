use super::*;

#[derive(Debug)]
pub(super) struct Options {
    pub(super) xdt: PathBuf,
    pub(super) asset_root: PathBuf,
    pub(super) output: PathBuf,
    pub(super) preview: Option<PathBuf>,
    pub(super) legacy_plan: Option<PathBuf>,
    pub(super) blocker_reports: Vec<PathBuf>,
    pub(super) texture_metadata: Vec<PathBuf>,
    pub(super) texture_catalog_output: Option<PathBuf>,
    pub(super) frames: u64,
    pub(super) timeout: f64,
    pub(super) max_models: Option<usize>,
    pub(super) resume: bool,
    pub(super) command: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct EntityRecord {
    pub(super) row_index: usize,
    pub(super) npc_number: i64,
    pub(super) npc_name: String,
    pub(super) role: String,
    pub(super) team: i64,
    pub(super) hnpc: bool,
    pub(super) npc_class: i64,
    pub(super) mesh_index: usize,
    pub(super) model_stem: String,
    pub(super) legacy_route: String,
    pub(super) table_scale: f64,
    pub(super) texture: Option<String>,
    pub(super) texture2: Option<String>,
    pub(super) texture_present: Option<bool>,
    pub(super) texture2_present: Option<bool>,
    pub(super) texture_path: Option<String>,
    pub(super) texture2_path: Option<String>,
    pub(super) texture_resolution: Option<String>,
    pub(super) texture2_resolution: Option<String>,
    pub(super) texture_sampler_resolution: Option<String>,
    pub(super) texture2_sampler_resolution: Option<String>,
    pub(super) registry_id: Option<String>,
    pub(super) registry_category: Option<String>,
    pub(super) logical_name: Option<String>,
    pub(super) glb: Option<String>,
    pub(super) selected_animation: Option<String>,
    pub(super) status: String,
    pub(super) blockers: Vec<String>,
    pub(super) image: String,
    pub(super) model_report: Option<String>,
    pub(super) appearance_key: Option<String>,
}

#[derive(Default)]
pub(super) struct LegacyEvidence {
    pub(super) ready: BTreeSet<String>,
    pub(super) blockers: BTreeMap<String, BTreeSet<String>>,
}

impl Options {
    pub(super) fn parse(raw_args: Vec<OsString>) -> Result<Self, String> {
        let command = env::args().collect::<Vec<_>>();
        let args = raw_args
            .into_iter()
            .map(|arg| {
                arg.into_string()
                    .map_err(|_| "arguments must be valid Unicode".to_owned())
            })
            .collect::<Result<Vec<_>, _>>()?;
        if args
            .iter()
            .any(|arg| matches!(arg.as_str(), "-h" | "--help"))
        {
            return Err(HELP.to_owned());
        }
        let mut xdt = None;
        let mut asset_root = None;
        let mut output = None;
        let mut preview = None;
        let mut legacy_plan = None;
        let mut blocker_reports = Vec::new();
        let mut texture_metadata = Vec::new();
        let mut texture_catalog_output = None;
        let mut frames = 900_u64;
        let mut timeout = 45.0_f64;
        let mut max_models = None;
        let mut resume = false;
        let mut index = 0;
        while index < args.len() {
            let flag = args[index].as_str();
            if flag == "--resume" {
                resume = true;
                index += 1;
                continue;
            }
            let value = args
                .get(index + 1)
                .ok_or_else(|| format!("{flag} requires a value"))?;
            match flag {
                "--xdt" => set_once(&mut xdt, PathBuf::from(value), flag)?,
                "--asset-root" => set_once(&mut asset_root, PathBuf::from(value), flag)?,
                "--output" => set_once(&mut output, PathBuf::from(value), flag)?,
                "--preview" => set_once(&mut preview, PathBuf::from(value), flag)?,
                "--legacy-plan" => {
                    set_once(&mut legacy_plan, PathBuf::from(value), flag)?;
                }
                "--blocker-report" => {
                    blocker_reports.push(PathBuf::from(value));
                }
                "--texture-metadata" => {
                    texture_metadata.push(PathBuf::from(value));
                }
                "--texture-catalog-output" => {
                    set_once(&mut texture_catalog_output, PathBuf::from(value), flag)?;
                }
                "--frames" => {
                    frames = value
                        .parse()
                        .map_err(|_| format!("invalid --frames value {value:?}"))?;
                    if frames == 0 {
                        return Err("--frames must be greater than zero".to_owned());
                    }
                }
                "--timeout" => {
                    timeout = value
                        .parse()
                        .map_err(|_| format!("invalid --timeout value {value:?}"))?;
                    if !timeout.is_finite() || timeout <= 0.0 {
                        return Err("--timeout must be a finite positive number".to_owned());
                    }
                }
                "--max-models" => {
                    let parsed = value
                        .parse::<usize>()
                        .map_err(|_| format!("invalid --max-models value {value:?}"))?;
                    if parsed == 0 {
                        return Err("--max-models must be greater than zero".to_owned());
                    }
                    max_models = Some(parsed);
                }
                _ => return Err(format!("unknown option {flag:?}")),
            }
            index += 2;
        }
        let xdt = require_file(xdt, "--xdt")?;
        let asset_root = require_directory(asset_root, "--asset-root")?;
        let output = output.ok_or("--output is required")?;
        let preview = preview.map(|path| canonical_file(&path)).transpose()?;
        let legacy_plan = legacy_plan.map(|path| canonical_file(&path)).transpose()?;
        let blocker_reports = blocker_reports
            .iter()
            .map(|path| canonical_file(path))
            .collect::<Result<Vec<_>, _>>()?;
        let texture_metadata = texture_metadata
            .iter()
            .map(|path| canonical_file(path))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            xdt,
            asset_root,
            output,
            preview,
            legacy_plan,
            blocker_reports,
            texture_metadata,
            texture_catalog_output,
            frames,
            timeout,
            max_models,
            resume,
            command,
        })
    }
}
