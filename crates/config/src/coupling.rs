use std::fmt;
use std::str::FromStr;

use crate::ChurnWindow;

pub const DEFAULT_MAX_COMMIT_FILES: usize = 50;
pub const DEFAULT_MIN_CO_CHANGES: usize = 3;
pub const DEFAULT_MIN_COUPLING_RATIO: f64 = 0.3;

#[derive(Debug, Clone, Copy)]
pub struct CouplingRatio(f64);

impl CouplingRatio {
    pub fn new(value: f64) -> Result<CouplingRatio, CouplingRatioError> {
        if value.is_finite() && (0.0..=1.0).contains(&value) {
            let normalized = if value == 0.0 { 0.0 } else { value };
            Ok(CouplingRatio(normalized))
        } else {
            Err(CouplingRatioError {
                text: value.to_string(),
            })
        }
    }

    pub fn value(self) -> f64 {
        self.0
    }
}

impl Default for CouplingRatio {
    fn default() -> CouplingRatio {
        CouplingRatio(DEFAULT_MIN_COUPLING_RATIO)
    }
}

impl PartialEq for CouplingRatio {
    fn eq(&self, other: &Self) -> bool {
        self.0.to_bits() == other.0.to_bits()
    }
}

impl Eq for CouplingRatio {}

impl fmt::Display for CouplingRatio {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}", self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{text:?} is not a coupling ratio between 0 and 1, such as 0.3")]
pub struct CouplingRatioError {
    text: String,
}

impl FromStr for CouplingRatio {
    type Err = CouplingRatioError;

    fn from_str(text: &str) -> Result<CouplingRatio, CouplingRatioError> {
        let value = text.parse::<f64>().map_err(|_| CouplingRatioError {
            text: text.to_owned(),
        })?;
        CouplingRatio::new(value).map_err(|_| CouplingRatioError {
            text: text.to_owned(),
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CouplingSettings {
    pub window: ChurnWindow,
    pub max_commit_files: usize,
    pub min_co_changes: usize,
    pub min_ratio: CouplingRatio,
    pub include_tests: bool,
}

impl Default for CouplingSettings {
    fn default() -> CouplingSettings {
        CouplingSettings {
            window: ChurnWindow::default(),
            max_commit_files: DEFAULT_MAX_COMMIT_FILES,
            min_co_changes: DEFAULT_MIN_CO_CHANGES,
            min_ratio: CouplingRatio::default(),
            include_tests: false,
        }
    }
}
