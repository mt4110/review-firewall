use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Status {
    #[default]
    Ok,
    Partial,
    Error,
}

impl Status {
    pub const fn merge(self, other: Self) -> Self {
        match (self, other) {
            (Self::Error, _) | (_, Self::Error) => Self::Error,
            (Self::Partial, _) | (_, Self::Partial) => Self::Partial,
            _ => Self::Ok,
        }
    }

    pub const fn terminal_label(self) -> &'static str {
        match self {
            Self::Ok => "OK",
            Self::Partial => "PARTIAL",
            Self::Error => "ERROR",
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DataCoverage {
    #[default]
    Full,
    Partial,
    Failed,
}

impl DataCoverage {
    pub const fn terminal_label(self) -> &'static str {
        match self {
            Self::Full => "FULL",
            Self::Partial => "PARTIAL",
            Self::Failed => "FAILED",
        }
    }

    pub const fn from_partial_sources(has_partial_sources: bool) -> Self {
        if has_partial_sources {
            Self::Partial
        } else {
            Self::Full
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ReviewSignal {
    Blocked,
    Clear,
    #[default]
    Unknown,
}

impl ReviewSignal {
    pub const fn terminal_label(self) -> &'static str {
        match self {
            Self::Blocked => "BLOCKED",
            Self::Clear => "CLEAR",
            Self::Unknown => "UNKNOWN",
        }
    }
}

pub const fn review_signal_for(coverage: DataCoverage, residual_blockers: usize) -> ReviewSignal {
    match coverage {
        DataCoverage::Full => {
            if residual_blockers > 0 {
                ReviewSignal::Blocked
            } else {
                ReviewSignal::Clear
            }
        }
        DataCoverage::Partial | DataCoverage::Failed => ReviewSignal::Unknown,
    }
}
