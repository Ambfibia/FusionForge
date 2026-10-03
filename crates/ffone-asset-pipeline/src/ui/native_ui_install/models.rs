use super::*;

pub(super) fn validate_archived_route_model(catalog: &ArchivedCatalog) -> Result<()> {
    let desired = desired_route_sources();
    let mut archived = BTreeSet::new();
    for asset in &catalog.assets {
        if !archived.insert(asset.path.as_str()) {
            return invalid(format!(
                "duplicate archived gameplay UI route {:?}",
                asset.path
            ));
        }
        let Some((source, _)) = desired.get(&asset.path) else {
            return invalid(format!(
                "archived gameplay UI route {:?} is no longer modeled",
                asset.path
            ));
        };
        if *source != asset.source_path {
            return invalid(format!(
                "archived source identity mismatch for {:?}",
                asset.path
            ));
        }
    }
    Ok(())
}
