use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReplyType {
    Accept,
    AskForEvidence,
    AskForScope,
    #[serde(alias = "move")]
    MoveToAdr,
    MoveToRfc,
    NeedsHumanJudgment,
    #[serde(alias = "decline")]
    CannotClassify,
}
