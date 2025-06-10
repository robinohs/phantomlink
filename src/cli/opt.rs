use camino::Utf8PathBuf;

use crate::{
    build,
    cli::{ReconfigurationMode, StartupMode},
    Shells,
};
use clap::{arg, command, Args, Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(name = "phantomlink")]
#[command(author)]
#[command(version = build::CLAP_LONG_VERSION)]
#[command(about = "phantomlink - Virtual End-to-End Link")]
pub struct Opt {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    #[command(about = "Start the main virtual link runtime")]
    Start(StartArgs),
    #[command(about = "Start the socket stats logger")]
    SocketStats(SocketstatsArgs),
    #[command(about = "Generate shell completions")]
    Generate(GenerateArgs),
}

#[derive(Args, Debug)]
pub struct StartArgs {
    #[arg(help = "Path to the input file")]
    pub input: Utf8PathBuf,
    #[arg(
        help = "Multiplier for the size of the bottleneck buffer with BDP as the base (1.0: size is equal to BDP)",
        default_value_t = 1.0,
        required = false
    )]
    pub bottleneck_buffer_multiplier: f64,
    #[arg(
        help = "Milliseconds it takes to reconfigure routes (the Pacer is paused during that time)",
        default_value_t = 0.0,
        required = false
    )]
    pub reconfiguration_delay: f64,
    #[clap(
        value_enum,
        help = "Set reconfiguration mode",
        default_value_t = ReconfigurationMode::GSL,
        required = false
    )]
    pub reconfiguration_mode: ReconfigurationMode,
    #[clap(
        value_enum,
        help = "Defines the constraint that is checked to determine the point in time after which the runtime synchronizes and starts to follow the input file",
        default_value_t = StartupMode::FirstPacket,
        required = false
    )]
    pub startup_mode: StartupMode,
}

#[derive(Args, Debug)]
pub struct SocketstatsArgs {
    #[arg(help = "Specifies the output file")]
    pub output_file: Utf8PathBuf,
}

#[derive(Args, Debug)]
pub struct GenerateArgs {
    #[arg(help = "Specify a shell [Default: Loaded from the environment]")]
    pub shell: Option<Shells>,
}
