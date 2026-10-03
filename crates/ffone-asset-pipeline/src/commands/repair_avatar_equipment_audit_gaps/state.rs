use super::*;

pub(super) fn refresh_runtime_coverage(
    avatar: &CharacterCreationAvatarItems,
    runtime: &mut CharacterCreationRuntimeTextures,
) -> Result<(), String> {
    let published_paths = runtime
        .textures
        .iter()
        .map(|texture| texture.native_asset.path.clone())
        .collect::<BTreeSet<_>>();
    let mut references = 0_u64;
    let mut verified_paths = BTreeSet::new();
    let mut used_published_paths = BTreeSet::new();
    let mut missing = BTreeSet::new();
    let mut ambiguous = BTreeSet::new();
    let mut missing_source = BTreeSet::new();
    for item in &avatar.items {
        for visual in [&item.male, &item.female] {
            for reference in [&visual.primary_texture, &visual.secondary_texture]
                .into_iter()
                .flatten()
            {
                references += 1;
                match reference.status {
                    NativeLookupStatus::VerifiedUnique if reference.candidates.len() == 1 => {
                        let path = reference.candidates[0].path.clone();
                        verified_paths.insert(path.clone());
                        if published_paths.contains(&path) {
                            used_published_paths.insert(path);
                        } else {
                            missing_source.insert(reference.true_name.clone());
                        }
                    }
                    NativeLookupStatus::Missing => {
                        missing.insert(reference.true_name.clone());
                    }
                    _ => {
                        ambiguous.insert(reference.true_name.clone());
                    }
                }
            }
        }
    }
    runtime.coverage.avatar_texture_references = references;
    runtime.coverage.avatar_verified_unique_routes = verified_paths.len() as u64;
    runtime.coverage.avatar_published_routes = used_published_paths.len() as u64;
    runtime.coverage.avatar_deferred_verified_routes = verified_paths
        .len()
        .saturating_sub(used_published_paths.len())
        as u64;
    runtime.coverage.avatar_missing_true_names = missing.into_iter().collect();
    runtime.coverage.avatar_ambiguous_true_names = ambiguous.into_iter().collect();
    runtime.coverage.avatar_missing_source_metadata = missing_source.into_iter().collect();
    Ok(())
}
