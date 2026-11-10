use super::super::{process, types::{Installation, Manager, Owner}};
use std::{path::Path, process::Command};

pub fn command(item: &Installation) -> Result<Command, String> {
    if item.manager == Manager::Npm {
        if item.executable.extension().is_some_and(|extension| extension == "js" || extension == "cjs") {
            let Some(Owner::Npm { node, .. }) = &item.owner else { return Err("The owning Node runtime is unavailable".into()); };
            let mut command = Command::new(node);
            command.arg(process::command_path(&item.executable));
            return Ok(command);
        }
        return Ok(Command::new(&item.executable));
    }
    if cfg!(windows) && item.executable.extension().is_some_and(|ext| ext == "cmd" || ext == "bat") {
        return Err("Unverified command shim; use the original installation manager".into());
    }
    Ok(Command::new(&item.launcher))
}

pub fn arguments(item: &Installation) -> Result<Vec<String>, String> {
    let command = command(item)?;
    Ok(std::iter::once(command.get_program()).chain(command.get_args())
        .map(|arg| process::command_path(Path::new(arg)).to_string_lossy().into_owned()).collect())
}
