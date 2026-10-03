use std::{path::PathBuf, process::ExitCode};

use crate::{PlayerRigAnimationPublishOptions, publish_player_rig_animations};

pub(super) fn run_command(command_args: &[String]) -> ExitCode {
    let args = command_args.iter().cloned().collect::<Vec<_>>();
    if !(2..=3).contains(&args.len()) {
        eprintln!(
            "usage: publish_player_rig_animations <asset-root> <clip-source-root> [custom-clip-source-root]"
        );
        return ExitCode::FAILURE;
    }
    let mut options =
        PlayerRigAnimationPublishOptions::new(PathBuf::from(&args[0]), PathBuf::from(&args[1]));
    if let Some(custom_source_root) = args.get(2) {
        options = options.with_custom_clip_source_root(PathBuf::from(custom_source_root));
    }
    match publish_player_rig_animations(&options) {
        Ok(()) => {
            println!(
                "transactionally published player runtime clips into each native player skeleton GLB and refreshed the optional legacy asset manifest"
            );
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}
