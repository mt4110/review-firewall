use std::path::{Path, PathBuf};

use rf_core::build_report_markdown;
use rf_core::domain::{DraftReplyArtifact, GateArtifact, ScanArtifact, Status};
use serde::de::DeserializeOwned;

use crate::adapter::git;
use crate::command::CommandOutcome;
use crate::io::{artifacts, run_store};

pub fn run(cwd: &Path) -> Result<CommandOutcome, String> {
    let repo_root = git::repo_root(cwd);
    let run = run_store::latest_or_create(&repo_root.path).map_err(io_error)?;

    let scan = read_json_artifact::<ScanArtifact>(
        run.directory.join("scan.json"),
        "scan.json",
        Status::Error,
        "scan.json not found; run review-firewall scan first",
    );
    let gate = read_json_artifact::<GateArtifact>(
        run.directory.join("gate.json"),
        "gate.json",
        Status::Partial,
        "gate.json not found; run review-firewall gate first",
    );
    let draft = read_json_artifact::<DraftReplyArtifact>(
        run.directory.join("draft_reply.json"),
        "draft_reply.json",
        Status::Partial,
        "draft_reply.json not found; run review-firewall draft-reply first",
    );
    let escalation = read_text_artifact(
        run.directory.join("escalation.md"),
        "escalation.md",
        Status::Partial,
        "escalation.md not found; run review-firewall escalate first",
    );
    let input_problems = [
        scan.problem.clone(),
        gate.problem.clone(),
        draft.problem.clone(),
        escalation.problem.clone(),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>();

    let (status, reason) = report_status_and_reason(
        scan.value.as_ref(),
        gate.value.as_ref(),
        draft.value.as_ref(),
        escalation.value.as_deref(),
        &input_problems,
    );

    let markdown = build_report_markdown(
        status,
        reason.as_deref(),
        scan.value.as_ref(),
        gate.value.as_ref(),
        draft.value.as_ref(),
        escalation.value.as_deref(),
    );
    artifacts::write_text(run.directory.join("report.md"), &markdown).map_err(io_error)?;

    let action_count = count_author_actions(&markdown);

    Ok(CommandOutcome {
        status,
        reason,
        lines: vec![
            format!(
                "Residual blockers: {}",
                gate.value
                    .as_ref()
                    .map(|artifact| artifact.residual_blockers.len())
                    .unwrap_or(0)
            ),
            String::from("PM summary ready: yes"),
            format!("Author actions: {action_count}"),
        ],
        next: None,
    })
}

#[derive(Debug, Clone)]
struct ArtifactProblem {
    status: Status,
    reason: String,
}

struct ArtifactRead<T> {
    value: Option<T>,
    problem: Option<ArtifactProblem>,
}

fn read_json_artifact<T: DeserializeOwned>(
    path: PathBuf,
    label: &str,
    missing_status: Status,
    missing_reason: &str,
) -> ArtifactRead<T> {
    match artifacts::read_json::<T>(path) {
        Ok(value @ Some(_)) => ArtifactRead {
            value,
            problem: None,
        },
        Ok(None) => ArtifactRead {
            value: None,
            problem: Some(ArtifactProblem {
                status: missing_status,
                reason: missing_reason.to_owned(),
            }),
        },
        Err(error) => ArtifactRead {
            value: None,
            problem: Some(ArtifactProblem {
                status: Status::Error,
                reason: format!("{label} could not be read: {error}"),
            }),
        },
    }
}

fn read_text_artifact(
    path: PathBuf,
    label: &str,
    missing_status: Status,
    missing_reason: &str,
) -> ArtifactRead<String> {
    match artifacts::read_text(path) {
        Ok(value @ Some(_)) => ArtifactRead {
            value,
            problem: None,
        },
        Ok(None) => ArtifactRead {
            value: None,
            problem: Some(ArtifactProblem {
                status: missing_status,
                reason: missing_reason.to_owned(),
            }),
        },
        Err(error) => ArtifactRead {
            value: None,
            problem: Some(ArtifactProblem {
                status: Status::Error,
                reason: format!("{label} could not be read: {error}"),
            }),
        },
    }
}

fn count_author_actions(markdown: &str) -> usize {
    markdown
        .lines()
        .filter(|line| is_numbered_action(line.trim()))
        .count()
}

fn is_numbered_action(line: &str) -> bool {
    let Some((number, rest)) = line.split_once('.') else {
        return false;
    };
    !number.is_empty() && number.as_bytes().iter().all(u8::is_ascii_digit) && rest.starts_with(' ')
}

#[cfg(test)]
#[allow(dead_code)]
pub fn count_author_actions_for_tests(markdown: &str) -> usize {
    count_author_actions(markdown)
}

fn report_status_and_reason(
    scan: Option<&ScanArtifact>,
    gate: Option<&GateArtifact>,
    draft: Option<&DraftReplyArtifact>,
    escalation: Option<&str>,
    input_problems: &[ArtifactProblem],
) -> (Status, Option<String>) {
    let escalation_status = escalation
        .and_then(markdown_status)
        .unwrap_or(Status::Partial);
    let escalation_reason = escalation.and_then(markdown_reason).or_else(|| {
        if escalation.is_some() && escalation.and_then(markdown_status).is_none() {
            Some(String::from("escalation.md is missing STATUS"))
        } else {
            None
        }
    });
    let mut status = scan
        .map(|artifact| artifact.status)
        .unwrap_or(Status::Error);
    if let Some(gate) = gate {
        status = status.merge(gate.status);
    }
    if let Some(draft) = draft {
        status = status.merge(draft.status);
    }
    if escalation.is_some() {
        status = status.merge(escalation_status);
    }
    for problem in input_problems {
        status = status.merge(problem.status);
    }

    let reason = gate
        .and_then(|artifact| artifact.reason.clone())
        .or_else(|| draft.and_then(|artifact| artifact.reason.clone()))
        .or_else(|| escalation_reason.clone())
        .or_else(|| scan.and_then(|artifact| artifact.reason.clone()))
        .or_else(|| input_problems.first().map(|problem| problem.reason.clone()));

    (status, reason)
}

#[cfg(test)]
#[allow(dead_code)]
pub fn report_status_and_reason_for_tests(
    scan: Option<&ScanArtifact>,
    gate: Option<&GateArtifact>,
    draft: Option<&DraftReplyArtifact>,
    escalation: Option<&str>,
) -> (Status, Option<String>) {
    report_status_and_reason(scan, gate, draft, escalation, &[])
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
