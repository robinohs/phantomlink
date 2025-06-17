use std::{io, path::Path};

use clap::{builder::PossibleValue, Command, CommandFactory, ValueEnum};
use clap_complete::{generate, Generator, Shell};
use clap_complete_nushell::Nushell;
use eyre::{bail, eyre};

use crate::cli::opt::{GenerateArgs, Opt};

pub(crate) fn run(generate_args: GenerateArgs) -> eyre::Result<()> {
    match generate_args.shell {
        Some(shell) => print_completions(shell),
        None => print_completions_env(),
    }
}

fn print_completions(shell: Shells) -> eyre::Result<()> {
    let cmd = &mut Opt::command();
    generate(shell, cmd, cmd.get_name().to_string(), &mut io::stdout());
    Ok(())
}

fn print_completions_env() -> eyre::Result<()> {
    let cmd = &mut Opt::command();
    let generator = extract_generator()?;
    generate(generator, cmd, cmd.get_name().to_string(), &mut io::stdout());
    Ok(())
}

impl From<Shell> for Shells {
    fn from(val: Shell) -> Self {
        match val {
            Shell::Bash => Shells::Bash(Shell::Bash),
            Shell::Elvish => Shells::Elvish(Shell::Elvish),
            Shell::Fish => Shells::Fish(Shell::Fish),
            Shell::PowerShell => Shells::PowerShell(Shell::PowerShell),
            Shell::Zsh => Shells::Zsh(Shell::Zsh),
            _ => unimplemented!(),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum Shells {
    /// Bourne Again `SHell` (bash)
    Bash(Shell),
    /// Elvish shell
    Elvish(Shell),
    /// Friendly Interactive `SHell` (fish)
    Fish(Shell),
    /// `PowerShell`
    PowerShell(Shell),
    /// Z `SHell` (zsh)
    Zsh(Shell),
    /// New type of shell
    Nushell,
}

// Hand-rolled so it can work even when `derive` feature is disabled
impl ValueEnum for Shells {
    fn value_variants<'a>() -> &'a [Self] {
        &[
            Shells::Bash(Shell::Bash),
            Shells::Elvish(Shell::Elvish),
            Shells::Fish(Shell::Fish),
            Shells::PowerShell(Shell::PowerShell),
            Shells::Zsh(Shell::Zsh),
            Shells::Nushell,
        ]
    }

    fn to_possible_value(&self) -> Option<PossibleValue> {
        Some(match self {
            Shells::Bash(shell) => shell.to_possible_value()?,
            Shells::Elvish(shell) => shell.to_possible_value()?,
            Shells::Fish(shell) => shell.to_possible_value()?,
            Shells::PowerShell(shell) => shell.to_possible_value()?,
            Shells::Zsh(shell) => shell.to_possible_value()?,
            Shells::Nushell => PossibleValue::new("nushell"),
        })
    }
}

impl Generator for Shells {
    fn file_name(&self, name: &str) -> String {
        match self {
            Shells::Bash(shell) => shell.file_name(name),
            Shells::Elvish(shell) => shell.file_name(name),
            Shells::Fish(shell) => shell.file_name(name),
            Shells::PowerShell(shell) => shell.file_name(name),
            Shells::Zsh(shell) => shell.file_name(name),
            Shells::Nushell => Nushell.file_name(name),
        }
    }

    fn generate(&self, cmd: &Command, buf: &mut dyn io::Write) {
        match self {
            Shells::Bash(shell) => shell.generate(cmd, buf),
            Shells::Elvish(shell) => shell.generate(cmd, buf),
            Shells::Fish(shell) => shell.generate(cmd, buf),
            Shells::PowerShell(shell) => shell.generate(cmd, buf),
            Shells::Zsh(shell) => shell.generate(cmd, buf),
            Shells::Nushell => Nushell.generate(cmd, buf),
        }
    }
}

fn extract_generator() -> eyre::Result<Shells> {
    let path = match std::env::var_os("SHELL") {
        Some(path) => path,
        None => unimplemented!("Failed to load shell from environment"),
    };

    let path = Path::new(&path);

    match Shell::from_shell_path(path) {
        Some(shell) => Ok(shell.into()),
        None => match path
            .file_stem()
            .ok_or(eyre!("Could not read file stem"))?
            .to_str()
            .ok_or(eyre!("Could not convert name to a string"))?
        {
            "nu" => Ok(Shells::Nushell),
            name => bail!("Unsupported shell {}", name),
        },
    }
}
