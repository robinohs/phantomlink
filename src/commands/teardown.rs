use log::info;

use crate::phork::namespace;

pub(crate) fn run() -> eyre::Result<()> {
    namespace::clean().map_err(|e| eyre::eyre!("Failed to clean up network namespaces: {}", e))?;
    info!("Network environment has been successfully torn down.");
    Ok(())
}
