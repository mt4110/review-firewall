use std::path::Path;

use rf_core::domain::{DataCoverage, GateArtifact, ReviewSignal, ScanArtifact, Status};
use rf_core::gate_scan;

use crate::adapter::git;
use crate::command::CommandOutcome;
use crate::io::{artifacts, codeowners, config, run_store};

pub fn run(cwd: &Path) -> Result<CommandOutcome, String> {
    let repo_root = git::repo_root(cwd);
    let run = run_store::latest_or_create(&repo_root.path).map_err(io_error)?;
    let scan = match artifacts::read_json::<ScanArtifact>(run.directory.join("scan.json")) {
        Ok(scan) => scan,
        Err(error) => {
            let mut artifact = error_gate_artifact(format!("scan.json could not be read: {error}"));
            artifacts::write_json(run.directory.join("gate.json"), &artifact).map_err(io_error)?;
            return Ok(command_outcome(&mut artifact));
        }
    };

    let mut artifact = if let Some(scan) = scan {
        let policy = config::load(&repo_root.path);
        let codeowners_file = codeowners::load(&repo_root.path);
        let mut gate = gate_scan(&scan, &policy.gate_snapshot(), &codeowners_file.rules);
        if let Some(codeowners_reason) = codeowners_file.reason {
            gate.status = gate.status.merge(Status::Partial);
            if gate.reason.is_none() {
                gate.reason = Some(codeowners_reason.clone());
            }
            gate.warnings.push(codeowners_reason);
        }
        if let Some(config_reason) = policy.reason {
            gate.status = gate.status.merge(policy.status);
            if gate.reason.is_none() {
                gate.reason = Some(config_reason.clone());
            }
            gate.warnings.push(config_reason);
        }
        gate
    } else {
        missing_gate_artifact()
    };

    artifacts::write_json(run.directory.join("gate.json"), &artifact).map_err(io_error)?;

    Ok(command_outcome(&mut artifact))
}

fn command_outcome(artifact: &mut GateArtifact) -> CommandOutcome {
    let mut lines = vec![
        format!("Comments analyzed: {}", artifact.comments_analyzed),
        format!("Residual blockers: {}", artifact.residual_blockers.len()),
        format!("Questions: {}", artifact.counts.questions),
        format!("Suggestions: {}", artifact.counts.suggestions),
        format!("Nits: {}", artifact.counts.nits),
        format!("Praise: {}", artifact.counts.praise),
    ];
    if let Some(summary) = artifact.review_decision_summary.as_ref() {
        lines.push(format!(
            "Review state: {} (informational only)",
            summary.states.join(", ")
        ));
    }
    if let Some(top) = artifact.residual_blockers.first() {
        lines.push(format!(
            "Top blocker: [{}] {} ({})",
            concern_label(&top.concern),
            top.failure_mode,
            weight_label(&top.advisory_weight)
        ));
    }

    CommandOutcome {
        status: artifact.status,
        data_coverage: artifact.data_coverage,
        review_signal: artifact.review_signal,
        residual_blockers: artifact.residual_blockers.len(),
        reason: artifact.reason.take(),
        lines,
        next: if artifact.review_signal == ReviewSignal::Unknown {
            Some(String::from(
                "Resolve missing review inputs before treating this PR as clear.",
            ))
        } else if artifact.review_signal == ReviewSignal::Clear
            && artifact
                .review_decision_summary
                .as_ref()
                .is_some_and(|summary| summary.changes_requested)
        {
            Some(String::from(
                "No residual blocker was extracted, but GitHub still shows changes requested; align it with concrete PR-local evidence or clear the stale review state.",
            ))
        } else {
            None
        },
    }
}

fn missing_gate_artifact() -> GateArtifact {
    error_gate_artifact("scan.json not found; run review-firewall scan first")
}

fn error_gate_artifact(reason: impl Into<String>) -> GateArtifact {
    GateArtifact {
        status: Status::Error,
        data_coverage: DataCoverage::Failed,
        review_signal: ReviewSignal::Unknown,
        reason: Some(reason.into()),
        comments_analyzed: 0,
        residual_blockers: Vec::new(),
        counts: Default::default(),
        candidate_blockers: Vec::new(),
        downgraded_comments: Vec::new(),
        duplicates_collapsed: Vec::new(),
        warnings: Vec::new(),
        config_snapshot: Default::default(),
        review_decision_summary: None,
        classified_comments: Vec::new(),
        escalation_candidates: Vec::new(),
    }
}

fn concern_label(concern: &rf_core::domain::BlockerConcern) -> &'static str {
    match concern {
        rf_core::domain::BlockerConcern::Correctness => "correctness",
        rf_core::domain::BlockerConcern::Security => "security",
        rf_core::domain::BlockerConcern::Performance => "performance",
        rf_core::domain::BlockerConcern::Operability => "operability",
        rf_core::domain::BlockerConcern::Api => "api",
    }
}

fn weight_label(weight: &rf_core::domain::AdvisoryWeight) -> &'static str {
    match weight {
        rf_core::domain::AdvisoryWeight::High => "high",
        rf_core::domain::AdvisoryWeight::Medium => "medium",
        rf_core::domain::AdvisoryWeight::Low => "low",
    }
}

fn io_error(error: std::io::Error) -> String {
    error.to_string()
}
