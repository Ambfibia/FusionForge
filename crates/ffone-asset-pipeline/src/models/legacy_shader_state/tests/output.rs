use super::*;

#[test]
    fn resolves_hash_bound_additive_transparent_depth_write_program() {
        let script = ADDITIVE_TRANSPARENT_SCRIPT
            .replace(ADDITIVE_TRANSPARENT, ADDITIVE_TRANSPARENT_DEPTH_WRITE)
            .replace("Transparent+10", "Transparent")
            .replace("\tZWrite Off\n", "")
            .replace(
                "\tTags {\"Queue\"=\"Transparent\"}\n\n\tBlend",
                "\tTags {\"Queue\"=\"Transparent\"}\n\tBlend",
            )
            .replace("\t\n\n\tSubShader", "\t\n\t\n\tSubShader");
        assert_eq!(script.len(), 681);
        assert_eq!(
            format!("{:x}", Sha256::digest(script.as_bytes())),
            ADDITIVE_TRANSPARENT_DEPTH_WRITE_SHA256
        );
        let (queue, passes) =
            resolve_legacy_material(ADDITIVE_TRANSPARENT_DEPTH_WRITE, &script, None, &[], &[])
                .unwrap();
        assert_eq!(queue, 3_000);
        assert_eq!(passes.len(), 1);
        let pass = &passes[0];
        assert!(pass.blend.enabled);
        assert_eq!(pass.blend.source_color, MaterialBlendFactor::SourceAlpha);
        assert_eq!(pass.blend.destination_color, MaterialBlendFactor::One);
        assert_eq!(pass.cull, MaterialCullMode::Back);
        assert!(pass.z_write);
        assert_eq!(pass.color_mask, 0b0111);
    }
