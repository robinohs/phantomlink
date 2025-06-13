use log::{error, info};

use crate::{commands::utils, phork};

pub(crate) fn run() -> eyre::Result<()> {
    // check if the user is root
    utils::ensure_user_is_root()?;

    // Check if already set up
    if phork::namespace::is_setup()? {
        error!("Network environment is already set up. Use `teardown` to reset.");
        return Ok(());
    }
    info!("Setting up network environment");
    phork::namespace::setup()
}
