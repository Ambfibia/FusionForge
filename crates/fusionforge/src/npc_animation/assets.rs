use super::*;

#[derive(Debug, Clone, Default)]
pub(super) struct JointLookup {
    pub(super) by_path: HashMap<String, usize>,
    pub(super) unique_by_name: HashMap<String, usize>,
}

impl JointLookup {
    pub(super) fn new(joints: &[Joint]) -> Self {
        let by_path = joints
            .iter()
            .enumerate()
            .map(|(index, joint)| (joint.path.to_ascii_lowercase(), index))
            .collect::<HashMap<_, _>>();
        let mut by_terminal = HashMap::<String, Vec<usize>>::new();
        for (index, joint) in joints.iter().enumerate() {
            let name = joint
                .path
                .rsplit('/')
                .next()
                .unwrap_or(&joint.path)
                .to_ascii_lowercase();
            by_terminal.entry(name).or_default().push(index);
        }
        let unique_by_name = by_terminal
            .into_iter()
            .filter_map(|(name, indices)| (indices.len() == 1).then_some((name, indices[0])))
            .collect();
        Self {
            by_path,
            unique_by_name,
        }
    }

    pub(super) fn resolve(&self, value: &str) -> Option<usize> {
        let normalized = normalized_path(value).to_ascii_lowercase();
        self.by_path.get(&normalized).copied().or_else(|| {
            normalized
                .rsplit('/')
                .next()
                .and_then(|name| self.unique_by_name.get(name).copied())
        })
    }
}

pub(super) fn normalized_path(value: &str) -> String {
    value
        .trim()
        .replace('\\', "/")
        .split('/')
        .filter(|part| !part.trim().is_empty())
        .map(str::trim)
        .collect::<Vec<_>>()
        .join("/")
}

pub(super) fn parent_path(path: &str) -> Option<&str> {
    path.rsplit_once('/').map(|(parent, _)| parent)
}

pub(super) fn path_and_ancestors(path: &String) -> Vec<String> {
    let mut result = Vec::new();
    let mut current = path.as_str();
    loop {
        result.push(current.to_string());
        let Some(parent) = parent_path(current) else {
            break;
        };
        current = parent;
    }
    result
}
