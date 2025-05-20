use std::{error::Error, fmt::Display, time::Duration};

use cli::create_cli;
use color_eyre::Result;
use eyre::OptionExt;
use generator::Shells;
use log::info;
use runtime::Runtime;
use shadow_rs::shadow;

#[derive(Debug)]
struct ParseError(String);
impl Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}
impl Error for ParseError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        None
    }

    fn description(&self) -> &str {
        "description() is deprecated; use Display"
    }

    fn cause(&self) -> Option<&dyn Error> {
        self.source()
    }
}

#[derive(Clone, Copy, Debug)]
enum ReconfigurationMode {
    #[allow(clippy::upper_case_acronyms)]
    GSL,
    All,
}

impl TryFrom<String> for ReconfigurationMode {
    type Error = ParseError;

    fn try_from(value: String) -> std::result::Result<Self, Self::Error> {
        match value.to_lowercase().as_str() {
            "gsl" => Ok(Self::GSL),
            "all" => Ok(Self::All),
            _ => Err(ParseError(format!("{} is not a valid reconfiguration mode.", value))),
        }
    }
}

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
    pub mod drainer;
    pub mod oneway_virtual_link;
    pub mod pacer;
}
mod byte_bounded_channel;
mod generator;
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

            // get reconfiguration mode
            let reconfiguration_mode = sub_matches
                .get_one::<String>(cli::command::RUN_ARG_RECONFIGURATION_MODE)
                .ok_or_eyre("Could not get reconfiguration mode from arguments")?
                .clone();
            let reconfiguration_mode = ReconfigurationMode::try_from(reconfiguration_mode)?;

            // start command
            let mut rt = Runtime::new(
                input_file_path,
                startup_mode,
                buffer_size_multiplier,
                reconfiguration_delay,
                reconfiguration_mode,
            )?;
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
        Some((cli::command::GENERATOR, sub_matches)) => {
            // parse CLI arguments
            let shell = sub_matches.get_one::<Shells>(cli::command::GEN_ARG_SHELL).copied();

            match shell {
                Some(shell) => generator::print_completions(shell)?,
                None => generator::print_completions_env()?,
            }
        }
        _ => unimplemented!("unknown command"),
    }
    Ok(())
}
