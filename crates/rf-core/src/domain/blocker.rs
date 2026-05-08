use serde::{Deserialize, Serialize};

use crate::domain::comment::BlockerConcern;
use crate::domain::ownership::{AdvisoryWeight, OwnershipScope};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct GateCounts {
    pub questions: usize,
    pub suggestions: usize,
    pub nits: usize,
    pub praise: usize,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub unknown: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResidualBlocker {
    pub comment_id: String,
    pub concern: BlockerConcern,
    pub failure_mode: String,
    pub evidence: Vec<String>,
    pub owner_match: bool,
    pub ownership_scope: OwnershipScope,
    pub advisory_weight: AdvisoryWeight,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    pub author: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DuplicateGroup {
    pub primary_comment_id: String,
    pub duplicate_comment_ids: Vec<String>,
}

const fn is_zero(value: &usize) -> bool {
    *value == 0
}
