use clap::Parser;
use color_eyre::Result;
use env_logger::Env;
use shadow_rs::shadow;

use crate::{
    cli::{opt::Opt, ReconfigurationMode},
    commands::{
        exec,
        generator::{self, Shells},
        setup, socketstats, start, teardown,
    },
};

shadow!(build);

mod cli {
    pub mod opt;
    mod reconfiguration_mode;
    pub use reconfiguration_mode::ReconfigurationMode;
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
mod phork {
    pub mod namespace;
    mod utils;
    mod veth;
}
mod runtime;
mod commands {
    pub mod exec;
    pub mod generator;
    pub mod setup;
    pub mod socketstats;
    pub mod start;
    pub mod teardown;
}

fn main() -> Result<()> {
    color_eyre::install()?;

    let opt = Opt::parse();

    env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();

    match opt.command {
        cli::opt::Commands::Start(start_args) => start::run(start_args)?,
        cli::opt::Commands::SocketStats(socketstats_args) => socketstats::run(socketstats_args)?,
        cli::opt::Commands::Generate(generate_args) => generator::run(generate_args)?,
        cli::opt::Commands::Setup => setup::run()?,
        cli::opt::Commands::Teardown => teardown::run()?,
        cli::opt::Commands::Exec(exec_args) => exec::run(exec_args)?,
    }

    Ok(())

    // run subcommand
    // match matches.subcommand() {
    //     Some((cli::command::RUN, sub_matches)) => {
    //         // parse CLI arguments
    //         let input_file_path = sub_matches
    //             .get_one::<std::path::PathBuf>(cli::command::RUN_ARG_INPUT_FILE)
    //             .ok_or_eyre("Could not get input file path from arguments")?;

    //         // get the startup mode (the condition to follow the input)
    //         let startup_mode = *sub_matches
    //             .get_one::<cli::StartupMode>(cli::command::RUN_ARG_STARTUP_MODE)
    //             .ok_or_eyre("Could not get startup_mode from arguments")?;

    //         // get BtlBfr size
    //         let buffer_size_multiplier = *sub_matches
    //             .get_one::<f64>(cli::command::RUN_ARG_BUFFER_SIZE_MULTIPLIER)
    //             .ok_or_eyre("Could not get buffer size multiplicator from arguments")?;

    //         // get reconfiguration delay
    //         let reconfiguration_delay = *sub_matches
    //             .get_one::<f64>(cli::command::RUN_ARG_RECONFIGURATION_DELAY)
    //             .ok_or_eyre("Could not get reconfiguration delay from arguments")?;
    //         let reconfiguration_delay = Duration::from_millis(reconfiguration_delay as u64);

    //         // get reconfiguration mode
    //         let reconfiguration_mode = *sub_matches
    //             .get_one::<cli::ReconfigurationMode>(cli::command::RUN_ARG_RECONFIGURATION_MODE)
    //             .ok_or_eyre("Could not get reconfiguration mode from arguments")?;

    // // start command
    // let mut rt = Runtime::new(
    //     input_file_path,
    //     startup_mode,
    //     buffer_size_multiplier,
    //     reconfiguration_delay,
    //     reconfiguration_mode,
    // )?;
    // info!("Running virtual link until receiving Ctrl-C...");
    // rt.run().unwrap();
    //     }
    //     Some((cli::command::SOCKETSTATS, sub_matches)) => {
    //         // parse CLI arguments
    //         let output_file_path = sub_matches
    //             .get_one::<std::path::PathBuf>(cli::command::SS_ARG_OUTPUT_FILE)
    //             .ok_or_eyre("Could not get output file path from arguments")?;

    //         socketstats::socketstats(output_file_path).unwrap();
    //     }
    //     Some((cli::command::GENERATOR, sub_matches)) => {
    //         // parse CLI arguments
    //         let shell = sub_matches.get_one::<Shells>(cli::command::GEN_ARG_SHELL).copied();

    //         match shell {
    //             Some(shell) => generator::print_completions(shell)?,
    //             None => generator::print_completions_env()?,
    //         }
    //     }
    //     _ => unimplemented!("unknown command"),
    // }
}
