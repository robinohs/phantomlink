use crate::{build, phork::namespace::Namespace};
use log::debug;
use nix::unistd::Uid;

/// Ensures that the network environment is set up.
pub(crate) fn require_network_environment() -> eyre::Result<()> {
    if !crate::phork::namespace::is_setup()? {
        eyre::bail!("Network environment is not set up. Please run '{} setup' to create the necessary network namespaces and virtual ethernet links.", build::PROJECT_NAME);
    }
    Ok(())
}

/// Switches the current process to the specified network namespace.
pub(crate) fn switch_network_environment(namespace: &str) -> eyre::Result<()> {
    debug!("Switching to namespace: '{}'", namespace);
    Namespace::try_load(namespace)
        .map_err(|e| eyre::eyre!("Failed to load namespace '{}': {}", namespace, e))?
        .try_switch_calling_pid_to_namespace()
        .map_err(|e| eyre::eyre!("Failed to switch to namespace '{}': {}", namespace, e))?;
    Ok(())
}

/// Ensures that the current user is root.
pub(crate) fn ensure_user_is_root() -> eyre::Result<()> {
    if !Uid::effective().is_root() {
        return Err(eyre::eyre!(
            "This command must be run as root. Please use 'sudo' or switch to the root user."
        ));
    }
    Ok(())
}
