//! Source-pinned conversion of the accepted quit-menu surface and its handlers.
use super::unity::{UnityEnvironment, UnityValue};
use ffone_ui_layout::quit_menu::{
    QuitMenuAction, QuitMenuControl, QuitMenuDocument, QuitMenuStyle, QUIT_MENU_DOCUMENT_PATH,
};
use image::{codecs::png::PngEncoder, ImageEncoder};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

const RECIPE: &str = include_str!("../../../../recipes/native/ui/quit-menu-direct.json");

pub(super) fn run(args: &[String]) -> Result<(), String> {
    if args.len() < 3 || args[1] != "quit-menu" {
        return Err("convert-native-ui <raw-build-root> quit-menu <native-asset-root> [--text-metrics <capture.json>] [--check] [--replace-existing]; unsupported UI adapters fail closed".into());
    }
    let mut check_only = false;
    let mut replace_existing = false;
    let mut capture = None;
    let mut options = args[3..].iter();
    while let Some(option) = options.next() {
        match option.as_str() {
            "--check" if !check_only => check_only = true,
            "--replace-existing" if !replace_existing => replace_existing = true,
            "--text-metrics" if capture.is_none() => {
                capture = Some(
                    options
                        .next()
                        .ok_or("--text-metrics requires a capture path")?,
                );
            }
            _ => {
                return Err(format!(
                    "unknown or duplicate UI conversion option {option}"
                ))
            }
        }
    }
    let recipe: Value = serde_json::from_str(RECIPE).map_err(|e| e.to_string())?;
    let source = Path::new(&args[0]).join("main.unity3d");
    let bytes = std::fs::read(&source).map_err(|e| e.to_string())?;
    if format!("{:x}", Sha256::digest(&bytes))
        != recipe["sourceSha256"]
            .as_str()
            .ok_or("recipe hash missing")?
    {
        return Err("unsupported quit-menu source revision; source hash differs from the accepted behavior recipe".into());
    }
    let output = super::workspace::resolve_destination(Path::new(&args[2]))?;
    let source_root = Path::new(&args[0])
        .canonicalize()
        .map_err(|e| e.to_string())?;
    if output.starts_with(source_root) {
        return Err("output cannot be inside the immutable source build".into());
    }
    let env = UnityEnvironment::from_assets(super::direct_input::read_assets(&source)?);
    let index = env
        .assets
        .iter()
        .position(|a| a.name == "sharedassets0.assets")
        .ok_or("missing UI serialized asset")?;
    let object = |id: i64| -> Result<UnityValue, String> {
        let asset = &env.assets[index];
        asset.read_object(
            index,
            asset
                .objects
                .get(&id)
                .ok_or_else(|| format!("missing UI object {id}"))?,
        )
    };
    let owner = object(recipe["component"].as_i64().ok_or("missing component")?)?;
    let skin_ptr = owner
        .get("pSkin")
        .and_then(UnityValue::as_pointer)
        .ok_or("missing skin pointer")?;
    if skin_ptr.file_id != 0 {
        return Err("unsupported external quit skin".into());
    }
    let skin = object(skin_ptr.path_id)?;
    let styles = skin
        .get("customStyles")
        .and_then(UnityValue::as_array)
        .ok_or("missing custom styles")?;
    let find = |name: &str| -> Result<&UnityValue, String> {
        let matches = styles
            .iter()
            .filter(|s| s.get("m_Name").and_then(UnityValue::as_str) == Some(name))
            .collect::<Vec<_>>();
        match matches.as_slice() {
            [style] => Ok(*style),
            _ => Err(format!("missing/ambiguous UI style {name}")),
        }
    };
    let paths: BTreeMap<String, String> =
        serde_json::from_value(recipe["texturePaths"].clone()).map_err(|e| e.to_string())?;
    let mut files = Vec::new();
    for (id, route) in &paths {
        let value = object(id.parse::<i64>().map_err(|e| e.to_string())?)?;
        let mut rgba = super::decode_texture(&env, &value)
            .and_then(|t| t.image())
            .ok_or("unsupported UI texture")?;
        image::imageops::flip_vertical_in_place(&mut rgba);
        let mut png = Vec::new();
        PngEncoder::new(&mut png)
            .write_image(
                rgba.as_raw(),
                rgba.width(),
                rgba.height(),
                image::ColorType::Rgba8.into(),
            )
            .map_err(|e| e.to_string())?;
        // Preserve accepted PNG encodings only when every decoded pixel agrees.
        if let Ok(existing) = std::fs::read(output.join(route)) {
            if image::load_from_memory(&existing)
                .map_err(|e| e.to_string())?
                .to_rgba8()
                != rgba
            {
                return Err(format!("UI texture conflict {route}"));
            }
            png = existing;
        }
        files.push((PathBuf::from(route), png));
    }
    let route = |style: &UnityValue, state: &str| -> Result<String, String> {
        let p = style
            .get(state)
            .and_then(|s| s.get("m_Background"))
            .and_then(UnityValue::as_pointer)
            .ok_or("missing state image")?;
        if p.file_id != 0 {
            return Err("unsupported external UI state image".into());
        }
        paths
            .get(&p.path_id.to_string())
            .cloned()
            .ok_or("unmapped UI image dependency".into())
    };
    let color = |style: &UnityValue, state: &str| -> Result<[f32; 4], String> {
        let c = style
            .get(state)
            .and_then(|s| s.get("m_TextColor"))
            .ok_or("missing state color")?;
        Ok([
            number(c, "r")?,
            number(c, "g")?,
            number(c, "b")?,
            number(c, "a")?,
        ])
    };
    let convert_style = |s: &UnityValue| -> Result<QuitMenuStyle, String> {
        let font = s
            .get("m_Font")
            .and_then(UnityValue::as_pointer)
            .ok_or("missing explicit UI font")?;
        if font.file_id != 0 || font.path_id != recipe["fontObject"].as_i64().unwrap_or(-1) {
            return Err("unsupported font mapping".into());
        }
        if number(s, "m_Alignment")? != 4.0 {
            return Err("unsupported quit text alignment".into());
        }
        let offset = s.get("m_ContentOffset").ok_or("missing text offset")?;
        let active_pointer = s
            .get("m_Active")
            .and_then(|s| s.get("m_Background"))
            .and_then(UnityValue::as_pointer)
            .ok_or("missing active state image")?;
        // A null state background falls back to Normal in the legacy style.
        let active = if active_pointer.path_id == 0 {
            route(s, "m_Normal")?
        } else {
            route(s, "m_Active")?
        };
        Ok(QuitMenuStyle {
            background_box: ffone_ui_layout::quit_menu::QuitMenuBackgroundBox::BorderBox,
            normal: route(s, "m_Normal")?,
            hover: route(s, "m_Hover")?,
            active: Some(active),
            border: insets(s, "m_Border")?,
            padding: insets(s, "m_Padding")?,
            normal_color: color(s, "m_Normal")?,
            hover_color: color(s, "m_Hover")?,
            active_color: color(s, "m_Active")?,
            word_wrap: number(s, "m_WordWrap")? != 0.0,
            clips_text: number(s, "m_TextClipping")? != 0.0,
            content_offset: [number(offset, "x")?, number(offset, "y")?],
            font_compensation: [0.0; 2],
        })
    };
    let rect = owner.get("rectButton").ok_or("missing control geometry")?;
    let first = [
        number(rect, "x")?,
        number(rect, "y")?,
        number(rect, "width")?,
        number(rect, "height")?,
    ];
    let gap = recipe["buttonGap"].as_f64().ok_or("missing button gap")? as f32;
    let mut buttons = Vec::new();
    for (i, row) in recipe["controls"]
        .as_array()
        .ok_or("missing controls")?
        .iter()
        .enumerate()
    {
        let action: QuitMenuAction =
            serde_json::from_value(row["action"].clone()).map_err(|e| e.to_string())?;
        buttons.push(QuitMenuControl {
            action,
            localization_key: row["key"].as_str().ok_or("missing key")?.into(),
            fallback: row["fallback"].as_str().ok_or("missing fallback")?.into(),
            rect: [
                first[0],
                first[1] + i as f32 * (first[3] + gap),
                first[2],
                first[3],
            ],
            style: row["style"].as_u64().ok_or("missing style")? as usize,
        });
    }
    let dialog_style = find("dlg")?;
    let mut document = QuitMenuDocument {
        schema: "ffone.quit-menu.v1".into(),
        dialog_size: serde_json::from_value(recipe["dialogSize"].clone())
            .map_err(|e| e.to_string())?,
        reference_height: recipe["referenceHeight"].as_f64().ok_or("missing scale")? as f32,
        scale_nudge: recipe["scaleNudge"].as_f64().ok_or("missing scale")? as f32,
        backdrop: route(find("back")?, "m_Normal")?,
        dialog: route(dialog_style, "m_Normal")?,
        font: recipe["replacementFont"]
            .as_str()
            .ok_or("missing replacement font")?
            .into(),
        font_size: recipe["fontSize"].as_f64().ok_or("missing font size")? as f32,
        line_height: recipe["lineHeight"].as_f64().ok_or("missing line height")? as f32,
        styles: [
            convert_style(skin.get("m_button").ok_or("missing button style")?)?,
            convert_style(find("CancelButton")?)?,
        ],
        buttons: buttons
            .try_into()
            .map_err(|_| "quit adapter requires three controls")?,
    };
    document.validate()?;
    if !output.join(&document.font).is_file() {
        return Err("approved native replacement font is absent".into());
    }
    for locale in ["en", "ru"] {
        let bundle: Value = serde_json::from_slice(
            &std::fs::read(output.join(format!("localization/{locale}.json")))
                .map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        for control in &document.buttons {
            if bundle["entries"]
                .get(&control.localization_key)
                .and_then(Value::as_str)
                .is_none()
            {
                return Err(format!("missing {locale} key {}", control.localization_key));
            }
        }
    }
    if let Some(capture) = capture {
        let font = std::fs::read(output.join(&document.font)).map_err(|e| e.to_string())?;
        let font_id = recipe["fontObject"].as_i64().ok_or("missing font object")?;
        let asset = &env.assets[index];
        let raw_font =
            asset.object_raw_data(asset.objects.get(&font_id).ok_or("missing font object")?)?;
        super::unity_text_adapter::calibrate_quit_menu(
            &mut document,
            &std::fs::read(capture).map_err(|e| e.to_string())?,
            recipe["sourceSha256"]
                .as_str()
                .ok_or("missing source hash")?,
            font_id,
            &format!("{:x}", Sha256::digest(raw_font)),
            &format!("{:x}", Sha256::digest(&font)),
            &output,
        )?;
    }
    document.validate()?;
    let mut data = serde_json::to_vec_pretty(&document).map_err(|e| e.to_string())?;
    data.push(b'\n');
    files.push((QUIT_MENU_DOCUMENT_PATH.into(), data));
    if check_only {
        let font = object(recipe["fontObject"].as_i64().ok_or("missing font object")?)?;
        let metrics = ["m_FontSize", "m_Ascent", "m_LineSpacing"]
            .into_iter()
            .filter_map(|field| font.get(field).map(|value| (field, value.to_json_sample())))
            .collect::<BTreeMap<_, _>>();
        println!(
            "source font metrics: {}",
            serde_json::to_string(&metrics).map_err(|e| e.to_string())?
        );
        for (index, style) in document.styles.iter().enumerate() {
            println!(
                "quit style {index}: active={} contentOffset={:?} fontCompensation={:?}",
                style.active.as_deref().unwrap_or("accepted fallback"),
                style.content_offset,
                style.font_compensation
            );
        }
        ffone_asset_pipeline::direct_output::check_with_permission(
            &output,
            &files,
            replace_existing,
        )?;
        println!("Validated quit-menu conversion; no files written");
        return Ok(());
    }
    ffone_asset_pipeline::direct_output::install_with_permission(
        &output,
        &files,
        replace_existing,
    )?;
    println!(
        "Converted quit-menu: native description, six image dependencies and three typed actions; retained EN/RU and replacement font"
    );
    Ok(())
}

fn number(value: &UnityValue, key: &str) -> Result<f32, String> {
    let v = value.get(key).ok_or_else(|| format!("missing {key}"))?;
    let n = match v {
        UnityValue::Float(n) => *n,
        UnityValue::Int(n) => *n as f64,
        UnityValue::UInt(n) => *n as f64,
        _ => return Err(format!("non-numeric {key}")),
    };
    if !n.is_finite() {
        return Err(format!("non-finite {key}"));
    }
    Ok(n as f32)
}
fn insets(value: &UnityValue, key: &str) -> Result<[f32; 4], String> {
    let v = value.get(key).ok_or("missing insets")?;
    Ok([
        number(v, "m_Left")?,
        number(v, "m_Right")?,
        number(v, "m_Top")?,
        number(v, "m_Bottom")?,
    ])
}
