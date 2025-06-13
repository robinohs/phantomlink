use log::info;

use crate::{commands::utils, phork::namespace};

pub(crate) fn run() -> eyre::Result<()> {
    // check if the user is root
    utils::ensure_user_is_root()?;

    namespace::clean().map_err(|e| eyre::eyre!("Failed to clean up network namespaces: {}", e))?;
    info!("Network environment has been successfully torn down.");
    Ok(())
}
