use rf_core::domain::{DataCoverage, ReviewSignal, Status};

pub mod draft_reply;
pub mod escalate;
pub mod gate;
pub mod report;
pub mod scan;

#[derive(Debug, Clone, Copy)]
pub enum CommandKind {
    Scan { pr: Option<u64> },
    Gate,
    DraftReply,
    Escalate,
    Report,
}

pub struct CommandOutcome {
    pub status: Status,
    pub data_coverage: DataCoverage,
    pub review_signal: ReviewSignal,
    pub residual_blockers: usize,
    pub reason: Option<String>,
    pub lines: Vec<String>,
    pub next: Option<String>,
}
