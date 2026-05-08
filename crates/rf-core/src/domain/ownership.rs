use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OwnershipScope {
    Exact,
    Partial,
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AdvisoryWeight {
    High,
    Medium,
    Low,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OwnershipAdvisory {
    pub owner_match: bool,
    pub ownership_scope: OwnershipScope,
    pub advisory_weight: AdvisoryWeight,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodeownerRule {
    pub pattern: String,
    pub owners: Vec<String>,
}
