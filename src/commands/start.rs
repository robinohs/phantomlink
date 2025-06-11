use std::time::Duration;

use log::info;

use crate::{
    cli::opt::StartArgs,
    phork::namespace::{self, Namespace, NS_NAME_LINK},
    runtime::Runtime,
};

pub(crate) fn run(start_args: StartArgs) -> eyre::Result<()> {
    // check if network environment is set up
    if !crate::phork::namespace::is_setup()? {
        info!("Network environment is not set up. Setting up...");
        namespace::setup().map_err(|e| eyre::eyre!("Failed to set up network environment: {}", e))?;
    }

    // switch to the link namespace
    info!("Switching to namespace: '{}'", NS_NAME_LINK);
    Namespace::try_load(NS_NAME_LINK)
        .map_err(|e| eyre::eyre!("Failed to load namespace '{}': {}", NS_NAME_LINK, e))?
        .try_switch_calling_pid_to_namespace()
        .map_err(|e| eyre::eyre!("Failed to switch to namespace '{}': {}", NS_NAME_LINK, e))?;

    let micros_reconfiguration_delay = (start_args.reconfiguration_delay * 1000.0) as u64;
    let reconfiguration_delay = Duration::from_micros(micros_reconfiguration_delay);

    // start command
    let mut rt = Runtime::new(
        start_args.input.as_std_path(),
        start_args.startup_mode,
        start_args.bottleneck_buffer_multiplier,
        reconfiguration_delay,
        start_args.reconfiguration_mode,
    )?;
    info!("Running virtual link until receiving Ctrl-C...");
    rt.run().unwrap();
    Ok(())
}
