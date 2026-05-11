use serde::{Deserialize, Serialize};

use crate::domain::blocker::{DuplicateGroup, GateCounts, ResidualBlocker};
use crate::domain::comment::{ClassifiedComment, CommentRecord, ReviewThread};
use crate::domain::escalation::EscalationCandidate;
use crate::domain::reply::ReplyType;
use crate::domain::status::{DataCoverage, ReviewSignal, Status};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LatestPointer {
    pub timestamp: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PullRequestSummary {
    pub number: Option<u64>,
    pub title: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub body: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub author: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base_branch: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub head_branch: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub head_oid: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub labels: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub review_decisions: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProductBoundarySnapshot {
    pub category: String,
    pub consumes_existing_review_comments: bool,
    pub generates_ai_reviews: bool,
    pub posts_pr_comments: bool,
    pub uses_llm_for_core_judgment: bool,
}

impl Default for ProductBoundarySnapshot {
    fn default() -> Self {
        Self {
            category: String::from("post_review_triage_firewall"),
            consumes_existing_review_comments: true,
            generates_ai_reviews: false,
            posts_pr_comments: false,
            uses_llm_for_core_judgment: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScanArtifact {
    pub status: Status,
    #[serde(default)]
    pub data_coverage: DataCoverage,
    #[serde(default)]
    pub review_signal: ReviewSignal,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    pub scan_partial: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repo_root: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    pub pr: PullRequestSummary,
    pub files_changed: usize,
    pub review_comments: usize,
    pub threads: usize,
    pub codeowners_found: bool,
    pub policy_found: bool,
    #[serde(default)]
    pub product_boundary: ProductBoundarySnapshot,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub changed_files: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub comments: Vec<CommentRecord>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub issue_comments: Vec<CommentRecord>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub review_threads: Vec<ReviewThread>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub partial_sources: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GateConfigSnapshot {
    pub require_failure_mode: bool,
    pub require_concern: bool,
    pub require_evidence: bool,
    pub require_alternative: bool,
    pub max_pr_thread_roundtrips: usize,
    pub use_codeowners: bool,
}

impl Default for GateConfigSnapshot {
    fn default() -> Self {
        Self {
            require_failure_mode: true,
            require_concern: true,
            require_evidence: true,
            require_alternative: false,
            max_pr_thread_roundtrips: 2,
            use_codeowners: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GateArtifact {
    pub status: Status,
    #[serde(default)]
    pub data_coverage: DataCoverage,
    #[serde(default)]
    pub review_signal: ReviewSignal,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    pub comments_analyzed: usize,
    pub residual_blockers: Vec<ResidualBlocker>,
    pub counts: GateCounts,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub candidate_blockers: Vec<ResidualBlocker>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub downgraded_comments: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub duplicates_collapsed: Vec<DuplicateGroup>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
    pub config_snapshot: GateConfigSnapshot,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub classified_comments: Vec<ClassifiedComment>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub escalation_candidates: Vec<EscalationCandidate>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DraftReplyArtifact {
    pub status: Status,
    #[serde(default)]
    pub data_coverage: DataCoverage,
    #[serde(default)]
    pub review_signal: ReviewSignal,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    pub reply_type: ReplyType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_comment_id: Option<String>,
    pub body: String,
}
