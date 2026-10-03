use super::*;

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EquipmentBundleWarmupTiming {
    pub bundle_path: String,
    pub requested_routes: u64,
    pub extract_directories: u64,
    pub unity_assets: u64,
    pub indexed_objects: u64,
    pub indexed_container_routes: u64,
    pub milliseconds: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EquipmentBundleProof {
    pub bundle_path: String,
    pub bundle_name: String,
    pub byte_length: u64,
    pub sha256: String,
    pub resolved_routes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EquipmentContainerOwner {
    pub bundle_path: String,
    pub bundle_name: String,
    pub asset_name: String,
    pub exact_container_route: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct BundleEntry {
    pub(super) path: String,
    pub(super) name: String,
    #[serde(default)]
    pub(super) assets: Vec<BundleAsset>,
}

pub(super) fn bundle_proofs(candidates: &[RouteCandidate]) -> Result<Vec<EquipmentBundleProof>, String> {
    let mut grouped = BTreeMap::<String, (String, usize)>::new();
    for candidate in candidates {
        let entry = grouped
            .entry(candidate.owner.bundle_path.clone())
            .or_insert_with(|| (candidate.owner.bundle_name.clone(), 0));
        entry.1 += 1;
    }
    let mut proofs = Vec::new();
    for (path, (name, count)) in grouped {
        let bytes =
            fs::read(&path).map_err(|err| format!("could not hash source bundle {path}: {err}"))?;
        proofs.push(EquipmentBundleProof {
            bundle_path: path,
            bundle_name: name,
            byte_length: u64_count(bytes.len())?,
            sha256: sha256_hex(&bytes),
            resolved_routes: u64_count(count)?,
        });
    }
    proofs.sort_by(|left, right| left.bundle_name.cmp(&right.bundle_name));
    Ok(proofs)
}
