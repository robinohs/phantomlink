use std::path::PathBuf;

use clap::{crate_version, value_parser, Arg, Command};

use crate::cli::{LogLevel, StartupMode};

pub const RUN: &str = "run";
pub const RUN_ARG_INPUT_FILE: &str = "input file";
pub const RUN_ARG_LEVEL: &str = "level";
pub const RUN_ARG_BUFFER_SIZE_MULTIPLIER: &str = "buffer-size-multiplier";
pub const RUN_ARG_STARTUP_MODE: &str = "startup-mode";
pub const SOCKETSTATS: &str = "socketstats";
pub const SS_ARG_OUTPUT_FILE: &str = "output file";

pub fn create_cli() -> Command {
    clap::Command::new("phantomlink")
        .version(crate_version!())
        .about("phantomlink - Virtual Link")
        .bin_name("phantomlink")
        .subcommand_required(true)
        .arg_required_else_help(true)
        .subcommand(
            Command::new(RUN)
                .about("Starts the virtual link.")
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
                .about("Starts the socket stats logger.")
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
}
