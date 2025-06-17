use clap::{builder::PossibleValue, ValueEnum};

#[derive(Clone, Copy, Debug)]
pub enum ReconfigurationMode {
    #[allow(clippy::upper_case_acronyms)]
    GSL,
    All,
}

impl ValueEnum for ReconfigurationMode {
    fn value_variants<'a>() -> &'a [Self] {
        &[ReconfigurationMode::GSL, ReconfigurationMode::All]
    }

    fn to_possible_value<'a>(&self) -> Option<PossibleValue> {
        Some(match self {
            ReconfigurationMode::GSL => PossibleValue::new("gsl").help("Apply reconfiguration to GSLs only"),
            ReconfigurationMode::All => PossibleValue::new("all").help("Apply reconfiguration to all links"),
        })
    }
}

impl std::str::FromStr for ReconfigurationMode {
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
