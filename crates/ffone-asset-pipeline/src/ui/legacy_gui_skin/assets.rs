use super::*;

pub(super) type ObjectIdentityIndex = BTreeMap<(String, i64), ObjectIdentityMatch>;

pub(super) fn referenced_asset_counts(contract: &LegacyGuiSkinContract) -> (usize, usize) {
    let mut textures = BTreeSet::new();
    let mut fonts = BTreeSet::new();
    for skin in &contract.skins {
        if skin.font.path_id != 0 {
            fonts.insert((
                skin.source_asset.clone(),
                skin.font.file_id,
                skin.font.path_id,
            ));
        }
        for style in skin
            .built_in_styles
            .values()
            .chain(skin.custom_styles.iter())
        {
            if style.font.path_id != 0 {
                fonts.insert((
                    skin.source_asset.clone(),
                    style.font.file_id,
                    style.font.path_id,
                ));
            }
            for state in style.states.values() {
                if state.background.path_id != 0 {
                    textures.insert((
                        skin.source_asset.clone(),
                        state.background.file_id,
                        state.background.path_id,
                    ));
                }
            }
        }
    }
    (textures.len(), fonts.len())
}

pub(super) fn normalize_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}
