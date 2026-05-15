pub mod classify;
pub mod dedupe;
pub mod domain;
pub mod draft_reply;
pub mod escalation;
pub mod normalize;
pub mod ownership;
pub mod report;

pub use classify::gate_scan;
pub use draft_reply::build_draft_reply;
pub use escalation::{build_escalation_markdown, evaluate_escalation_candidates};
pub use normalize::{
    build_conversation_threads, build_conversation_threads_for_author, build_review_threads,
    build_review_threads_for_author, normalize_path,
};
pub use ownership::build_ownership_advisory;
pub use report::{ReportHeader, ReportInputs, build_report_markdown};
