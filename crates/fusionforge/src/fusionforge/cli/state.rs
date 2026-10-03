use super::*;

pub(super) fn cli_rewrite_selected_external_pointers_to_local(
    value: &mut UnityValue,
    env: &UnityEnvironment,
    primary_asset_index: usize,
    selected: &BTreeSet<(usize, i64)>,
) {
    match value {
        UnityValue::Array(items) => {
            for item in items {
                cli_rewrite_selected_external_pointers_to_local(
                    item,
                    env,
                    primary_asset_index,
                    selected,
                );
            }
        }
        UnityValue::Object(fields) => {
            for item in fields.values_mut() {
                cli_rewrite_selected_external_pointers_to_local(
                    item,
                    env,
                    primary_asset_index,
                    selected,
                );
            }
        }
        UnityValue::Pair(left, right) => {
            cli_rewrite_selected_external_pointers_to_local(
                left,
                env,
                primary_asset_index,
                selected,
            );
            cli_rewrite_selected_external_pointers_to_local(
                right,
                env,
                primary_asset_index,
                selected,
            );
        }
        UnityValue::Pointer(pointer) => {
            let Ok(target) = env.resolve_pointer(pointer) else {
                return;
            };
            if target.asset == primary_asset_index {
                return;
            }
            if selected.contains(&(target.asset, target.path_id)) {
                pointer.source_asset = primary_asset_index;
                pointer.file_id = 0;
                pointer.path_id = target.path_id;
            }
        }
        _ => {}
    }
}
