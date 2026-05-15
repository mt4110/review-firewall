use std::path::{Path, PathBuf};

use rf_core::domain::{
    DataCoverage, DraftReplyArtifact, GateArtifact, ReviewSignal, ScanArtifact,
    SourceCoverageArtifact, SourceCoverageStatus, Status,
};
use rf_core::{ReportHeader, ReportInputs, build_report_markdown};
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
    let source_coverage = read_json_artifact::<SourceCoverageArtifact>(
        run.directory.join("source_coverage.json"),
        "source_coverage.json",
        Status::Partial,
        "source_coverage.json not found; run review-firewall scan first",
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
        source_coverage.problem.clone(),
        gate.problem.clone(),
        draft.problem.clone(),
        escalation.problem.clone(),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>();

    let summary = report_summary(
        scan.value.as_ref(),
        source_coverage.value.as_ref(),
        gate.value.as_ref(),
        draft.value.as_ref(),
        escalation.value.as_deref(),
        &input_problems,
    );

    let markdown = build_report_markdown(
        ReportHeader {
            run_status: summary.status,
            data_coverage: summary.data_coverage,
            review_signal: summary.review_signal,
            residual_blockers: gate
                .value
                .as_ref()
                .map(|artifact| artifact.residual_blockers.len())
                .unwrap_or(0),
        },
        summary.reason.as_deref(),
        ReportInputs {
            scan: scan.value.as_ref(),
            source_coverage: source_coverage.value.as_ref(),
            source_coverage_notice: source_coverage
                .problem
                .as_ref()
                .map(|problem| problem.reason.as_str()),
            gate: gate.value.as_ref(),
            draft_reply: draft.value.as_ref(),
            escalation_markdown: escalation.value.as_deref(),
        },
    );
    artifacts::write_text(run.directory.join("report.md"), &markdown).map_err(io_error)?;

    let action_count = count_author_actions(&markdown);

    Ok(CommandOutcome {
        status: summary.status,
        data_coverage: summary.data_coverage,
        review_signal: summary.review_signal,
        residual_blockers: gate
            .value
            .as_ref()
            .map(|artifact| artifact.residual_blockers.len())
            .unwrap_or(0),
        reason: summary.reason,
        lines: vec![
            format!(
                "Residual blockers: {}",
                gate.value
                    .as_ref()
                    .map(|artifact| artifact.residual_blockers.len())
                    .unwrap_or(0)
            ),
            source_coverage_summary_line(
                source_coverage.value.as_ref(),
                source_coverage.problem.as_ref(),
            ),
            String::from("PM summary ready: yes"),
            format!("Author actions: {action_count}"),
        ],
        next: if summary.review_signal == ReviewSignal::Unknown {
            Some(String::from(
                "Resolve missing blocker analysis inputs before making a merge decision.",
            ))
        } else {
            None
        },
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

struct ReportSummary {
    status: Status,
    data_coverage: DataCoverage,
    review_signal: ReviewSignal,
    reason: Option<String>,
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

fn source_coverage_summary_line(
    artifact: Option<&SourceCoverageArtifact>,
    problem: Option<&ArtifactProblem>,
) -> String {
    if let Some(artifact) = artifact {
        let incomplete_required = artifact
            .sources
            .iter()
            .filter(|source| source.required && source.status != SourceCoverageStatus::Full)
            .count();
        return format!(
            "Source coverage: {} ({incomplete_required} incomplete required sources)",
            artifact.data_coverage.terminal_label()
        );
    }

    if let Some(problem) = problem {
        return format!("Source coverage: unavailable ({})", problem.reason);
    }

    String::from("Source coverage: unavailable")
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

fn report_summary(
    scan: Option<&ScanArtifact>,
    source_coverage: Option<&SourceCoverageArtifact>,
    gate: Option<&GateArtifact>,
    draft: Option<&DraftReplyArtifact>,
    escalation: Option<&str>,
    input_problems: &[ArtifactProblem],
) -> ReportSummary {
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
    if let Some(source_coverage) = source_coverage {
        status = status.merge(source_coverage.status);
    }
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

    let data_coverage = if let Some(gate) = gate {
        gate.data_coverage
    } else if gate_missing_or_unreadable(input_problems) {
        DataCoverage::Failed
    } else {
        scan.map(|artifact| artifact.data_coverage)
            .unwrap_or(DataCoverage::Failed)
    };

    let review_signal = gate
        .map(|artifact| artifact.review_signal)
        .unwrap_or(ReviewSignal::Unknown);

    let reason = gate
        .and_then(|artifact| artifact.reason.clone())
        .or_else(|| draft.and_then(|artifact| artifact.reason.clone()))
        .or_else(|| escalation_reason.clone())
        .or_else(|| scan.and_then(|artifact| artifact.reason.clone()))
        .or_else(|| source_coverage.and_then(|artifact| artifact.reason.clone()))
        .or_else(|| input_problems.first().map(|problem| problem.reason.clone()));

    ReportSummary {
        status,
        data_coverage,
        review_signal,
        reason,
    }
}

#[cfg(test)]
#[allow(dead_code)]
pub fn report_status_and_reason_for_tests(
    scan: Option<&ScanArtifact>,
    source_coverage: Option<&SourceCoverageArtifact>,
    gate: Option<&GateArtifact>,
    draft: Option<&DraftReplyArtifact>,
    escalation: Option<&str>,
) -> (Status, DataCoverage, ReviewSignal, Option<String>) {
    let summary = report_summary(scan, source_coverage, gate, draft, escalation, &[]);
    (
        summary.status,
        summary.data_coverage,
        summary.review_signal,
        summary.reason,
    )
}

fn markdown_status(markdown: &str) -> Option<Status> {
    markdown.lines().find_map(|line| match line.trim() {
        "RUN_STATUS: OK" => Some(Status::Ok),
        "RUN_STATUS: PARTIAL" => Some(Status::Partial),
        "RUN_STATUS: ERROR" => Some(Status::Error),
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

fn gate_missing_or_unreadable(problems: &[ArtifactProblem]) -> bool {
    problems.iter().any(|problem| {
        problem.reason.contains("gate.json not found")
            || problem.reason.contains("gate.json could not be read")
    })
}

fn io_error(error: std::io::Error) -> String {
    error.to_string()
}
