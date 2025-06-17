use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};

use crossbeam::channel::unbounded;
use log::warn;

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

    // set up Ctrl-C handler for cleanup before exit
    let (tx, rx) = unbounded::<()>();
    let exiting = Arc::new(AtomicBool::new(false));
    ctrlc::set_handler(move || {
        if exiting.load(Ordering::Relaxed) {
            warn!("Ctrl-C received twice, exiting ungracefully. This may leave the network environment in an inconsistent state.");
            std::process::exit(1);
        } else {
            tx.send(()).unwrap_or_else(|_| {
                eprintln!("Failed to send shutdown signal.");
            });
            exiting.store(true, Ordering::Relaxed);
        }
    })
    .expect("Error setting Ctrl-C handler");

    // start command
    let mut rt = Runtime::new(
        start_args.input.as_std_path(),
        start_args.startup_mode,
        start_args.bottleneck_buffer_multiplier,
        reconfiguration_delay,
        start_args.reconfiguration_mode,
    )?;

    rt.run(rx).unwrap();
    Ok(())
}
