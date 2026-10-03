use super::*;
use crate::fusionforge::unity::{Asset, ObjectInfo};

#[test]
fn legacy_negative_file_id_is_available_when_the_pointer_resolves() {
    let mut asset = Asset::empty_with_metadata("legacy.asset", 7).expect("legacy asset");
    asset.objects.insert(
        91_877_802,
        ObjectInfo {
            path_id: 91_877_802,
            data_offset: 0,
            size: 0,
            type_id: 1,
            class_id: 1,
        },
    );
    let env = UnityEnvironment::from_assets(vec![asset]);
    let pointer = Pointer {
        source_asset: 0,
        file_id: -1_015_676_928,
        path_id: 91_877_802,
    };

    assert!(pointer_ref_loaded_for_preview(&env, &pointer));
}

#[test]
fn compiled_add_opcode_does_not_make_an_opaque_shader_additive() {
    let mode = alpha_mode_hint(
        "CharacterBody",
        "ToonShader ADD R0, R1, R2",
        false,
        1.0,
        None,
        None,
        None,
    );

    assert_eq!(mode, "opaque");
}

#[test]
fn encoded_blend_names_still_select_real_blend_modes() {
    assert_eq!(
        alpha_mode_hint(
            "Fusion Finn",
            "ToonShading_blendSrcalphaInvsrcalpha",
            false,
            1.0,
            None,
            None,
            None,
        ),
        "blend"
    );
    assert_eq!(
        alpha_mode_hint("Glow", "Effect_blendOneOne", false, 1.0, None, None, None,),
        "additive"
    );
}
