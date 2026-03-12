use camino::Utf8PathBuf;

use crate::{
    build,
    cli::{ReconfigurationMode, StartupMode},
    phork::namespace::{NS_NAME_CLIENT, NS_NAME_SERVER},
    Shells,
};
use clap::{Args, Parser, Subcommand, ValueEnum};

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
    #[command(about = "Execute a command in the virtual link environment")]
    Exec(ExecArgs),
    #[command(about = "Setup the network environment")]
    Setup,
    #[command(about = "Tear down the network environment")]
    Teardown,
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
        long = "buffer-size-multiplier",
        help = "Multiplier for the size of the bottleneck buffer with BDP as the base (1.0: size is equal to BDP)",
        default_value_t = 1.0,
        required = false
    )]
    pub bottleneck_buffer_multiplier: f64,
    #[arg(
        long = "reconfiguration-delay",
        help = "Milliseconds it takes to reconfigure routes (the Pacer is paused during that time)",
        default_value_t = 0.0,
        required = false
    )]
    pub reconfiguration_delay: f64,
    #[clap(
        value_enum,
        long = "reconfiguration-mode",
        help = "Set reconfiguration mode",
        default_value_t = ReconfigurationMode::GSL,
        required = false
    )]
    pub reconfiguration_mode: ReconfigurationMode,
    #[clap(
        value_enum,
        long = "startup-mode",
        help = "Defines the constraint that is checked to determine the point in time after which the runtime synchronizes and starts to follow the input file",
        default_value_t = StartupMode::FirstPacket,
        required = false
    )]
    pub startup_mode: StartupMode,
}

#[derive(ValueEnum, Debug, Clone)]
pub enum ExecNamespaces {
    Client,
    Server,
}

impl ExecNamespaces {
    pub fn as_str(&self) -> &str {
        match self {
            ExecNamespaces::Client => NS_NAME_CLIENT,
            ExecNamespaces::Server => NS_NAME_SERVER,
        }
    }
}

#[derive(Args, Debug)]
pub struct ExecArgs {
    #[clap(help = "The namespace in which the command is to be executed", value_enum, required = true)]
    pub namespace: ExecNamespaces,
    #[arg(
        required(true),
        help = "The command to be executed in the virtual link environment",
        trailing_var_arg(true)
    )]
    //we activate trailing_var_arg to ignore flags in the program we want to run internally
    pub command: Vec<String>,
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
