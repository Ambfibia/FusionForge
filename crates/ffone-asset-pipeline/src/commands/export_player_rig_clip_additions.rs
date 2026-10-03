use std::{path::Path, process::ExitCode};

pub(super) fn run_command(command_args: &[String]) -> ExitCode {
    let args = command_args.iter().cloned().collect::<Vec<_>>();
    if args.len() != 3 {
        eprintln!("usage: export_player_rig_clip_additions <native-asset-root> <scoped-objects-dir> <staging-dir>");
        return ExitCode::FAILURE;
    }
    match crate::export_player_rig_clip_additions(
        Path::new(&args[0]), Path::new(&args[1]), Path::new(&args[2]),
    ) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => { eprintln!("{error}"); ExitCode::FAILURE }
    }
}
