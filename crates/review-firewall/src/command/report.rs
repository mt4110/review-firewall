use std::path::Path;

use rf_core::build_report_markdown;
use rf_core::domain::{DraftReplyArtifact, GateArtifact, ScanArtifact, Status};

use crate::adapter::git;
use crate::command::CommandOutcome;
use crate::io::{artifacts, run_store};

pub fn run(cwd: &Path) -> Result<CommandOutcome, String> {
    let repo_root = git::repo_root(cwd);
    let run = run_store::latest_or_create(&repo_root.path).map_err(io_error)?;

    let scan =
        artifacts::read_json::<ScanArtifact>(run.directory.join("scan.json")).map_err(io_error)?;
    let gate =
        artifacts::read_json::<GateArtifact>(run.directory.join("gate.json")).map_err(io_error)?;
    let draft = artifacts::read_json::<DraftReplyArtifact>(run.directory.join("draft_reply.json"))
        .map_err(io_error)?;
    let escalation = artifacts::read_text(run.directory.join("escalation.md")).map_err(io_error)?;

    let (status, reason) = if let Some(scan) = scan.as_ref() {
        if gate.is_none() || draft.is_none() || escalation.is_none() {
            (
                scan.status.merge(Status::Partial),
                Some(String::from("One or more upstream artifacts are missing")),
            )
        } else {
            (
                gate.as_ref()
                    .map(|artifact| artifact.status)
                    .unwrap_or(scan.status),
                gate.as_ref()
                    .and_then(|artifact| artifact.reason.clone())
                    .or_else(|| scan.reason.clone()),
            )
        }
    } else {
        (
            Status::Error,
            Some(String::from(
                "scan.json not found; run review-firewall scan first",
            )),
        )
    };

    let markdown = build_report_markdown(
        status,
        reason.as_deref(),
        scan.as_ref(),
        gate.as_ref(),
        draft.as_ref(),
        escalation.as_deref(),
    );
    artifacts::write_text(run.directory.join("report.md"), &markdown).map_err(io_error)?;

    let action_count = markdown
        .lines()
        .filter(|line| {
            line.starts_with("1.")
                || line.starts_with("2.")
                || line.starts_with("3.")
                || line.starts_with("4.")
        })
        .count();

    Ok(CommandOutcome {
        status,
        reason,
        lines: vec![
            format!(
                "Residual blockers: {}",
                gate.as_ref()
                    .map(|artifact| artifact.residual_blockers.len())
                    .unwrap_or(0)
            ),
            String::from("PM summary ready: yes"),
            format!("Author actions: {action_count}"),
        ],
        next: None,
    })
}

fn io_error(error: std::io::Error) -> String {
    error.to_string()
}
