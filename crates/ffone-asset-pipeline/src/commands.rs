mod export_player_rig_clip_additions;
mod install_alternate_ultimate_cannonbolt;
mod install_primary_missing_hat_variant;
mod install_table_set;
mod normalize_map_routes;
mod player_shared_skeleton_probe;
mod publish_exact_texture;
mod publish_exact_texture_mips;
mod publish_legacy_static_exact_mips_manifest;
mod publish_player_head_models;
mod publish_player_rig_animations;
mod publish_player_shared_rig;
mod publish_retrobution_hostile_xdt_textures;
mod render_xdt_npc_gallery;
mod repair_avatar_equipment_audit_gaps;
mod repair_normalized_object_catalog;
mod repair_verified_avatar_model_routes;

pub fn run(command: &str, args: &[String]) -> Result<String, String> {
    match command {
        "--help" | "-h" => Ok("Available fusionforge utility operations:\nexport-player-rig-clip-additions\ninstall-alternate-ultimate-cannonbolt\ninstall-primary-missing-hat-variant\ninstall-table-set\nnormalize-map-routes\nplayer-shared-skeleton-probe\npublish-exact-texture\npublish-exact-texture-mips\npublish-legacy-static-exact-mips-manifest\npublish-player-head-models\npublish-player-rig-animations\npublish-player-shared-rig\npublish-retrobution-hostile-xdt-textures\nrender-xdt-npc-gallery\nrepair-avatar-equipment-audit-gaps\nrepair-normalized-object-catalog\nrepair-verified-avatar-model-routes".into()),
        "export-player-rig-clip-additions" => {
            if export_player_rig_clip_additions::run_command(args)
                == std::process::ExitCode::SUCCESS
            {
                Ok(String::new())
            } else {
                Err("export-player-rig-clip-additions failed".into())
            }
        }
        "install-alternate-ultimate-cannonbolt" => {
            install_alternate_ultimate_cannonbolt::run(args).map(|value| format!("{value:?}"))
        }
        "install-primary-missing-hat-variant" => {
            install_primary_missing_hat_variant::run(args).map(|value| format!("{value:?}"))
        }
        "install-table-set" => install_table_set::run(args).map(|value| format!("{value:?}")),
        "normalize-map-routes" => normalize_map_routes::run(args).map(|value| format!("{value:?}")),
        "player-shared-skeleton-probe" => player_shared_skeleton_probe::run_command(args)
            .map(|_| String::new())
            .map_err(|e| e.to_string()),
        "publish-exact-texture" => {
            publish_exact_texture::run(args).map(|value| format!("{value:?}"))
        }
        "publish-exact-texture-mips" => {
            publish_exact_texture_mips::run(args).map(|value| format!("{value:?}"))
        }
        "publish-legacy-static-exact-mips-manifest" => {
            publish_legacy_static_exact_mips_manifest::run(args).map(|value| format!("{value:?}"))
        }
        "publish-player-head-models" => {
            if publish_player_head_models::run_command(args) == std::process::ExitCode::SUCCESS {
                Ok(String::new())
            } else {
                Err("publish-player-head-models failed".into())
            }
        }
        "publish-player-rig-animations" => {
            if publish_player_rig_animations::run_command(args) == std::process::ExitCode::SUCCESS {
                Ok(String::new())
            } else {
                Err("publish-player-rig-animations failed".into())
            }
        }
        "publish-player-shared-rig" => {
            if publish_player_shared_rig::run_command(args) == std::process::ExitCode::SUCCESS {
                Ok(String::new())
            } else {
                Err("publish-player-shared-rig failed".into())
            }
        }
        "publish-retrobution-hostile-xdt-textures" => {
            publish_retrobution_hostile_xdt_textures::run(args).map(|value| format!("{value:?}"))
        }
        "render-xdt-npc-gallery" => {
            render_xdt_npc_gallery::run(args.iter().map(std::ffi::OsString::from).collect())
        }
        "repair-avatar-equipment-audit-gaps" => {
            repair_avatar_equipment_audit_gaps::run(args).map(|value| format!("{value:?}"))
        }
        "repair-normalized-object-catalog" => {
            repair_normalized_object_catalog::run(args).map(|value| format!("{value:?}"))
        }
        "repair-verified-avatar-model-routes" => {
            repair_verified_avatar_model_routes::run(args).map(|value| format!("{value:?}"))
        }
        _ => Err(format!("unknown utility {command}")),
    }
}
