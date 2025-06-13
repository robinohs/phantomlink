use std::time::Duration;

use crate::{cli::opt::StartArgs, commands::utils, phork::namespace::NS_NAME_LINK, runtime::Runtime};

pub(crate) fn run(start_args: StartArgs) -> eyre::Result<()> {
    // check if the user is root
    utils::ensure_user_is_root()?;
    // check if network environment is set up
    utils::require_network_environment()?;
    // switch to the desired namespace
    utils::switch_network_environment(NS_NAME_LINK)?;

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
    rt.run().unwrap();
    Ok(())
}
