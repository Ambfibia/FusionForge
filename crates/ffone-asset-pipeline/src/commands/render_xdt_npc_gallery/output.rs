use super::*;

pub(super) fn write_html(path: &Path, summary: &Value, records: &[EntityRecord]) -> Result<(), String> {
    let counts = summary.get("counts").cloned().unwrap_or(Value::Null);
    let mut html = String::from(
        "<!doctype html><html><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width,initial-scale=1\"><title>FFOne XDT NPC render audit</title><style>\
        :root{color-scheme:dark;font-family:Segoe UI,Arial,sans-serif;background:#0d131d;color:#e7edf6}body{margin:0}header{position:sticky;top:0;z-index:2;background:#111a27ee;padding:16px;border-bottom:1px solid #33445b}h1{margin:0 0 8px;font-size:22px}.stats{color:#a9bad0;margin-bottom:10px}.filters{display:flex;gap:8px;flex-wrap:wrap}input,select{background:#1b2737;color:#fff;border:1px solid #40536d;border-radius:6px;padding:8px}main{display:grid;grid-template-columns:repeat(auto-fill,minmax(230px,1fr));gap:12px;padding:14px}.card{background:#15202e;border:1px solid #2b3b50;border-radius:9px;overflow:hidden}.card img{width:100%;aspect-ratio:1;object-fit:contain;background:#111923}.meta{padding:10px}.name{font-weight:700}.route,.details{font-size:12px;color:#a9bad0;overflow-wrap:anywhere}.status{display:inline-block;margin:6px 0;padding:3px 6px;border-radius:4px;background:#32445c;font-size:11px}.rendered .status{background:#176b4a}.hidden_location_marker .status,.primary_interaction_placeholder .status{background:#285c78}.texture_override_unresolved .status,.primary_ready_unpublished .status{background:#8b5b23}.primary_blocked .status,.render_failed .status{background:#8b303f}.not_in_primary_catalog .status{background:#633b82}</style></head><body>",
    );
    html.push_str("<header><h1>FFOne XDT NPC/HNPC render audit</h1><div class=\"stats\">");
    html.push_str(&format!(
        "XDT rows: {} · eligible: {} · runtime models: {} · appearances: {} · rendered: {}",
        counts.get("xdtRows").and_then(Value::as_u64).unwrap_or(0),
        counts
            .get("eligibleRows")
            .and_then(Value::as_u64)
            .unwrap_or(0),
        counts
            .get("uniqueRuntimeModels")
            .and_then(Value::as_u64)
            .unwrap_or(0),
        counts
            .get("uniqueRuntimeAppearances")
            .and_then(Value::as_u64)
            .unwrap_or(0),
        counts
            .get("renderedAppearances")
            .and_then(Value::as_u64)
            .unwrap_or(0),
    ));
    html.push_str("</div><div class=\"filters\"><input id=\"q\" placeholder=\"name, id, model\"><select id=\"role\"><option value=\"\">all roles</option><option>npc</option><option>hnpc</option><option>mob</option></select><select id=\"status\"><option value=\"\">all statuses</option>");
    for status in records
        .iter()
        .map(|record| record.status.as_str())
        .collect::<BTreeSet<_>>()
    {
        html.push_str(&format!("<option>{}</option>", escape_html(status)));
    }
    html.push_str("</select></div></header><main id=\"grid\">");
    for record in records {
        let search = format!(
            "{} {} {} {}",
            record.npc_name, record.npc_number, record.model_stem, record.legacy_route
        )
        .to_lowercase();
        html.push_str(&format!(
            "<article class=\"card {status}\" data-role=\"{role}\" data-status=\"{status}\" data-search=\"{search}\"><img loading=\"lazy\" src=\"{image}\" alt=\"{name}\"><div class=\"meta\"><div class=\"name\">#{number} {name}</div><div class=\"status\">{status}</div><div class=\"route\">{route}</div><div class=\"details\">row {row} · class {class} · mesh {mesh} · scale {scale:.4} · team {team}<br>main: {texture}<br>sub: {texture2}</div></div></article>",
            status = escape_html(&record.status),
            role = escape_html(&record.role),
            search = escape_html(&search),
            image = escape_html(&record.image),
            name = escape_html(&record.npc_name),
            number = record.npc_number,
            route = escape_html(&record.legacy_route),
            row = record.row_index,
            class = record.npc_class,
            mesh = record.mesh_index,
            scale = record.table_scale,
            team = record.team,
            texture = escape_html(
                record
                    .texture_path
                    .as_deref()
                    .or(record.texture.as_deref())
                    .unwrap_or("—")
            ),
            texture2 = escape_html(
                record
                    .texture2_path
                    .as_deref()
                    .or(record.texture2.as_deref())
                    .unwrap_or("—")
            ),
        ));
    }
    html.push_str("</main><script>const q=document.querySelector('#q'),r=document.querySelector('#role'),s=document.querySelector('#status'),cards=[...document.querySelectorAll('.card')];function f(){const t=q.value.toLowerCase();for(const c of cards)c.hidden=!!((t&&!c.dataset.search.includes(t))||(r.value&&c.dataset.role!==r.value)||(s.value&&c.dataset.status!==s.value))}q.oninput=r.onchange=s.onchange=f;</script></body></html>");
    fs::write(path, html).map_err(|error| format!("cannot write {}: {error}", path.display()))
}

pub(super) fn link_or_copy(source: &Path, destination: &Path, resume: bool) -> Result<(), String> {
    if destination.exists() {
        if resume {
            fs::remove_file(destination).map_err(|error| {
                format!(
                    "cannot replace resumed entity image {}: {error}",
                    destination.display()
                )
            })?;
        } else {
            return Err(format!(
                "entity image already exists: {}",
                destination.display()
            ));
        }
    }
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("cannot create {}: {error}", parent.display()))?;
    }
    if fs::hard_link(source, destination).is_err() {
        fs::copy(source, destination).map_err(|error| {
            format!(
                "cannot link or copy {} to {}: {error}",
                source.display(),
                destination.display()
            )
        })?;
    }
    Ok(())
}

pub(super) fn write_json(path: &Path, value: &impl Serialize) -> Result<(), String> {
    let bytes = serde_json::to_vec_pretty(value)
        .map_err(|error| format!("cannot serialize {}: {error}", path.display()))?;
    fs::write(path, bytes).map_err(|error| format!("cannot write {}: {error}", path.display()))
}
