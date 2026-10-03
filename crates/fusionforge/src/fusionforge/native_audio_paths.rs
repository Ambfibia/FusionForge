//! Checked filename migration for existing localized audio. No audio bytes are converted.
use super::native_publication::{digest, encode, read_json, relative, snapshot, write};
use serde::Deserialize;
use serde_json::json;
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Move {
    old_name: String,
    path: String,
    sha256: String,
}

pub(super) fn run(root: &Path, work: &Path, recipe: &Path, apply: bool) -> Result<(), String> {
    let rows: Vec<Move> = serde_json::from_value(read_json(recipe)?).map_err(|e| e.to_string())?;
    let mut pending = Vec::new();
    let mut seen = BTreeSet::new();
    let mut routes = Vec::new();
    let mut outputs = Vec::new();
    let mut expected = BTreeMap::new();
    for row in rows {
        if Path::new(&row.old_name).components().count() != 1 {
            return Err("old audio name must be a filename".into());
        }
        let destination = relative(root, &row.path)?;
        let parent = Path::new(&row.path)
            .parent()
            .ok_or("audio parent missing")?;
        let old = parent
            .join(&row.old_name)
            .to_string_lossy()
            .replace('\\', "/");
        let source = relative(root, &old)?;
        if !seen.insert(row.path.to_ascii_lowercase()) || !seen.insert(old.to_ascii_lowercase()) {
            return Err("overlapping audio renames".into());
        }
        let existing = if destination.exists() {
            &destination
        } else {
            &source
        };
        let bytes = fs::read(existing).map_err(|e| format!("{}: {e}", existing.display()))?;
        if digest(&bytes) != row.sha256 {
            return Err(format!("audio preimage differs: {}", existing.display()));
        }
        if destination.exists()
            && source.exists()
            && fs::read(&source).map_err(|e| e.to_string())? != bytes
        {
            return Err("distinct audio exists at old and new paths".into());
        }
        write(&relative(&work.join("native"), &row.path)?, &bytes)?;
        expected.insert(old.clone(), source.exists().then(|| row.sha256.clone()));
        expected.insert(
            row.path.clone(),
            destination.exists().then(|| row.sha256.clone()),
        );
        routes.extend([old.clone(), row.path.clone()]);
        outputs.push(json!({"path":row.path,"oldPath":old,"sha256":row.sha256,"bytes":bytes.len(),"alreadyPresent":destination.exists()}));
        if !destination.exists() {
            pending.push((source, destination, bytes));
        }
    }
    if snapshot(root, &routes)? != expected {
        return Err("audio paths changed during staging".into());
    }
    write(
        &work.join("recipe.json"),
        &fs::read(recipe).map_err(|e| e.to_string())?,
    )?;
    // Stage all selected bytes, then make only same-volume file renames.
    if apply {
        if snapshot(root, &routes)? != expected {
            return Err("audio paths changed during staging".into());
        }
        commit_moves(&pending, |from, to| {
            fs::rename(from, to).map_err(|e| e.to_string())
        })
        .map_err(|e| format!("{e}; staged bytes: {}", work.display()))?;
    }
    let report = json!({"applied":apply,"renames":pending.len(),"outputs":outputs});
    write(&work.join("publication.json"), &encode(&report)?)?;
    println!("{report}");
    Ok(())
}
fn commit_moves(
    rows: &[(PathBuf, PathBuf, Vec<u8>)],
    mut rename: impl FnMut(&Path, &Path) -> Result<(), String>,
) -> Result<(), String> {
    let mut done = Vec::new();
    for (index, (from, to, before)) in rows.iter().enumerate() {
        let result = (|| {
            if fs::read(from).map_err(|e| e.to_string())? != *before || to.exists() {
                return Err("audio path changed before rename".into());
            }
            fs::create_dir_all(to.parent().ok_or("audio destination parent")?)
                .map_err(|e| e.to_string())?;
            rename(from, to)
        })();
        if let Err(error) = result {
            let mut failures = Vec::new();
            for &previous in done.iter().rev() {
                let (from, to, _): &(PathBuf, PathBuf, Vec<u8>) = &rows[previous];
                if let Err(e) = fs::rename(to, from) {
                    failures.push(e.to_string());
                }
            }
            return Err(format!(
                "audio rename failed: {error}; rollback failures: {failures:?}"
            ));
        }
        done.push(index);
    }
    Ok(())
}
#[cfg(test)]
mod tests;
