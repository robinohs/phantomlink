use std::process::Command;

use crate::{cli::opt::ExecArgs, commands::utils};

pub(crate) fn run(exec_args: ExecArgs) -> eyre::Result<()> {
    // check if the user is root
    utils::ensure_user_is_root()?;
    // check if network environment is set up
    utils::require_network_environment()?;
    // switch to the desired namespace
    let namespace = exec_args.namespace.as_str();
    utils::switch_network_environment(namespace)?;

    // exec command
    let command = exec_args
        .command
        .first()
        .ok_or_else(|| eyre::eyre!("No command provided. Please specify a command to execute."))?;
    let args = exec_args.command.iter().skip(1).map(String::as_str).collect::<Vec<_>>();
    Command::new(command)
        .args(args)
        .spawn()
        .map_err(|e| eyre::eyre!("Failed to execute command '{}': {}", command, e))?
        .wait()
        .map_err(|e| eyre::eyre!("Command '{}' failed: {}", command, e))?;

    Ok(())
}
