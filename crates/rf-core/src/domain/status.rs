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
