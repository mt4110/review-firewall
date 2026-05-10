use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EscalationLabel {
    StayInPr,
    MoveToAdr,
    MoveToRfc,
    NeedsHumanJudgment,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EscalationCandidate {
    pub thread_id: String,
    pub root_comment_id: String,
    pub label: EscalationLabel,
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    pub roundtrips: usize,
    pub participants: Vec<String>,
    pub position_a: String,
    pub position_b: String,
    pub why: String,
}
