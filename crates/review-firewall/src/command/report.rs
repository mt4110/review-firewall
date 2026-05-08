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

    let (status, reason) = report_status_and_reason(
        scan.as_ref(),
        gate.as_ref(),
        draft.as_ref(),
        escalation.as_deref(),
    );

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

fn report_status_and_reason(
    scan: Option<&ScanArtifact>,
    gate: Option<&GateArtifact>,
    draft: Option<&DraftReplyArtifact>,
    escalation: Option<&str>,
) -> (Status, Option<String>) {
    let Some(scan) = scan else {
        return (
            Status::Error,
            Some(String::from(
                "scan.json not found; run review-firewall scan first",
            )),
        );
    };

    if gate.is_none() || draft.is_none() || escalation.is_none() {
        return (
            scan.status.merge(Status::Partial),
            Some(String::from("One or more upstream artifacts are missing")),
        );
    }

    let escalation_status = escalation
        .and_then(markdown_status)
        .unwrap_or(Status::Partial);
    let escalation_reason = escalation.and_then(markdown_reason).or_else(|| {
        if escalation.and_then(markdown_status).is_none() {
            Some(String::from("escalation.md is missing STATUS"))
        } else {
            None
        }
    });
    let gate = gate.expect("checked gate");
    let draft = draft.expect("checked draft");
    let status = scan
        .status
        .merge(gate.status)
        .merge(draft.status)
        .merge(escalation_status);
    let reason = gate
        .reason
        .clone()
        .or_else(|| draft.reason.clone())
        .or_else(|| escalation_reason.clone())
        .or_else(|| scan.reason.clone());

    (status, reason)
}

fn markdown_status(markdown: &str) -> Option<Status> {
    markdown.lines().find_map(|line| match line.trim() {
        "STATUS: OK" => Some(Status::Ok),
        "STATUS: PARTIAL" => Some(Status::Partial),
        "STATUS: ERROR" => Some(Status::Error),
        _ => None,
    })
}

fn markdown_reason(markdown: &str) -> Option<String> {
    markdown
        .lines()
        .find_map(|line| line.trim().strip_prefix("REASON: "))
        .filter(|reason| !reason.is_empty())
        .map(ToOwned::to_owned)
}

fn io_error(error: std::io::Error) -> String {
    error.to_string()
}
