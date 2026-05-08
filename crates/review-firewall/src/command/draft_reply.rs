use std::path::Path;

use rf_core::build_draft_reply;
use rf_core::domain::{DraftReplyArtifact, GateArtifact, ReplyType, Status};

use crate::adapter::git;
use crate::command::CommandOutcome;
use crate::io::{artifacts, config, run_store};

pub fn run(cwd: &Path) -> Result<CommandOutcome, String> {
    let repo_root = git::repo_root(cwd);
    let run = run_store::latest_or_create(&repo_root.path).map_err(io_error)?;
    let policy = config::load(&repo_root.path);
    let gate =
        artifacts::read_json::<GateArtifact>(run.directory.join("gate.json")).map_err(io_error)?;

    let draft = if let Some(gate) = gate {
        build_draft_reply(&gate, policy.reply.max_lines)
    } else {
        DraftReplyArtifact {
            status: Status::Error,
            reason: Some(String::from(
                "gate.json not found; run review-firewall gate first",
            )),
            reply_type: ReplyType::Decline,
            target_comment_id: None,
            body: String::from(
                "Thanks. I do not think this blocks merge for this PR.\nReason: the current gate output is missing.\nIf needed, I can track it separately.",
            ),
        }
    };

    artifacts::write_json(run.directory.join("draft_reply.json"), &draft).map_err(io_error)?;
    artifacts::write_text(run.directory.join("draft_reply.md"), &draft.body).map_err(io_error)?;

    Ok(CommandOutcome {
        status: draft.status,
        reason: draft.reason.clone(),
        lines: vec![
            format!("Reply type: {}", reply_label(&draft.reply_type)),
            format!(
                "Target comment: {}",
                draft
                    .target_comment_id
                    .clone()
                    .unwrap_or_else(|| String::from("none"))
            ),
        ],
        next: None,
    })
}

fn reply_label(reply_type: &ReplyType) -> &'static str {
    match reply_type {
        ReplyType::Accept => "accept",
        ReplyType::Decline => "decline",
        ReplyType::Move => "move",
    }
}

fn io_error(error: std::io::Error) -> String {
    error.to_string()
}
