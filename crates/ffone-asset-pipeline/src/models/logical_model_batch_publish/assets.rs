use super::*;

#[derive(Default)]
pub(super) struct PortableOutputRegistry {
    pub(super) files: BTreeMap<String, RegisteredPath>,
    pub(super) directories: BTreeMap<String, RegisteredPath>,
}

pub(super) struct RegisteredPath {
    pub(super) exact: String,
    pub(super) owner: String,
}

impl PortableOutputRegistry {
    pub(super) fn register(&mut self, path: &Path, owner: &str) -> Result<()> {
        let exact = relative_path_string(path)?;
        let components = exact.split('/').collect::<Vec<_>>();
        for component in &components {
            validate_windows_component(component, "output path")?;
            if has_generated_identity(component) {
                return batch_error(format!(
                    "generated/hash/PathID identity is forbidden in output path {exact:?}"
                ));
            }
        }
        let key = portable_path_key(path)?;
        if let Some(previous) = self.files.get(&key) {
            return output_collision(&exact, owner, previous);
        }
        if let Some(previous) = self.directories.get(&key) {
            return output_collision(&exact, owner, previous);
        }

        let mut parent = PathBuf::new();
        for component in &components[..components.len() - 1] {
            parent.push(component);
            let parent_exact = relative_path_string(&parent)?;
            let parent_key = portable_path_key(&parent)?;
            if let Some(previous) = self.files.get(&parent_key) {
                return output_collision(&parent_exact, owner, previous);
            }
            if let Some(previous) = self.directories.get(&parent_key) {
                if previous.exact != parent_exact {
                    return output_collision(&parent_exact, owner, previous);
                }
            } else {
                self.directories.insert(
                    parent_key,
                    RegisteredPath {
                        exact: parent_exact,
                        owner: owner.to_owned(),
                    },
                );
            }
        }
        self.files.insert(
            key,
            RegisteredPath {
                exact,
                owner: owner.to_owned(),
            },
        );
        Ok(())
    }
}

pub(super) fn portable_path_key(path: &Path) -> Result<String> {
    let mut key = Vec::new();
    for component in path.components() {
        match component {
            Component::Normal(value) => {
                let value = value
                    .to_str()
                    .ok_or_else(|| batch_error_value("output path is not valid Unicode"))?;
                key.push(portable_string_key(value));
            }
            _ => return batch_error("output path is not a clean relative path"),
        }
    }
    if key.is_empty() {
        return batch_error("output path is empty");
    }
    Ok(key.join("/"))
}

pub(super) fn relative_path_string(path: &Path) -> Result<String> {
    let mut components = Vec::new();
    for component in path.components() {
        match component {
            Component::Normal(value) => components.push(
                value
                    .to_str()
                    .ok_or_else(|| batch_error_value("path is not valid Unicode"))?,
            ),
            _ => return batch_error("path is not a clean relative path"),
        }
    }
    if components.is_empty() {
        return batch_error("relative path is empty");
    }
    Ok(components.join("/"))
}

pub(super) fn slash_path(path: &Path) -> String {
    path.components()
        .filter_map(|component| match component {
            Component::Normal(value) => value.to_str(),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("/")
}
