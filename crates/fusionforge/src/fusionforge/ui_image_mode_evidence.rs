use std::{
    env, fs,
    io::{self, Write},
    path::{Component, Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub(crate) const REQUEST_SCHEMA: &str = "fusionforge.unity-ui-image-mode-request.v1";
pub(crate) const EVIDENCE_SCHEMA: &str = "fusionforge.unity-ui-image-mode-evidence.v1";
pub(crate) const NATIVE_CONTRACT_SCHEMA: &str = "ffone.native-ui-image-contract.v1";

const USAGE: &str = "export-ui-image-mode-evidence <request.json> [--out <evidence.json|->]";
const COORDINATE_SPACE: &str = "top-left origin; x right; y down";
const BORDER_ORDER: &str = "left/right/top/bottom";

#[derive(Debug)]
struct CliOptions {
    request: PathBuf,
    output: Option<PathBuf>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ImageModeRequest {
    schema: String,
    semantic_role: String,
    texture_semantic_id: String,
    control_state: ControlState,
    target_rect: Rect,
    draw_sources: Vec<DrawSource>,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ControlState {
    enabled: bool,
    on: bool,
    interaction: Interaction,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
enum Interaction {
    Normal,
    Hover,
    Active,
    Focused,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
enum DrawSource {
    DirectDrawTexture {
        overload: DirectDrawOverload,
        scale_mode: DirectScaleMode,
        texture: TextureEvidence,
    },
    GuiStyleBackground {
        style: GuiStyleEvidence,
        selected_state: GuiStyleState,
        texture: TextureEvidence,
        border: RectOffset,
        overflow: RectOffset,
    },
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
enum DirectDrawOverload {
    RectTexture,
    RectTextureScaleMode,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
enum DirectScaleMode {
    StretchToFill,
    ScaleToFit,
    ScaleAndCrop,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct GuiStyleEvidence {
    owner: UnityObjectIdentity,
    style_name: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
enum GuiStyleState {
    Normal,
    Hover,
    Active,
    Focused,
    OnNormal,
    OnHover,
    OnActive,
    OnFocused,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct TextureEvidence {
    object: UnityObjectIdentity,
    width: u32,
    height: u32,
    source_payload_sha256: String,
    base_mip_decoded_rgba_sha256: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct UnityObjectIdentity {
    source_alias: String,
    relative_container: String,
    raw_container_sha256: String,
    serialized_asset: String,
    #[serde(rename = "type")]
    unity_type: String,
    path_id: i64,
    raw_object_sha256: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RectOffset {
    left: i64,
    right: i64,
    top: i64,
    bottom: i64,
}

impl RectOffset {
    const ZERO: Self = Self {
        left: 0,
        right: 0,
        top: 0,
        bottom: 0,
    };

    fn is_zero(self) -> bool {
        self == Self::ZERO
    }

    fn as_array(self) -> [f64; 4] {
        [
            self.left as f64,
            self.right as f64,
            self.top as f64,
            self.bottom as f64,
        ]
    }
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Rect {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

impl Rect {
    fn from_edges(left: f64, top: f64, right: f64, bottom: f64) -> Self {
        Self {
            x: left,
            y: top,
            width: right - left,
            height: bottom - top,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ImageModeEvidence {
    schema: &'static str,
    schema_version: u32,
    request_sha256: String,
    semantic_role: String,
    texture_semantic_id: String,
    control_state: ResolvedControlState,
    draw_source: SelectedDrawSource,
    decision: ImageModeDecision,
    geometry: Geometry,
    native_contract: NativeImageContract,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ResolvedControlState {
    enabled: bool,
    on: bool,
    interaction: Interaction,
    canonical_state: &'static str,
    expected_gui_style_state: GuiStyleState,
}

#[derive(Clone, Debug, Serialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
enum SelectedDrawSource {
    DirectDrawTexture {
        api: &'static str,
        overload: DirectDrawOverload,
        scale_mode: DirectScaleMode,
        texture: TextureEvidence,
    },
    GuiStyleBackground {
        style: GuiStyleEvidence,
        selected_state: GuiStyleState,
        texture: TextureEvidence,
        border: RectOffset,
        overflow: RectOffset,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
enum NativeImageMode {
    Stretch,
    NineSlice,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
enum DecisionReason {
    DirectDrawTextureStretchToFill,
    GuiStyleZeroBorder,
    GuiStyleNonzeroBorder,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ImageModeDecision {
    image_mode: NativeImageMode,
    reason: DecisionReason,
    border_order: &'static str,
    border: [f64; 4],
}

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
enum Geometry {
    Stretch {
        coordinate_space: &'static str,
        source_rect: Rect,
        target_rect: Rect,
        patches: Vec<SlicePatch>,
    },
    NineSlice {
        coordinate_space: &'static str,
        source_columns: [f64; 4],
        source_rows: [f64; 4],
        target_columns: [f64; 4],
        target_rows: [f64; 4],
        patches: Vec<SlicePatch>,
    },
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SlicePatch {
    role: &'static str,
    class: &'static str,
    source_rect: Rect,
    target_rect: Rect,
    drawable: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    scale: Option<Scale2>,
}

#[derive(Clone, Copy, Debug, Serialize)]
struct Scale2 {
    x: f64,
    y: f64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct NativeImageContract {
    schema: &'static str,
    semantic_role: String,
    texture_semantic_id: String,
    contract_scope: &'static str,
    required_external_contracts: [&'static str; 4],
    coordinate_space: &'static str,
    source_units: &'static str,
    target_units: &'static str,
    source_rect: Rect,
    layout_rect: Rect,
    effective_draw_rect: Rect,
    image_mode: NativeImageMode,
    border_order: &'static str,
    border: [f64; 4],
    corner_scale: f64,
    center_scale_mode: &'static str,
    sides_scale_mode: &'static str,
}

pub(crate) fn export_ui_image_mode_evidence_cli(args: &[String]) -> Result<(), String> {
    let options = parse_cli(args)?;
    let bytes = fs::read(&options.request)
        .map_err(|error| format!("{}: {error}", options.request.display()))?;
    let request: ImageModeRequest = serde_json::from_slice(&bytes)
        .map_err(|error| format!("{}: {error}", options.request.display()))?;
    let evidence = build_evidence_with_request_sha256(request, sha256_hex(&bytes))?;
    write_json(options.output.as_deref(), &evidence)
}

fn parse_cli(args: &[String]) -> Result<CliOptions, String> {
    let Some(request) = args.first() else {
        return Err(USAGE.to_string());
    };
    if request.starts_with('-') {
        return Err(USAGE.to_string());
    }

    let mut output = None;
    let mut index = 1usize;
    while index < args.len() {
        match args[index].as_str() {
            "--out" => {
                let value = args
                    .get(index + 1)
                    .ok_or_else(|| "--out requires a JSON path or '-'".to_string())?;
                if output.replace(PathBuf::from(value)).is_some() {
                    return Err("--out may be supplied only once".to_string());
                }
                index += 2;
            }
            option => {
                return Err(format!(
                    "unknown export-ui-image-mode-evidence option '{option}'\n{USAGE}"
                ));
            }
        }
    }

    let request = PathBuf::from(request);
    if let Some(path) = output.as_deref().filter(|path| *path != Path::new("-")) {
        if same_path(&request, path)? {
            return Err("request and evidence output must be different files".to_string());
        }
    }
    Ok(CliOptions { request, output })
}

fn same_path(left: &Path, right: &Path) -> Result<bool, String> {
    let left = lexical_absolute_path(left)?;
    let right = lexical_absolute_path(right)?;
    #[cfg(windows)]
    {
        Ok(left
            .to_string_lossy()
            .eq_ignore_ascii_case(&right.to_string_lossy()))
    }
    #[cfg(not(windows))]
    {
        Ok(left == right)
    }
}

fn lexical_absolute_path(path: &Path) -> Result<PathBuf, String> {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        env::current_dir()
            .map_err(|error| format!("could not resolve current directory: {error}"))?
            .join(path)
    };
    let mut normalized = PathBuf::new();
    for component in absolute.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            Component::Prefix(_) | Component::RootDir | Component::Normal(_) => {
                normalized.push(component.as_os_str());
            }
        }
    }
    Ok(normalized)
}

fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

#[cfg(test)]
fn build_evidence(request: ImageModeRequest) -> Result<ImageModeEvidence, String> {
    build_evidence_with_request_sha256(request, sha256_hex(b"unit-test-request"))
}

fn build_evidence_with_request_sha256(
    request: ImageModeRequest,
    request_sha256: String,
) -> Result<ImageModeEvidence, String> {
    if request.schema != REQUEST_SCHEMA {
        return Err(format!(
            "unsupported request schema '{}'; expected '{REQUEST_SCHEMA}'",
            request.schema
        ));
    }
    validate_semantic_id(&request.semantic_role, "semanticRole")?;
    validate_semantic_id(&request.texture_semantic_id, "textureSemanticId")?;
    validate_rect(request.target_rect, "targetRect")?;

    let source = match request.draw_sources.as_slice() {
        [] => {
            return Err(
                "draw source is unresolved: provide exactly one directDrawTexture or guiStyleBackground entry"
                    .to_string(),
            );
        }
        [source] => source.clone(),
        sources => {
            let kinds = sources.iter().map(DrawSource::kind).collect::<Vec<_>>();
            return Err(format!(
                "draw source is ambiguous: expected exactly one entry, found {} ({})",
                sources.len(),
                kinds.join(", ")
            ));
        }
    };

    let resolved_state = resolve_control_state(request.control_state);
    let (selected, texture, border, mode, reason, effective_draw_rect) = match source {
        DrawSource::DirectDrawTexture {
            overload,
            scale_mode,
            texture,
        } => {
            validate_texture(&texture)?;
            if scale_mode != DirectScaleMode::StretchToFill {
                return Err(format!(
                    "GUI.DrawTexture scaleMode {scale_mode:?} is not a stretch/nine-slice contract; v1 accepts only stretchToFill"
                ));
            }
            (
                SelectedDrawSource::DirectDrawTexture {
                    api: "GUI.DrawTexture",
                    overload,
                    scale_mode,
                    texture: texture.clone(),
                },
                texture,
                RectOffset::ZERO,
                NativeImageMode::Stretch,
                DecisionReason::DirectDrawTextureStretchToFill,
                request.target_rect,
            )
        }
        DrawSource::GuiStyleBackground {
            style,
            selected_state,
            texture,
            border,
            overflow,
        } => {
            validate_style(&style)?;
            validate_texture(&texture)?;
            if style.owner.source_alias != texture.object.source_alias {
                return Err(format!(
                    "GUIStyle owner sourceAlias '{}' and selected Texture2D sourceAlias '{}' must share one canonical source role; Academy must be represented as 'alternate'",
                    style.owner.source_alias,
                    texture.object.source_alias
                ));
            }
            if selected_state != resolved_state.expected_gui_style_state {
                return Err(format!(
                    "GUIStyle selectedState {selected_state:?} contradicts controlState; expected {:?}",
                    resolved_state.expected_gui_style_state
                ));
            }
            let effective_draw_rect = apply_gui_style_overflow(request.target_rect, overflow)?;
            validate_border(border, &texture, effective_draw_rect)?;
            let (mode, reason) = if border.is_zero() {
                (NativeImageMode::Stretch, DecisionReason::GuiStyleZeroBorder)
            } else {
                (
                    NativeImageMode::NineSlice,
                    DecisionReason::GuiStyleNonzeroBorder,
                )
            };
            (
                SelectedDrawSource::GuiStyleBackground {
                    style,
                    selected_state,
                    texture: texture.clone(),
                    border,
                    overflow,
                },
                texture,
                border,
                mode,
                reason,
                effective_draw_rect,
            )
        }
    };

    let source_rect = Rect {
        x: 0.0,
        y: 0.0,
        width: texture.width as f64,
        height: texture.height as f64,
    };
    let geometry = match mode {
        NativeImageMode::Stretch => stretch_geometry(source_rect, effective_draw_rect),
        NativeImageMode::NineSlice => nine_slice_geometry(source_rect, effective_draw_rect, border),
    };
    let native_contract = NativeImageContract {
        schema: NATIVE_CONTRACT_SCHEMA,
        semantic_role: request.semantic_role.clone(),
        texture_semantic_id: request.texture_semantic_id.clone(),
        contract_scope: "geometryOnly",
        required_external_contracts: [
            "texturePayload",
            "sampler",
            "decodedImageOrientation",
            "colorAlphaContract",
        ],
        coordinate_space: COORDINATE_SPACE,
        source_units: "texturePixels",
        target_units: "referenceCanvasPoints",
        source_rect,
        layout_rect: request.target_rect,
        effective_draw_rect,
        image_mode: mode,
        border_order: BORDER_ORDER,
        border: border.as_array(),
        center_scale_mode: "stretch",
        corner_scale: 1.0,
        sides_scale_mode: "stretch",
    };

    Ok(ImageModeEvidence {
        schema: EVIDENCE_SCHEMA,
        schema_version: 1,
        request_sha256,
        semantic_role: request.semantic_role,
        texture_semantic_id: request.texture_semantic_id,
        control_state: resolved_state,
        draw_source: selected,
        decision: ImageModeDecision {
            image_mode: mode,
            reason,
            border_order: BORDER_ORDER,
            border: border.as_array(),
        },
        geometry,
        native_contract,
    })
}

impl DrawSource {
    fn kind(&self) -> &'static str {
        match self {
            Self::DirectDrawTexture { .. } => "directDrawTexture",
            Self::GuiStyleBackground { .. } => "guiStyleBackground",
        }
    }
}

fn resolve_control_state(state: ControlState) -> ResolvedControlState {
    let expected_gui_style_state = match (state.enabled, state.on, state.interaction) {
        (false, false, _) => GuiStyleState::Normal,
        (false, true, _) => GuiStyleState::OnNormal,
        (true, false, Interaction::Normal) => GuiStyleState::Normal,
        (true, false, Interaction::Hover) => GuiStyleState::Hover,
        (true, false, Interaction::Active) => GuiStyleState::Active,
        (true, false, Interaction::Focused) => GuiStyleState::Focused,
        (true, true, Interaction::Normal) => GuiStyleState::OnNormal,
        (true, true, Interaction::Hover) => GuiStyleState::OnHover,
        (true, true, Interaction::Active) => GuiStyleState::OnActive,
        (true, true, Interaction::Focused) => GuiStyleState::OnFocused,
    };
    let canonical_state = match (state.enabled, state.on, state.interaction) {
        (false, false, _) => "disabled",
        (false, true, _) => "onDisabled",
        (true, false, Interaction::Normal) => "normal",
        (true, false, Interaction::Hover) => "hover",
        (true, false, Interaction::Active) => "active",
        (true, false, Interaction::Focused) => "focused",
        (true, true, Interaction::Normal) => "onNormal",
        (true, true, Interaction::Hover) => "onHover",
        (true, true, Interaction::Active) => "onActive",
        (true, true, Interaction::Focused) => "onFocused",
    };
    ResolvedControlState {
        enabled: state.enabled,
        on: state.on,
        interaction: state.interaction,
        canonical_state,
        expected_gui_style_state,
    }
}

fn validate_semantic_id(value: &str, label: &str) -> Result<(), String> {
    if value.is_empty()
        || value.len() > 256
        || !value.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '.' | '_' | '-' | '/')
        })
    {
        return Err(format!(
            "{label} must be 1..=256 bytes using ASCII letters, digits, '.', '_', '-' or '/'"
        ));
    }
    if value
        .split('/')
        .any(|segment| segment.is_empty() || matches!(segment, "." | ".."))
    {
        return Err(format!(
            "{label} slash-delimited segments may not be empty, '.' or '..'"
        ));
    }
    Ok(())
}

fn validate_style(style: &GuiStyleEvidence) -> Result<(), String> {
    validate_identity(&style.owner, "GUIStyle owner")?;
    if style.owner.unity_type != "GUISkin" {
        return Err(format!(
            "GUIStyle owner must have exact Unity type 'GUISkin', found '{}'",
            style.owner.unity_type
        ));
    }
    if style.style_name.trim().is_empty() {
        return Err("GUIStyle styleName may not be empty".to_string());
    }
    Ok(())
}

fn validate_texture(texture: &TextureEvidence) -> Result<(), String> {
    validate_identity(&texture.object, "texture")?;
    if texture.object.unity_type != "Texture2D" {
        return Err(format!(
            "draw texture must have exact Unity type 'Texture2D', found '{}'",
            texture.object.unity_type
        ));
    }
    if texture.width == 0 || texture.height == 0 {
        return Err("source texture dimensions must both be positive".to_string());
    }
    validate_sha256(
        &texture.source_payload_sha256,
        "texture sourcePayloadSha256 (export-exact-texture sourcePayload.sha256)",
    )?;
    validate_sha256(
        &texture.base_mip_decoded_rgba_sha256,
        "texture baseMipDecodedRgbaSha256 (export-exact-texture mipLevels[0].decodedRgbaSha256)",
    )?;
    Ok(())
}

fn validate_identity(identity: &UnityObjectIdentity, label: &str) -> Result<(), String> {
    if !matches!(
        identity.source_alias.as_str(),
        "primary" | "patched" | "alternate"
    ) {
        return Err(format!(
            "{label} sourceAlias must be exactly 'primary', 'patched', or 'alternate'; Academy maps to canonical role 'alternate'"
        ));
    }
    validate_portable_relative_path(&identity.relative_container, "relativeContainer")?;
    validate_sha256(
        &identity.raw_container_sha256,
        &format!("{label} rawContainerSha256 (dump-object-evidence source.sha256)"),
    )?;
    if identity.serialized_asset.trim().is_empty() {
        return Err(format!("{label} serializedAsset may not be empty"));
    }
    if identity.path_id <= 0 {
        return Err(format!("{label} pathId must be positive"));
    }
    validate_sha256(
        &identity.raw_object_sha256,
        &format!("{label} rawObjectSha256 (dump-object-evidence object.rawSha256)"),
    )?;
    Ok(())
}

fn validate_sha256(value: &str, label: &str) -> Result<(), String> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(format!("{label} must be lowercase 64-hex SHA-256"));
    }
    Ok(())
}

fn validate_portable_relative_path(value: &str, label: &str) -> Result<(), String> {
    let path = Path::new(value);
    if value.is_empty() || path.is_absolute() {
        return Err(format!("{label} must be a non-empty relative path"));
    }
    let mut has_part = false;
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::Normal(_) => has_part = true,
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(format!("{label} may not escape its source root"));
            }
        }
    }
    if !has_part {
        return Err(format!("{label} must name a source-relative file"));
    }
    Ok(())
}

fn validate_rect(rect: Rect, label: &str) -> Result<(), String> {
    if ![rect.x, rect.y, rect.width, rect.height]
        .into_iter()
        .all(f64::is_finite)
    {
        return Err(format!("{label} values must be finite"));
    }
    if rect.width <= 0.0 || rect.height <= 0.0 {
        return Err(format!("{label} width and height must be positive"));
    }
    Ok(())
}

fn apply_gui_style_overflow(layout: Rect, overflow: RectOffset) -> Result<Rect, String> {
    let horizontal = overflow.left.checked_add(overflow.right).ok_or_else(|| {
        "GUIStyle horizontal RectOffset overflow sum overflows integer range".to_string()
    })?;
    let vertical = overflow.top.checked_add(overflow.bottom).ok_or_else(|| {
        "GUIStyle vertical RectOffset overflow sum overflows integer range".to_string()
    })?;
    let effective = Rect {
        x: layout.x - overflow.left as f64,
        y: layout.y - overflow.top as f64,
        width: layout.width + horizontal as f64,
        height: layout.height + vertical as f64,
    };
    validate_rect(
        effective,
        "effectiveDrawRect derived from GUIStyle overflow",
    )?;
    Ok(effective)
}

fn validate_border(
    border: RectOffset,
    texture: &TextureEvidence,
    target: Rect,
) -> Result<(), String> {
    if [border.left, border.right, border.top, border.bottom]
        .into_iter()
        .any(|value| value < 0)
    {
        return Err("GUIStyle RectOffset border values may not be negative".to_string());
    }
    let horizontal = border.left.checked_add(border.right).ok_or_else(|| {
        "GUIStyle horizontal RectOffset border sum overflows integer range".to_string()
    })?;
    let vertical = border.top.checked_add(border.bottom).ok_or_else(|| {
        "GUIStyle vertical RectOffset border sum overflows integer range".to_string()
    })?;
    if horizontal > i64::from(texture.width) || vertical > i64::from(texture.height) {
        return Err(format!(
            "impossible GUIStyle border {}/{}/{}/{} for source texture {}x{}",
            border.left, border.right, border.top, border.bottom, texture.width, texture.height
        ));
    }
    if horizontal as f64 > target.width || vertical as f64 > target.height {
        return Err(format!(
            "target rect {}x{} is smaller than the unscaled GUIStyle border totals {}x{}; exact 1:1 corner geometry is impossible",
            target.width, target.height, horizontal, vertical
        ));
    }
    Ok(())
}

fn stretch_geometry(source: Rect, target: Rect) -> Geometry {
    Geometry::Stretch {
        coordinate_space: COORDINATE_SPACE,
        source_rect: source,
        target_rect: target,
        patches: vec![patch("full", "full", source, target)],
    }
}

fn nine_slice_geometry(source: Rect, target: Rect, border: RectOffset) -> Geometry {
    let source_columns = [
        source.x,
        source.x + border.left as f64,
        source.x + source.width - border.right as f64,
        source.x + source.width,
    ];
    let source_rows = [
        source.y,
        source.y + border.top as f64,
        source.y + source.height - border.bottom as f64,
        source.y + source.height,
    ];
    let target_columns = [
        target.x,
        target.x + border.left as f64,
        target.x + target.width - border.right as f64,
        target.x + target.width,
    ];
    let target_rows = [
        target.y,
        target.y + border.top as f64,
        target.y + target.height - border.bottom as f64,
        target.y + target.height,
    ];
    let roles = [
        [
            ("topLeft", "corner"),
            ("top", "edge"),
            ("topRight", "corner"),
        ],
        [("left", "edge"), ("center", "center"), ("right", "edge")],
        [
            ("bottomLeft", "corner"),
            ("bottom", "edge"),
            ("bottomRight", "corner"),
        ],
    ];
    let mut patches = Vec::with_capacity(9);
    for row in 0..3 {
        for column in 0..3 {
            patches.push(patch(
                roles[row][column].0,
                roles[row][column].1,
                Rect::from_edges(
                    source_columns[column],
                    source_rows[row],
                    source_columns[column + 1],
                    source_rows[row + 1],
                ),
                Rect::from_edges(
                    target_columns[column],
                    target_rows[row],
                    target_columns[column + 1],
                    target_rows[row + 1],
                ),
            ));
        }
    }
    Geometry::NineSlice {
        coordinate_space: COORDINATE_SPACE,
        source_columns,
        source_rows,
        target_columns,
        target_rows,
        patches,
    }
}

fn patch(role: &'static str, class: &'static str, source: Rect, target: Rect) -> SlicePatch {
    let drawable =
        source.width > 0.0 && source.height > 0.0 && target.width > 0.0 && target.height > 0.0;
    let scale = drawable.then_some(Scale2 {
        x: target.width / source.width,
        y: target.height / source.height,
    });
    SlicePatch {
        role,
        class,
        source_rect: source,
        target_rect: target,
        drawable,
        scale,
    }
}

fn write_json(path: Option<&Path>, evidence: &ImageModeEvidence) -> Result<(), String> {
    let mut json = serde_json::to_vec_pretty(evidence).map_err(|error| error.to_string())?;
    json.push(b'\n');
    match path {
        None => io::stdout()
            .write_all(&json)
            .map_err(|error| error.to_string()),
        Some(path) if path == Path::new("-") => io::stdout()
            .write_all(&json)
            .map_err(|error| error.to_string()),
        Some(path) => {
            if let Some(parent) = path
                .parent()
                .filter(|parent| !parent.as_os_str().is_empty())
            {
                fs::create_dir_all(parent)
                    .map_err(|error| format!("{}: {error}", parent.display()))?;
            }
            fs::write(path, json).map_err(|error| format!("{}: {error}", path.display()))
        }
    }
}

#[cfg(test)]
mod tests;
