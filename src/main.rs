use std::time::Duration;

use cli::create_cli;
use color_eyre::Result;
use eyre::OptionExt;
use log::info;
use runtime::Runtime;
use shadow_rs::shadow;

shadow!(build);

mod cli {
    pub mod command;
    pub use command::create_cli;
    mod log_level;
    pub use log_level::LogLevel;
    mod startup_mode;
    pub use startup_mode::StartupMode;
}
mod route_metrics {
    mod route_metric;
    mod route_metric_queue;
    pub use route_metric_queue::RouteMetricQueue;
}
mod inflight_queue;
mod link {
    pub mod deliverer;
    pub mod dispatcher;
    pub mod drainer;
    pub mod oneway_virtual_link;
    pub mod pacer;
}
mod byte_bounded_channel;
mod runtime;
mod socketstats;

fn main() -> Result<()> {
    color_eyre::install()?;

    let matches = create_cli().get_matches();

    // configure logger
    if let Some((_, sub_matches)) = matches.subcommand() {
        // Configure logger
        let log_level: stderrlog::LogLevelNum = (*sub_matches
            .get_one::<cli::LogLevel>(cli::command::RUN_ARG_LEVEL)
            .unwrap_or(&cli::LogLevel::Info))
        .into();
        stderrlog::new()
            .module(module_path!())
            .verbosity(log_level)
            .timestamp(stderrlog::Timestamp::Millisecond)
            .init()
            .unwrap();
    }

    // run subcommand
    match matches.subcommand() {
        Some((cli::command::RUN, sub_matches)) => {
            // parse CLI arguments
            let input_file_path = sub_matches
                .get_one::<std::path::PathBuf>(cli::command::RUN_ARG_INPUT_FILE)
                .ok_or_eyre("Could not get input file path from arguments")?;

            // get the startup mode (the condition to follow the input)
            let startup_mode = *sub_matches
                .get_one::<cli::StartupMode>(cli::command::RUN_ARG_STARTUP_MODE)
                .ok_or_eyre("Could not get startup_mode from arguments")?;

            // get BtlBfr size
            let buffer_size_multiplier = *sub_matches
                .get_one::<f64>(cli::command::RUN_ARG_BUFFER_SIZE_MULTIPLIER)
                .ok_or_eyre("Could not get buffer size multiplicator from arguments")?;

            // get reconfiguration delay
            let reconfiguration_delay = *sub_matches
                .get_one::<f64>(cli::command::RUN_ARG_RECONFIGURATION_DELAY)
                .ok_or_eyre("Could not get reconfiguration delay from arguments")?;
            let reconfiguration_delay = Duration::from_millis(reconfiguration_delay as u64);

            // start command
            let mut rt = Runtime::new(input_file_path, startup_mode, buffer_size_multiplier, reconfiguration_delay)?;
            info!("Running virtual link until receiving Ctrl-C...");
            rt.run().unwrap();
        }
        Some((cli::command::SOCKETSTATS, sub_matches)) => {
            // parse CLI arguments
            let output_file_path = sub_matches
                .get_one::<std::path::PathBuf>(cli::command::SS_ARG_OUTPUT_FILE)
                .ok_or_eyre("Could not get output file path from arguments")?;

            socketstats::socketstats(output_file_path).unwrap();
        }
        _ => unimplemented!("unknown command"),
    }
    info!("Exit all");
    Ok(())
}
