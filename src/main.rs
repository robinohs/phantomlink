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
    pub mod utils;
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
}
