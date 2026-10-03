use super::*;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ModelRenderRecord {
    pub(super) appearance_key: String,
    pub(super) registry_id: String,
    pub(super) glb: String,
    pub(super) glb_blake3: String,
    pub(super) logical_name: String,
    pub(super) available_animations: Vec<String>,
    pub(super) has_stand1: bool,
    pub(super) selected_animation: Option<String>,
    pub(super) representative_scale: f64,
    pub(super) main_texture: Option<String>,
    pub(super) sub_texture: Option<String>,
    pub(super) main_sampler: Option<NativeSampler>,
    pub(super) sub_sampler: Option<NativeSampler>,
    pub(super) status: String,
    pub(super) screenshot: String,
    pub(super) report: String,
    pub(super) log: String,
    pub(super) exit_code: Option<i32>,
}

pub(super) fn render_models<'a>(
    options: &Options,
    tasks: impl Iterator<Item = &'a ModelTask>,
) -> Result<Vec<ModelRenderRecord>, String> {
    let mut results = Vec::new();
    for (index, task) in tasks.enumerate() {
        let log_relative = relative_to(&options.output, &task.log)?;
        let mut result = ModelRenderRecord {
            appearance_key: task.key.clone(),
            registry_id: task.model.id.clone(),
            glb: task.model.glb.clone(),
            glb_blake3: task.model.glb_blake3.clone(),
            logical_name: task.model.logical_name.clone(),
            available_animations: task.model.animations.clone(),
            has_stand1: task
                .model
                .animations
                .iter()
                .any(|name| name.eq_ignore_ascii_case("stand1")),
            selected_animation: task.animation.clone(),
            representative_scale: task.scale,
            main_texture: task.main_texture.clone(),
            sub_texture: task.sub_texture.clone(),
            main_sampler: task.main_sampler.clone(),
            sub_sampler: task.sub_sampler.clone(),
            status: "not_rendered".to_owned(),
            screenshot: task.relative_screenshot.clone(),
            report: task.relative_report.clone(),
            log: log_relative,
            exit_code: None,
        };
        if options.max_models.is_some_and(|limit| index >= limit) {
            result.status = "not_rendered_limit".to_owned();
            results.push(result);
            continue;
        }
        let Some(preview) = options.preview.as_deref() else {
            result.status = "not_rendered".to_owned();
            results.push(result);
            continue;
        };
        if options.resume && task.screenshot.is_file() && task.report.is_file() {
            result.status = "rendered".to_owned();
            results.push(result);
            continue;
        }
        if options.resume
            && !task.screenshot.exists()
            && task.report.is_file()
            && task.log.is_file()
        {
            result.status = "render_failed".to_owned();
            results.push(result);
            continue;
        }
        if task.screenshot.exists() || task.report.exists() || task.log.exists() {
            return Err(format!(
                "partial model output already exists for {:?}; use a fresh output or a valid --resume tree",
                task.key
            ));
        }
        if let Some(parent) = task.screenshot.parent() {
            fs::create_dir_all(parent).map_err(|error| {
                format!("cannot create model output {}: {error}", parent.display())
            })?;
        }
        let mut command = Command::new(preview);
        command
            .arg("--asset-root")
            .arg(&options.asset_root)
            .arg("--model")
            .arg(&task.model.glb)
            .arg("--screenshot")
            .arg(&task.screenshot)
            .arg("--character-kind")
            .arg("npc")
            .arg("--true-root")
            .arg(&task.model.logical_name)
            .arg("--npc-scale")
            .arg(task.scale.to_string())
            .arg("--camera-view")
            .arg("reverse")
            // Gallery orientation is an explicit diagnostic contract. A slow
            // first frame may be retried after warmup, but must never silently
            // turn the model around and publish its back as a successful row.
            .arg("--blank-camera-retry")
            .arg("same")
            .arg("--report")
            .arg(&task.report)
            .arg("--frames")
            .arg(options.frames.to_string())
            .arg("--timeout")
            .arg(options.timeout.to_string())
            .arg("--outline")
            .arg("source");
        if let Some(animation) = task.animation.as_deref() {
            command.arg("--animation-name").arg(animation);
        }
        if let Some(texture) = task.main_texture.as_deref() {
            command.arg("--main-texture").arg(texture);
        }
        if let Some(texture) = task.sub_texture.as_deref() {
            command.arg("--sub-texture").arg(texture);
        }
        if let Some(sampler) = task.main_sampler.as_ref() {
            command
                .arg("--main-sampler")
                .arg(serde_json::to_string(sampler).map_err(|error| {
                    format!("cannot serialize main sampler for {}: {error}", task.key)
                })?);
        }
        if let Some(sampler) = task.sub_sampler.as_ref() {
            command
                .arg("--sub-sampler")
                .arg(serde_json::to_string(sampler).map_err(|error| {
                    format!("cannot serialize sub sampler for {}: {error}", task.key)
                })?);
        }
        let output = command.output().map_err(|error| {
            format!(
                "cannot start GPU preview {} for {}: {error}",
                preview.display(),
                task.model.id
            )
        })?;
        result.exit_code = output.status.code();
        let mut log = Vec::new();
        log.extend_from_slice(b"--- stdout ---\n");
        log.extend_from_slice(&output.stdout);
        log.extend_from_slice(b"\n--- stderr ---\n");
        log.extend_from_slice(&output.stderr);
        fs::write(&task.log, log)
            .map_err(|error| format!("cannot write {}: {error}", task.log.display()))?;
        result.status = if output.status.success()
            && task
                .screenshot
                .metadata()
                .is_ok_and(|metadata| metadata.len() > 0)
            && task.report.is_file()
        {
            "rendered".to_owned()
        } else {
            "render_failed".to_owned()
        };
        results.push(result);
    }
    Ok(results)
}
