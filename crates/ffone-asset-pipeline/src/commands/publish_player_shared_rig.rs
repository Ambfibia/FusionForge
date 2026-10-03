use std::{path::PathBuf, process::ExitCode};

use crate::{PlayerSharedRigPublishOptions, publish_player_shared_rigs};

pub(super) fn run_command(command_args: &[String]) -> ExitCode {
    let args = command_args.iter().map(std::ffi::OsString::from).collect::<Vec<_>>();
    if args.len() != 2 {
        eprintln!("usage: publish_player_shared_rig <EXTRACTED_OBJECTS_JSON> <ASSETS_GAME_ROOT>");
        return ExitCode::FAILURE;
    }
    let options =
        PlayerSharedRigPublishOptions::new(PathBuf::from(&args[0]), PathBuf::from(&args[1]));
    match publish_player_shared_rigs(&options) {
        Ok(report) => {
            println!(
                "published male={} bones, female={} bones, clips={}, creatorParts={}, creatorChoices={}, skinPalettes={}, creatorPreviewReady={}, contract={}",
                report.male_actor_bones,
                report.female_actor_bones,
                report.published_animation_clips,
                report.verified_creator_parts,
                report.verified_creator_choices,
                report.verified_skin_palettes,
                report.creator_preview_ready,
                report.contract_path,
            );
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("publish_player_shared_rig: {error}");
            ExitCode::FAILURE
        }
    }
}
