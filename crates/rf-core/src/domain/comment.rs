use serde::{Deserialize, Serialize};

use crate::domain::ownership::{AdvisoryWeight, OwnershipScope};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CommentType {
    Blocker,
    Question,
    Suggestion,
    Nit,
    Praise,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BlockerConcern {
    Correctness,
    Security,
    Performance,
    Operability,
    Api,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CommentSource {
    ReviewComment,
    IssueComment,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommentRecord {
    pub comment_id: String,
    pub thread_id: String,
    pub author: String,
    pub body: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    pub source: CommentSource,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reply_to_comment_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub original_line: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReviewThread {
    pub thread_id: String,
    pub root_comment_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    pub participants: Vec<String>,
    pub roundtrips: usize,
    pub comments: Vec<CommentRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClassifiedComment {
    #[serde(flatten)]
    pub comment: CommentRecord,
    pub comment_type: CommentType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub concern: Option<BlockerConcern>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failure_mode: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub evidence: Vec<String>,
    pub present_pr_impact: bool,
    pub owner_match: bool,
    pub ownership_scope: OwnershipScope,
    pub advisory_weight: AdvisoryWeight,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duplicate_of_comment_id: Option<String>,
}
