use std::path::PathBuf;

use crate::{
    build,
    cli::{LogLevel, StartupMode},
    Shells,
};
use clap::{value_parser, Arg, ArgAction, Command};

pub const RUN: &str = "run";
pub const RUN_ARG_INPUT_FILE: &str = "input file";
pub const RUN_ARG_LEVEL: &str = "level";
pub const RUN_ARG_BUFFER_SIZE_MULTIPLIER: &str = "buffer-size-multiplier";
pub const RUN_ARG_STARTUP_MODE: &str = "startup-mode";
pub const RUN_ARG_RECONFIGURATION_DELAY: &str = "reconfiguration-delay";
pub const RUN_ARG_RECONFIGURATION_MODE: &str = "reconfiguration-mode";
pub const SOCKETSTATS: &str = "socketstats";
pub const SS_ARG_OUTPUT_FILE: &str = "output file";
pub const GENERATOR: &str = "generate";
pub const GEN_ARG_SHELL: &str = "shell";

pub fn create_cli() -> Command {
    clap::Command::new("phantomlink")
        .version(build::CLAP_LONG_VERSION)
        .about("phantomlink - Virtual Link")
        .bin_name("phantomlink")
        .subcommand_required(true)
        .arg_required_else_help(true)
        .subcommand(
            Command::new(RUN)
                .about("Start the virtual link.")
                .arg(
                    Arg::new(RUN_ARG_INPUT_FILE)
                        .required(true)
                        .value_parser(clap::value_parser!(PathBuf)),
                )
                .arg(
                    Arg::new(RUN_ARG_LEVEL)
                        .short('l')
                        .long("level")
                        .required(false)
                        .default_value("info")
                        .value_parser(value_parser!(LogLevel)),
                )
                .arg(
                    Arg::new(RUN_ARG_BUFFER_SIZE_MULTIPLIER)
                        .long("bottleneck-buffer-multiplier")
                        .required(false)
                        .default_value("1.0")
                        .help("Multiplier for the size of the bottleneck buffer with BDP as the basis (1.0: size is equal to BDP).")
                        .value_parser(clap::value_parser!(f64)),
                )
                .arg(
                    Arg::new(RUN_ARG_RECONFIGURATION_DELAY)
                        .long("reconfiguration-delay")
                        .required(false)
                        .default_value("0.0")
                        .help("Milliseconds it takes to reconfigure routes (the Pacer is paused during that time).")
                        .value_parser(clap::value_parser!(f64)),
                )
                .arg(
                    Arg::new(RUN_ARG_RECONFIGURATION_MODE)
                        .long("reconfiguration-mode")
                        .required(false)
                        .default_value("gsl")
                        .help("Reconfiguration mode can be one of [gsl, all]. ('gsl' is default)")
                        .value_parser(clap::value_parser!(String)),
                )
                .arg(
                    Arg::new(RUN_ARG_STARTUP_MODE)
                        .long("startup-mode")
                        .required(false)
                        .default_value("first-packet")
                        .help("Defines the condition for how phantomlink determines the point in time after which the input is tracked.")
                        .value_parser(clap::value_parser!(StartupMode)),
                ),
        )
        .subcommand(
            Command::new(SOCKETSTATS)
                .about("Start the socket stats logger.")
                .arg(
                    Arg::new(RUN_ARG_LEVEL)
                        .short('l')
                        .long("level")
                        .required(false)
                        .default_value("info")
                        .value_parser(value_parser!(LogLevel)),
                )
                .arg(
                    Arg::new(SS_ARG_OUTPUT_FILE)
                        .required(true)
                        .value_parser(clap::value_parser!(PathBuf)),
                ),
        )
        .subcommand(
            Command::new(GENERATOR).about("Generate shell completions.").arg(
                Arg::new(GEN_ARG_SHELL)
                    .required(false)
                    .help("Specify a shell [Default: Loaded from the environment]")
                    .action(ArgAction::Set)
                    .value_parser(value_parser!(Shells)),
            ),
        )
}
