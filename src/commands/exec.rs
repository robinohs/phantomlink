use std::process::Command;

use log::info;

use crate::{
    cli::opt::ExecArgs,
    phork::namespace::{self, Namespace},
};

pub(crate) fn run(exec_args: ExecArgs) -> eyre::Result<()> {
    // check if network environment is set up
    if !crate::phork::namespace::is_setup()? {
        info!("Network environment is not set up. Setting up...");
        namespace::setup().map_err(|e| eyre::eyre!("Failed to set up network environment: {}", e))?;
    }

    // switch to the desired namespace
    let namespace = exec_args.namespace.as_str();
    info!("Switching to namespace: '{}'", namespace);
    Namespace::try_load(namespace)
        .map_err(|e| eyre::eyre!("Failed to load namespace '{}': {}", namespace, e))?
        .try_switch_calling_pid_to_namespace()
        .map_err(|e| eyre::eyre!("Failed to switch to namespace '{}': {}", namespace, e))?;

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
