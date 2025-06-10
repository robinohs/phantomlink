use std::time::Duration;

use log::info;

use crate::{cli::opt::StartArgs, runtime::Runtime};

pub(crate) fn run(start_args: StartArgs) -> eyre::Result<()> {
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
