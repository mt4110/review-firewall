use serde::{Deserialize, Deserializer, Serialize};

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

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ScanArtifact {
    pub status: Status,
    pub data_coverage: DataCoverage,
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

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct ScanArtifactSerde {
    pub status: Status,
    #[serde(default)]
    pub data_coverage: Option<DataCoverage>,
    #[serde(default)]
    pub review_signal: Option<ReviewSignal>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(default)]
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

impl<'de> Deserialize<'de> for ScanArtifact {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = ScanArtifactSerde::deserialize(deserializer)?;
        let data_coverage = raw.data_coverage.unwrap_or_else(|| {
            legacy_scan_data_coverage(
                raw.status,
                raw.scan_partial,
                !raw.partial_sources.is_empty(),
            )
        });

        Ok(Self {
            status: raw.status,
            data_coverage,
            review_signal: raw.review_signal.unwrap_or(ReviewSignal::Unknown),
            reason: raw.reason,
            scan_partial: raw.scan_partial,
            repo_root: raw.repo_root,
            branch: raw.branch,
            pr: raw.pr,
            files_changed: raw.files_changed,
            review_comments: raw.review_comments,
            threads: raw.threads,
            codeowners_found: raw.codeowners_found,
            policy_found: raw.policy_found,
            product_boundary: raw.product_boundary,
            changed_files: raw.changed_files,
            comments: raw.comments,
            issue_comments: raw.issue_comments,
            review_threads: raw.review_threads,
            partial_sources: raw.partial_sources,
            warnings: raw.warnings,
        })
    }
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

const fn legacy_scan_data_coverage(
    status: Status,
    scan_partial: bool,
    has_partial_sources: bool,
) -> DataCoverage {
    match status {
        Status::Error => DataCoverage::Failed,
        Status::Partial => DataCoverage::Partial,
        Status::Ok => {
            if scan_partial || has_partial_sources {
                DataCoverage::Partial
            } else {
                DataCoverage::Full
            }
        }
    }
}
