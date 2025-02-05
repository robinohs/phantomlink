use clap::{builder::PossibleValue, ValueEnum};
use stderrlog::LogLevelNum;

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum LogLevel {
    Trace,
    Debug,
    Info,
}

// Can also be derived with feature flag `derive`
impl ValueEnum for LogLevel {
    fn value_variants<'a>() -> &'a [Self] {
        &[LogLevel::Trace, LogLevel::Debug, LogLevel::Info]
    }

    fn to_possible_value<'a>(&self) -> Option<PossibleValue> {
        Some(match self {
            LogLevel::Trace => PossibleValue::new("trace").help("Log everything."),
            LogLevel::Debug => PossibleValue::new("debug").help("Log important information and data useful for debugging."),
            LogLevel::Info => PossibleValue::new("info").help("Log important information."),
        })
    }
}

impl std::fmt::Display for LogLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.to_possible_value().expect("no values are skipped").get_name().fmt(f)
    }
}

impl std::str::FromStr for LogLevel {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        for variant in Self::value_variants() {
            if variant.to_possible_value().unwrap().matches(s, false) {
                return Ok(*variant);
            }
        }
        Err(format!("invalid variant: {s}"))
    }
}

impl From<LogLevel> for LogLevelNum {
    fn from(val: LogLevel) -> Self {
        match val {
            LogLevel::Trace => Self::Trace,
            LogLevel::Debug => Self::Debug,
            LogLevel::Info => Self::Info,
        }
    }
}
