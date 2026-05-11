use std::path::Path;

use rf_core::build_draft_reply;
use rf_core::domain::{
    DataCoverage, DraftReplyArtifact, GateArtifact, ReplyType, ReviewSignal, Status,
};

use crate::adapter::git;
use crate::command::CommandOutcome;
use crate::io::config::ReviewFirewallConfig;
use crate::io::{artifacts, config, run_store};

pub fn run(cwd: &Path) -> Result<CommandOutcome, String> {
    let repo_root = git::repo_root(cwd);
    let run = run_store::latest_or_create(&repo_root.path).map_err(io_error)?;
    let policy = config::load(&repo_root.path);
    let gate = match artifacts::read_json::<GateArtifact>(run.directory.join("gate.json")) {
        Ok(gate) => gate,
        Err(error) => {
            let mut draft =
                gate_error_draft_artifact(format!("gate.json could not be read: {error}"));
            merge_config_status(&mut draft.status, &mut draft.reason, &policy);
            write_draft_artifacts(&run, &draft)?;
            return Ok(command_outcome(&draft, 0));
        }
    };

    let (draft, residual_blockers) = if let Some(gate) = gate {
        let residual_blockers = gate.residual_blockers.len();
        let mut draft = build_draft_reply(&gate, policy.reply.max_lines);
        merge_config_status(&mut draft.status, &mut draft.reason, &policy);
        (draft, residual_blockers)
    } else {
        let mut draft =
            gate_error_draft_artifact("gate.json not found; run review-firewall gate first");
        merge_config_status(&mut draft.status, &mut draft.reason, &policy);
        (draft, 0)
    };

    write_draft_artifacts(&run, &draft)?;

    Ok(command_outcome(&draft, residual_blockers))
}

fn gate_error_draft_artifact(reason: impl Into<String>) -> DraftReplyArtifact {
    let reason = reason.into();
    DraftReplyArtifact {
        status: Status::Error,
        data_coverage: DataCoverage::Failed,
        review_signal: ReviewSignal::Unknown,
        reason: Some(reason.clone()),
        reply_type: ReplyType::CannotClassify,
        target_comment_id: None,
        body: format!(
            "I could not complete blocker analysis for this PR, so I cannot draft a safe review reply yet.\nReason: {reason}\nRun review-firewall gate and retry before posting a review response."
        ),
    }
}

fn write_draft_artifacts(
    run: &run_store::RunDirectory,
    draft: &DraftReplyArtifact,
) -> Result<(), String> {
    artifacts::write_json(run.directory.join("draft_reply.json"), draft).map_err(io_error)?;
    artifacts::write_text(run.directory.join("draft_reply.md"), &draft.body).map_err(io_error)
}

fn command_outcome(draft: &DraftReplyArtifact, residual_blockers: usize) -> CommandOutcome {
    CommandOutcome {
        status: draft.status,
        data_coverage: draft.data_coverage,
        review_signal: draft.review_signal,
        residual_blockers,
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
        next: if draft.reply_type == ReplyType::CannotClassify {
            Some(String::from(
                "Rerun review-firewall scan and gate before using this reply draft.",
            ))
        } else {
            None
        },
    }
}

fn merge_config_status(
    status: &mut Status,
    reason: &mut Option<String>,
    policy: &ReviewFirewallConfig,
) {
    if policy.status == Status::Ok {
        return;
    }
    *status = status.merge(policy.status);
    if reason.is_none() {
        *reason = policy
            .reason
            .clone()
            .or_else(|| Some(String::from("Config loaded with partial status")));
    }
}

fn reply_label(reply_type: &ReplyType) -> &'static str {
    match reply_type {
        ReplyType::Accept => "accept",
        ReplyType::AskForEvidence => "ask_for_evidence",
        ReplyType::AskForScope => "ask_for_scope",
        ReplyType::MoveToAdr => "move_to_adr",
        ReplyType::MoveToRfc => "move_to_rfc",
        ReplyType::NeedsHumanJudgment => "needs_human_judgment",
        ReplyType::CannotClassify => "cannot_classify",
    }
}

fn io_error(error: std::io::Error) -> String {
    error.to_string()
}
